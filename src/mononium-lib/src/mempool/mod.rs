//! Transaction mempool.
//!
//! Holds pending transactions awaiting block inclusion. Txs are ordered by
//! **Tip (fee) descending → Time received ascending → Nonce ascending**
//! per ADR-011.
//!
//! # Constraints
//!
//! * `max_size` — hard cap on total transactions.
//! * `min_fee` — transactions below this fee are rejected.
//! * `per_sender_cap` — maximum txs from one sender in a single block.
//! * `ttl` — stale transactions are removed by [`Mempool::evict_expired`].
//! * Duplicate (sender + nonce) — rejected on insert.

pub mod ordering;

use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use primitive_types::U256;

use crate::core::account::Address;
use crate::core::constants::DEFAULT_MIN_MEMPOOL_FEE;
use crate::core::transaction::Transaction;
use crate::error::{LibError, Result};

use self::ordering::{cmp_priority, PoolTx};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum transactions in the mempool.
pub const DEFAULT_MAX_SIZE: usize = 10_000;

/// How long a tx lives in the mempool before being evicted.
pub const DEFAULT_TTL_SECS: u64 = 600; // 10 minutes

/// Default maximum txs per sender per block.
pub const DEFAULT_PER_SENDER_CAP: usize = 50;

// ---------------------------------------------------------------------------
// MempoolConfig
// ---------------------------------------------------------------------------

/// Configuration for the mempool.
#[derive(Debug, Clone)]
pub struct MempoolConfig {
    /// Maximum total transactions in the pool.
    pub max_size: usize,
    /// Time-to-live for pending transactions.
    pub ttl: Duration,
    /// Minimum fee (MOXX) for a tx to be accepted.
    pub min_fee: U256,
    /// Maximum txs from a single sender in one block selection.
    pub per_sender_cap: usize,
}

impl Default for MempoolConfig {
    fn default() -> Self {
        Self {
            max_size: DEFAULT_MAX_SIZE,
            ttl: Duration::from_secs(DEFAULT_TTL_SECS),
            min_fee: DEFAULT_MIN_MEMPOOL_FEE,
            per_sender_cap: DEFAULT_PER_SENDER_CAP,
        }
    }
}

// ---------------------------------------------------------------------------
// Mempool
// ---------------------------------------------------------------------------

/// The transaction pool.
///
/// Thread-safe access is the caller's responsibility (wrap in `Arc<Mutex<>>`
/// or use the async interface provided by `MempoolHandle`).
pub struct Mempool {
    config: MempoolConfig,
    /// sender → (nonce → PoolTx)
    txs: HashMap<Address, BTreeMap<u64, PoolTx>>,
    /// Total number of transactions across all senders.
    count: usize,
}

impl Mempool {
    /// Create an empty mempool with the given configuration.
    #[must_use]
    pub fn new(config: MempoolConfig) -> Self {
        Self {
            config,
            txs: HashMap::new(),
            count: 0,
        }
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Insert a transaction into the pool.
    ///
    /// # Errors
    ///
    /// * `LibError::Consensus("mempool full")` if at capacity.
    /// * `LibError::Consensus("fee too low")` if below `min_fee`.
    /// * `LibError::InvalidNonce` if (sender, nonce) already exists.
    /// * `LibError::Consensus("sender at cap")` if sender already has
    ///   `per_sender_cap` txs in the pool.
    pub fn insert(&mut self, tx: Transaction) -> Result<()> {
        if tx.fee < self.config.min_fee {
            return Err(LibError::Consensus("fee too low"));
        }
        if self.count >= self.config.max_size {
            return Err(LibError::Consensus("mempool full"));
        }

        let sender_txs = self.txs.entry(tx.sender).or_default();
        if sender_txs.contains_key(&tx.nonce) {
            return Err(LibError::InvalidNonce(tx.nonce, tx.nonce));
        }
        if sender_txs.len() >= self.config.per_sender_cap {
            return Err(LibError::Consensus("sender at cap"));
        }

        let pool_tx = PoolTx::new(tx);
        sender_txs.insert(pool_tx.nonce(), pool_tx);
        self.count += 1;
        Ok(())
    }

    /// Remove a specific transaction.
    ///
    /// Returns `true` if the tx was present, `false` otherwise.
    pub fn remove(&mut self, sender: &Address, nonce: u64) -> bool {
        if let Some(sender_txs) = self.txs.get_mut(sender) {
            if sender_txs.remove(&nonce).is_some() {
                self.count -= 1;
                // Clean up empty sender entry
                if sender_txs.is_empty() {
                    self.txs.remove(sender);
                }
                return true;
            }
        }
        false
    }

    /// Select up to `max_count` transactions for block inclusion, ordered by
    /// priority (fee desc → time asc → nonce asc).
    ///
    /// No more than `per_sender_cap` txs are taken from any single sender.
    /// Selected txs are **removed** from the pool.
    #[must_use]
    pub fn select(&mut self, max_count: usize) -> Vec<Transaction> {
        if max_count == 0 || self.count == 0 {
            return Vec::new();
        }

        // Collect all txs, sorted by priority
        let mut all: Vec<&PoolTx> = self
            .txs
            .values()
            .flat_map(|m| m.values())
            .collect();
        all.sort_by(|a, b| cmp_priority(a, b)); // ascending priority

        // Apply per-sender cap
        let mut taken: HashMap<Address, usize> = HashMap::new();
        let mut result = Vec::with_capacity(max_count.min(all.len()));

        for pt in &all {
            let sender_count = taken.entry(*pt.sender()).or_insert(0);
            if *sender_count >= self.config.per_sender_cap {
                continue;
            }
            result.push(pt.tx.clone());
            *sender_count += 1;
            if result.len() >= max_count {
                break;
            }
        }

        // Remove selected txs
        for tx in &result {
            self.remove(&tx.sender, tx.nonce);
        }

        result
    }

    /// Remove all transactions whose age exceeds the TTL.
    ///
    /// Returns the number of evicted transactions.
    pub fn evict_expired(&mut self) -> usize {
        let now = Instant::now();
        let ttl = self.config.ttl;
        let mut evicted = 0;

        self.txs.retain(|_sender, sender_txs| {
            sender_txs.retain(|_nonce, pt| {
                let expired = now.duration_since(pt.received_at) >= ttl;
                if expired {
                    evicted += 1;
                }
                !expired
            });
            !sender_txs.is_empty()
        });

        self.count -= evicted;
        evicted
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Total number of pending transactions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.count
    }

    /// Whether the pool is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Number of pending transactions from a specific sender.
    #[must_use]
    pub fn sender_tx_count(&self, sender: &Address) -> usize {
        self.txs
            .get(sender)
            .map_or(0, |m| m.len())
    }

    /// Check if a specific (sender, nonce) is in the pool.
    #[must_use]
    pub fn contains(&self, sender: &Address, nonce: u64) -> bool {
        self.txs
            .get(sender)
            .is_some_and(|m| m.contains_key(&nonce))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::transaction::TxBody;
    use crate::crypto::falcon::Falcon512Signature;

    fn make_tx(sender: u8, nonce: u64, fee: u64) -> Transaction {
        Transaction {
            chain_id: 0,
            nonce,
            sender: Address::from([sender; 32]),
            fee: U256::from(fee),
            body: TxBody::Transfer {
                recipient: Address::from([0x99; 32]),
                amount: U256::from(100),
            },
            signature: Falcon512Signature::from_bytes(&[0u8; 809]).unwrap(),
        }
    }

    fn make_pool() -> Mempool {
        Mempool::new(MempoolConfig {
            max_size: 10_000,
            ttl: Duration::from_secs(600),
            min_fee: U256::from(10),
            per_sender_cap: 50,
        })
    }

    fn make_pool_with_cap(max_size: usize, per_sender_cap: usize, min_fee: u64) -> Mempool {
        Mempool::new(MempoolConfig {
            max_size,
            ttl: Duration::from_secs(600),
            min_fee: U256::from(min_fee),
            per_sender_cap,
        })
    }

    // -----------------------------------------------------------------------
    // Insert
    // -----------------------------------------------------------------------

    #[test]
    fn test_insert_increases_count() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 100)).unwrap();
        assert_eq!(pool.len(), 1);
        assert!(!pool.is_empty());
    }

    #[test]
    fn test_insert_duplicate_sender_nonce_rejected() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 100)).unwrap();
        let err = pool.insert(make_tx(1, 0, 200)).unwrap_err();
        assert!(err.to_string().contains("nonce"), "got: {err}");
    }

    #[test]
    fn test_insert_below_min_fee_rejected() {
        let mut pool = make_pool_with_cap(100, 50, 50);
        let err = pool.insert(make_tx(1, 0, 10)).unwrap_err();
        assert!(err.to_string().contains("fee too low"), "got: {err}");
    }

    #[test]
    fn test_insert_when_full_rejected() {
        let mut pool = make_pool_with_cap(1, 50, 0);
        pool.insert(make_tx(1, 0, 100)).unwrap();
        let err = pool.insert(make_tx(2, 0, 100)).unwrap_err();
        assert!(err.to_string().contains("mempool full"), "got: {err}");
    }

    #[test]
    fn test_insert_respects_sender_cap() {
        let mut pool = make_pool_with_cap(100, 3, 0);
        pool.insert(make_tx(1, 0, 100)).unwrap();
        pool.insert(make_tx(1, 1, 100)).unwrap();
        pool.insert(make_tx(1, 2, 100)).unwrap();
        let err = pool.insert(make_tx(1, 3, 100)).unwrap_err();
        assert!(err.to_string().contains("sender at cap"), "got: {err}");
    }

    // -----------------------------------------------------------------------
    // Remove
    // -----------------------------------------------------------------------

    #[test]
    fn test_remove_existing() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 100)).unwrap();
        assert!(pool.remove(&Address::from([1; 32]), 0));
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut pool = make_pool();
        assert!(!pool.remove(&Address::from([1; 32]), 0));
    }

    #[test]
    fn test_remove_cleans_up_sender_entry() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 100)).unwrap();
        pool.remove(&Address::from([1; 32]), 0);
        assert_eq!(pool.sender_tx_count(&Address::from([1; 32])), 0);
    }

    // -----------------------------------------------------------------------
    // Contains
    // -----------------------------------------------------------------------

    #[test]
    fn test_contains_after_insert() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 100)).unwrap();
        assert!(pool.contains(&Address::from([1; 32]), 0));
        assert!(!pool.contains(&Address::from([1; 32]), 1));
    }

    // -----------------------------------------------------------------------
    // Select
    // -----------------------------------------------------------------------

    #[test]
    fn test_select_returns_highest_fee_first() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 50)).unwrap();
        pool.insert(make_tx(2, 0, 200)).unwrap();
        pool.insert(make_tx(3, 0, 100)).unwrap();

        let selected = pool.select(10);
        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].fee, U256::from(200));
        assert_eq!(selected[1].fee, U256::from(100));
        assert_eq!(selected[2].fee, U256::from(50));
    }

    #[test]
    fn test_select_removes_txs_from_pool() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 100)).unwrap();
        let selected = pool.select(10);
        assert_eq!(selected.len(), 1);
        assert!(pool.is_empty());
    }

    #[test]
    fn test_select_respects_max_count() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 100)).unwrap();
        pool.insert(make_tx(2, 0, 100)).unwrap();
        pool.insert(make_tx(3, 0, 100)).unwrap();

        let selected = pool.select(2);
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn test_select_respects_per_sender_cap() {
        let mut pool = make_pool_with_cap(100, 2, 0);
        pool.insert(make_tx(1, 0, 100)).unwrap();
        pool.insert(make_tx(1, 1, 100)).unwrap();
        // Third insert is rejected by per-sender cap
        let err = pool.insert(make_tx(1, 2, 100)).unwrap_err();
        assert!(err.to_string().contains("sender at cap"), "got: {err}");

        let selected = pool.select(10);
        assert_eq!(selected.len(), 2); // cap is 2
    }

    #[test]
    fn test_select_interleaves_senders_by_fee() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 50)).unwrap();
        pool.insert(make_tx(1, 1, 50)).unwrap();
        pool.insert(make_tx(3, 0, 200)).unwrap();
        pool.insert(make_tx(2, 0, 100)).unwrap();

        let selected = pool.select(10);
        assert_eq!(selected.len(), 4);
        // Highest fee first
        assert_eq!(selected[0].sender, Address::from([3; 32]));
        assert_eq!(selected[1].sender, Address::from([2; 32]));
        // Then sender 1's txs
        assert_eq!(selected[2].sender, Address::from([1; 32]));
        assert_eq!(selected[3].sender, Address::from([1; 32]));
        // Nonce order within sender 1
        assert_eq!(selected[2].nonce, 0);
        assert_eq!(selected[3].nonce, 1);
    }

    // -----------------------------------------------------------------------
    // Evict (TTL)
    // -----------------------------------------------------------------------

    #[test]
    fn test_evict_expired_removes_old_txs() {
        let mut pool = Mempool::new(MempoolConfig {
            max_size: 100,
            ttl: Duration::from_secs(0), // everything expires immediately
            min_fee: U256::from(0),
            per_sender_cap: 50,
        });
        pool.insert(make_tx(1, 0, 100)).unwrap();
        assert_eq!(pool.len(), 1);
        // The sleep ensures at least some time passes so duration_since > 0
        std::thread::sleep(std::time::Duration::from_micros(500));
        let evicted = pool.evict_expired();
        assert_eq!(evicted, 1);
        assert!(pool.is_empty());
    }

    #[test]
    fn test_evict_leaves_fresh_txs() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 100)).unwrap();
        let evicted = pool.evict_expired();
        assert_eq!(evicted, 0);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn test_evict_mixed() {
        // First batch: TTL immediate
        let mut pool = Mempool::new(MempoolConfig {
            max_size: 100,
            ttl: Duration::from_secs(0),
            min_fee: U256::from(0),
            per_sender_cap: 50,
        });
        pool.insert(make_tx(1, 0, 100)).unwrap(); // will expire
        std::thread::sleep(std::time::Duration::from_micros(500));
        let evicted = pool.evict_expired();
        assert_eq!(evicted, 1);
        assert!(pool.is_empty());

        // Second batch: TTL long, insert stays
        pool.config.ttl = Duration::from_secs(600);
        pool.insert(make_tx(2, 0, 100)).unwrap();
        assert_eq!(pool.len(), 1);
        // No more expired
        let evicted2 = pool.evict_expired();
        assert_eq!(evicted2, 0);
        assert_eq!(pool.len(), 1);
    }

    // -----------------------------------------------------------------------
    // Sender query
    // -----------------------------------------------------------------------

    #[test]
    fn test_sender_tx_count() {
        let mut pool = make_pool();
        assert_eq!(pool.sender_tx_count(&Address::from([1; 32])), 0);
        pool.insert(make_tx(1, 0, 100)).unwrap();
        pool.insert(make_tx(1, 1, 100)).unwrap();
        pool.insert(make_tx(2, 0, 100)).unwrap();
        assert_eq!(pool.sender_tx_count(&Address::from([1; 32])), 2);
        assert_eq!(pool.sender_tx_count(&Address::from([2; 32])), 1);
    }

    // -----------------------------------------------------------------------
    // Empty select
    // -----------------------------------------------------------------------

    #[test]
    fn test_select_empty_pool() {
        let mut pool = make_pool();
        let selected = pool.select(10);
        assert!(selected.is_empty());
    }

    #[test]
    fn test_select_zero_count() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 100)).unwrap();
        let selected = pool.select(0);
        assert!(selected.is_empty());
        assert_eq!(pool.len(), 1); // tx not removed
    }

    #[test]
    fn test_select_deducts_from_sender_count() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 100)).unwrap();
        pool.insert(make_tx(1, 1, 100)).unwrap();
        assert_eq!(pool.sender_tx_count(&Address::from([1; 32])), 2);

        let selected = pool.select(1);
        assert_eq!(selected.len(), 1);
        // After select, only 1 tx left from sender 1
        assert_eq!(pool.sender_tx_count(&Address::from([1; 32])), 1);
    }

    #[test]
    fn test_remove_absent_sender_returns_false() {
        let mut pool = make_pool();
        assert!(!pool.remove(&Address::from([99; 32]), 0));
    }

    #[test]
    fn test_clear_resets_pool() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 100)).unwrap();
        pool.insert(make_tx(2, 0, 200)).unwrap();
        // Test that select(0) doesn't clear
        let selected = pool.select(0);
        assert!(selected.is_empty());
        assert_eq!(pool.len(), 2);
        // Select all to clear
        let selected = pool.select(100);
        assert_eq!(selected.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn test_insert_min_fee_boundary() {
        let mut pool = make_pool();
        // min_fee is 10 in make_pool()
        let low_fee_tx = make_tx(1, 0, 5);
        let err = pool.insert(low_fee_tx).unwrap_err();
        assert!(err.to_string().contains("min fee") || err.to_string().contains("fee"),
                "got: {err}");
    }

    #[test]
    fn test_select_only_takes_from_existing_txs() {
        let mut pool = make_pool();
        pool.insert(make_tx(1, 0, 100)).unwrap();
        let selected = pool.select(100);
        assert_eq!(selected.len(), 1);
        // Second select on empty pool returns nothing
        let empty = pool.select(100);
        assert!(empty.is_empty());
    }
}
