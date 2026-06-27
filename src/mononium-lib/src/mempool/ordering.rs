//! Mempool transaction ordering.
//!
//! Priority order: **Tip (fee) descending → Time received ascending → Nonce ascending**.
//! Per ADR-011.

use std::cmp::Ordering;
use std::time::Instant;

use primitive_types::U256;

use crate::core::account::Address;
use crate::core::transaction::Transaction;

/// A transaction inside the mempool, tagged with its arrival time.
#[derive(Debug, Clone)]
pub struct PoolTx {
    /// The full signed transaction.
    pub tx: Transaction,
    /// Monotonic timestamp when the tx was inserted.
    pub received_at: Instant,
}

impl PoolTx {
    /// Create a new pool entry.
    #[must_use]
    pub fn new(tx: Transaction) -> Self {
        Self {
            tx,
            received_at: Instant::now(),
        }
    }

    /// Create a pool entry with a specific receive time (for testing).
    #[must_use]
    pub fn new_at(tx: Transaction, received_at: Instant) -> Self {
        Self { tx, received_at }
    }

    /// Convenience accessors.
    pub fn sender(&self) -> &Address {
        &self.tx.sender
    }
    pub fn nonce(&self) -> u64 {
        self.tx.nonce
    }
    pub fn fee(&self) -> U256 {
        self.tx.fee
    }
}

// ---------------------------------------------------------------------------
// Priority ordering: fee desc → time asc → nonce asc
//
// The comparator returns Less when `a` has higher priority than `b`
// (i.e. `a` should appear before `b` in a sorted list).
// ---------------------------------------------------------------------------

/// Compare two `PoolTx` for priority ordering.
///
/// Returns `Less` when `a` should come before `b` (higher priority).
/// Ordering: fee descending, then time ascending, then nonce ascending.
pub fn cmp_priority(a: &PoolTx, b: &PoolTx) -> Ordering {
    // 1. Highest fee first
    b.fee()
        .cmp(&a.fee())
        // 2. Earliest received first
        .then_with(|| a.received_at.cmp(&b.received_at))
        // 3. Lowest nonce first
        .then_with(|| a.nonce().cmp(&b.nonce()))
}

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

    // `cmp_priority(a, b)` returns:
    //   Less    → a has higher priority (a before b)
    //   Greater → b has higher priority (b before a)

    #[test]
    fn test_higher_fee_higher_priority() {
        let low = PoolTx::new(make_tx(1, 0, 50));
        let high = PoolTx::new(make_tx(1, 0, 100));
        // high has higher fee → high before low
        assert_eq!(cmp_priority(&low, &high), Ordering::Greater);
        assert_eq!(cmp_priority(&high, &low), Ordering::Less);
    }

    #[test]
    fn test_earlier_time_higher_priority_same_fee() {
        let now = Instant::now();
        let early = PoolTx::new_at(make_tx(1, 0, 100), now);
        let late = PoolTx::new_at(make_tx(2, 0, 100), now + std::time::Duration::from_secs(1));
        // same fee → earlier first
        assert_eq!(cmp_priority(&early, &late), Ordering::Less);
        assert_eq!(cmp_priority(&late, &early), Ordering::Greater);
    }

    #[test]
    fn test_lower_nonce_higher_priority_same_fee_time() {
        let now = Instant::now();
        let a = PoolTx::new_at(make_tx(1, 5, 100), now);
        let b = PoolTx::new_at(make_tx(2, 0, 100), now);
        // same fee + same time → lower nonce first
        assert_eq!(cmp_priority(&b, &a), Ordering::Less);
        assert_eq!(cmp_priority(&a, &b), Ordering::Greater);
    }

    #[test]
    fn test_priority_sort_orders_correctly() {
        let now = Instant::now();
        let best = PoolTx::new_at(make_tx(1, 5, 200), now);
        let mid = PoolTx::new_at(make_tx(2, 0, 100), now + std::time::Duration::from_secs(1));
        let mid2 = PoolTx::new_at(make_tx(3, 3, 100), now);
        let worst = PoolTx::new_at(make_tx(4, 0, 50), now);

        let mut txs = vec![&worst, &mid, &best, &mid2];
        txs.sort_by(|a, b| cmp_priority(a, b));

        // Expected: best(mid2(early → mid(late → worst
        assert_eq!(txs[0].fee(), U256::from(200)); // best (highest fee)
        assert_eq!(txs[1].fee(), U256::from(100)); // mid2 (earliest time)
        assert_eq!(txs[2].fee(), U256::from(100)); // mid (later time)
        assert_eq!(txs[3].fee(), U256::from(50)); // worst (lowest fee)
    }
}
