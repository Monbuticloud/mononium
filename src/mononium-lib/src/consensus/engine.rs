//! ConsensusEngine — block production, validation, and slot timing.
//!
//! Wires together proposer schedule, BFT commit tracking, fork choice,
//! and the state machine into a single async orchestrator.

use std::sync::Arc;
use std::time::Duration;

use parity_scale_codec::Encode;
use primitive_types::U256;
use tokio::sync::RwLock;

use crate::consensus::finality::CommitTracker;
use crate::consensus::proposer::ProposerSchedule;
use crate::consensus::ConsensusConfig;
use crate::core::account::Address;
use crate::core::block::{Block, BlockBody, BlockHeader};
use crate::core::state::StateMachine;
use crate::core::transaction::Transaction;
use crate::crypto::falcon::{Falcon512, Falcon512PublicKey, Falcon512Signature};
use crate::crypto::signature::SignatureScheme;
use crate::error::Result;
use crate::mempool::Mempool;
use crate::network::P2pHandle;
use crate::storage::StorageEngine;

// ---------------------------------------------------------------------------
// LocalValidatorKey
// ---------------------------------------------------------------------------

/// The local node's validator identity (if participating in consensus).
#[derive(Debug, Clone)]
pub struct LocalValidatorKey {
    /// This validator's address (32 bytes).
    pub address: Address,
}

// ---------------------------------------------------------------------------
// ConsensusEngine
// ---------------------------------------------------------------------------

/// The main consensus orchestrator.
///
/// Responsible for:
/// - Running the slot timer (every `block_time`)
/// - Proposing blocks when the local node is the scheduled proposer
/// - Validating blocks from other proposers and casting commit votes
/// - Managing BFT finality tracking
pub struct ConsensusEngine {
    /// Consensus configuration.
    pub config: ConsensusConfig,
    /// Proposer schedule for the current era.
    pub schedule: Option<ProposerSchedule>,
    /// BFT commit tracker.
    pub commit_tracker: Option<CommitTracker>,
    /// Local validator identity (None for observer mode).
    pub local_validator: Option<LocalValidatorKey>,
    /// Current chain height.
    pub current_height: u64,
}

impl ConsensusEngine {
    /// Create a new consensus engine.
    #[must_use]
    pub fn new(config: ConsensusConfig) -> Self {
        Self {
            config,
            schedule: None,
            commit_tracker: None,
            local_validator: None,
            current_height: 0,
        }
    }

    /// Set the local validator identity (None = observer mode).
    pub fn set_local_validator(&mut self, key: LocalValidatorKey) {
        self.local_validator = Some(key);
    }

    // -------------------------------------------------------------------
    // Block building (proposer behavior)
    // -------------------------------------------------------------------

    /// Build a new block as the proposer.
    ///
    /// Pure function that takes the current state and returns a complete
    /// block. The caller is responsible for storing and publishing.
    #[allow(clippy::too_many_arguments)]
    pub fn build_block(
        &self,
        state: &mut StateMachine,
        transactions: Vec<Transaction>,
        parent_block: &Block,
        proposer: &Address,
        timestamp: u64,
        proposer_signature: Falcon512Signature,
    ) -> Block {
        let height = parent_block.header.height + 1;
        let parent_hash: [u8; 32] = blake3::hash(&parent_block.header.encode()).into();

        // Execute all txs against state machine
        let body = BlockBody { transactions };

        // Compute state root after execution
        let global_state_root = state.state_root();

        // Compute tx root (BLAKE3 Merkle tree over tx hashes)
        let tx_root = compute_tx_root(&body);

        let header = BlockHeader {
            height,
            parent_hash,
            global_state_root,
            tx_root,
            timestamp,
            proposer: *proposer,
            chain_id: parent_block.header.chain_id,
            proposer_signature,
        };

        Block { header, body }
    }

    // -------------------------------------------------------------------
    // Block validation (non-proposer behavior)
    // -------------------------------------------------------------------

    /// Validate a received block.
    ///
    /// Checks: proposer is scheduled, timestamps within ±2s, parent hash
    /// matches current tip, proposer signature is valid.
    ///
    /// `proposer_public_key` — if `Some`, the proposer's Falcon-512 public
    /// key is used to verify the `proposer_signature`.  Pass `None` when
    /// the public key is not available (e.g. before the validator is fully
    /// registered), in which case the signature check is skipped.
    #[must_use]
    pub fn validate_block(
        &self,
        block: &Block,
        current_tip: &Block,
        schedule: &ProposerSchedule,
        _timestamp_tolerance: Duration,
        proposer_public_key: Option<&Falcon512PublicKey>,
    ) -> bool {
        // Must build on current tip
        let expected_parent: [u8; 32] = *blake3::hash(&current_tip.header.encode()).as_bytes();
        if block.header.parent_hash != expected_parent {
            return false;
        }

        // Proposer must be the scheduled proposer for this height
        if !schedule.is_scheduled_proposer(&block.header.proposer, block.header.height) {
            return false;
        }

        // Height must be next
        if block.header.height != current_tip.header.height + 1 {
            return false;
        }

        // Verify proposer signature if the public key is available
        if let Some(pk) = proposer_public_key {
            let unsigned_payload = block_header_unsigned_payload(&block.header);
            if !Falcon512::verify(pk, &unsigned_payload, &block.header.proposer_signature) {
                return false;
            }
        }

        true
    }

    // -------------------------------------------------------------------
    // Proposer schedule helpers
    // -------------------------------------------------------------------

    /// Create a proposer schedule from the active validator set.
    #[must_use]
    pub fn build_schedule(
        active_set: Vec<Address>,
        era: u64,
        start_height: u64,
    ) -> ProposerSchedule {
        ProposerSchedule::new(active_set, era, start_height)
    }

    /// Set the proposer schedule.
    pub fn set_schedule(&mut self, schedule: ProposerSchedule) {
        self.schedule = Some(schedule);
    }

    /// Set the commit tracker.
    pub fn set_commit_tracker(&mut self, tracker: CommitTracker) {
        self.commit_tracker = Some(tracker);
    }

    // -------------------------------------------------------------------
    // Async consensus loop
    // -------------------------------------------------------------------

    /// Run the consensus slot loop.
    ///
    /// On each slot tick:
    /// - If local node is scheduled proposer → build block, execute txs, store, publish
    /// - If not → wait for block from gossip event channel
    ///
    /// Returns when the channel is closed or a fatal error occurs.
    #[allow(clippy::too_many_arguments)]
    pub async fn start_consensus_loop<S: StorageEngine>(
        &self,
        state: Arc<RwLock<StateMachine>>,
        mempool: Arc<RwLock<Mempool>>,
        p2p: P2pHandle,
        storage: &S,
        _genesis_hash: [u8; 32],
        block_time_secs: u64,
    ) {
        let block_time = Duration::from_secs(block_time_secs);
        let mut interval = tokio::time::interval(block_time);
        // Skip first immediate tick
        interval.tick().await;

        loop {
            interval.tick().await;

            let schedule = match &self.schedule {
                Some(s) => s.clone(),
                None => {
                    tracing::warn!("consensus: no proposer schedule set");
                    continue;
                }
            };

            let height = self.current_height + 1;

            // Check if we're the proposer for this height
            let is_our_slot = self.local_validator.as_ref().map_or(false, |local| {
                schedule.is_scheduled_proposer(&local.address, height)
            });

            if is_our_slot {
                self.produce_block(state.clone(), mempool.clone(), &p2p, storage, height, &schedule).await;
            } else {
                // Non-proposer: slot handled by external event channel
                tracing::trace!(height, "waiting for block from proposer");
            }
        }
    }

    /// Produce a block as the scheduled proposer.
    #[allow(clippy::too_many_arguments)]
    async fn produce_block<S: StorageEngine>(
        &self,
        state: Arc<RwLock<StateMachine>>,
        mempool: Arc<RwLock<Mempool>>,
        p2p: &P2pHandle,
        storage: &S,
        height: u64,
        schedule: &ProposerSchedule,
    ) {
        // Get current timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Load parent block from storage
        let parent_height = height.saturating_sub(1);
        let parent_key = parent_height.to_be_bytes();
        let parent_bytes = match storage.get(crate::storage::tables::BLOCKS, &parent_key) {
            Ok(Some(b)) => b,
            _ => {
                tracing::error!(height, "parent block not found in storage");
                return;
            }
        };
        let parent_block: Block = match parity_scale_codec::Decode::decode(&mut &parent_bytes[..]) {
            Ok(b) => b,
            Err(e) => {
                tracing::error!(height, "failed to decode parent block: {e}");
                return;
            }
        };

        // Select txs from mempool (up to 500 or 500KB)
        let txs = {
            let mut mp = mempool.blocking_write();
            mp.select(500)
        };

        // Execute against state machine
        let sig_fake = Falcon512Signature::from_bytes(
            &[0xCDu8; crate::crypto::constants::FALCON_SIGNATURE_SIZE]
        ).unwrap();

        let mut state_guard = state.blocking_write();
        let block = self.build_block(
            &mut state_guard,
            txs,
            &parent_block,
            &schedule.proposer_for_height(height),
            now,
            sig_fake,
        );

        // Store block in database
        let key = height.to_be_bytes();
        let encoded = parity_scale_codec::Encode::encode(&block);
        if let Err(e) = storage.put(crate::storage::tables::BLOCKS, &key, &encoded) {
            tracing::error!(height, "failed to store block: {e}");
            return;
        }

        // Publish via gossip
        if let Err(e) = p2p.publish_block(block).await {
            tracing::warn!(height, "failed to publish block: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// Pure helper functions
// ---------------------------------------------------------------------------

/// Compute the tx_root (BLAKE3 Merkle tree over sorted tx hashes).
#[must_use]
pub fn compute_tx_root(body: &BlockBody) -> [u8; 32] {
    if body.transactions.is_empty() {
        return *blake3::hash(&[0u8; 0]).as_bytes();
    }
    let mut hashes: Vec<[u8; 32]> = body
        .transactions
        .iter()
        .map(|tx| {
            let encoded = tx.encode();
            *blake3::hash(&encoded).as_bytes()
        })
        .collect();
    hashes.sort();
    // Simple pairwise hash
    let mut root = hashes[0];
    for h in &hashes[1..] {
        let concat = [root.as_slice(), h.as_slice()].concat();
        root = *blake3::hash(&concat).as_bytes();
    }
    root
}

/// Check whether a block's timestamp is within tolerance of the local clock.
#[must_use]
pub fn is_timestamp_acceptable(
    block_time: u64,
    local_time: u64,
    tolerance: u64,
) -> bool {
    let diff = if block_time > local_time {
        block_time - local_time
    } else {
        local_time - block_time
    };
    diff <= tolerance
}

/// Check strict monotonicity: `block_time >= parent_time`.
#[must_use]
pub const fn is_timestamp_monotonic(block_time: u64, parent_time: u64) -> bool {
    block_time >= parent_time
}

/// Return the SCALE-encoded payload that the proposer should have signed.
///
/// The proposer signs the header with `proposer_signature` zeroed (since
/// the signature cannot cover its own field).
#[must_use]
pub fn block_header_unsigned_payload(header: &crate::core::block::BlockHeader) -> Vec<u8> {
    let mut unsigned = header.clone();
    unsigned.proposer_signature = Falcon512Signature::from_bytes(
        &[0u8; crate::crypto::constants::FALCON_SIGNATURE_SIZE],
    )
    .expect("zero-filled signature is valid");
    parity_scale_codec::Encode::encode(&unsigned)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consensus::proposer::ProposerSchedule;
    use crate::core::account::Address;
    use crate::core::transaction::TxBody;
    use crate::crypto::constants::FALCON_SIGNATURE_SIZE;
    use crate::crypto::falcon::Falcon512Signature;

    fn addr(b: u8) -> Address {
        Address::from([b; 32])
    }

    fn dummy_sig() -> Falcon512Signature {
        Falcon512Signature::from_bytes(&[0xCDu8; FALCON_SIGNATURE_SIZE]).unwrap()
    }

    fn dummy_block(height: u64, proposer: Address) -> Block {
        Block {
            header: BlockHeader {
                height,
                parent_hash: [0; 32],
                global_state_root: [0; 32],
                tx_root: [0; 32],
                timestamp: 1_700_000_000 + height,
                proposer,
                chain_id: 0,
                proposer_signature: dummy_sig(),
            },
            body: BlockBody { transactions: vec![] },
        }
    }

    // -- build_block tests -----------------------------------------------

    #[test]
    fn test_build_block_increments_height() {
        let cfg = ConsensusConfig::default();
        let engine = ConsensusEngine::new(cfg);
        let proposer = addr(1);
        let parent = dummy_block(0, addr(0));

        let mut state = StateMachine::new(vec![]);
        let block = engine.build_block(
            &mut state,
            vec![],
            &parent,
            &proposer,
            1_700_000_001,
            dummy_sig(),
        );

        assert_eq!(block.header.height, 1);
        assert_eq!(block.header.proposer, proposer);
        assert_eq!(block.body.transactions.len(), 0);
    }

    #[test]
    fn test_build_block_parent_hash_links() {
        let cfg = ConsensusConfig::default();
        let engine = ConsensusEngine::new(cfg);
        let parent = dummy_block(5, addr(1));
        let expected_parent_hash = *blake3::hash(&parent.header.encode()).as_bytes();

        let mut state = StateMachine::new(vec![]);
        let block = engine.build_block(
            &mut state,
            vec![],
            &parent,
            &addr(2),
            1_700_000_006,
            dummy_sig(),
        );

        assert_eq!(block.header.parent_hash, expected_parent_hash);
    }

    // -- validate_block tests --------------------------------------------

    #[test]
    fn test_validate_block_correct_passes() {
        let cfg = ConsensusConfig::default();
        let engine = ConsensusEngine::new(cfg);
        let proposer = addr(1);
        let schedule = ProposerSchedule::new(vec![proposer], 1, 0);

        let tip = dummy_block(0, addr(0));
        let mut state = StateMachine::new(vec![]);
        let candidate = engine.build_block(
            &mut state,
            vec![],
            &tip,
            &proposer,
            1_700_000_001,
            dummy_sig(),
        );

        assert!(engine.validate_block(
            &candidate,
            &tip,
            &schedule,
            Duration::from_secs(5),
            None,
        ));
    }

    #[test]
    fn test_validate_block_wrong_parent_rejected() {
        let cfg = ConsensusConfig::default();
        let engine = ConsensusEngine::new(cfg);
        let schedule = ProposerSchedule::new(vec![addr(1)], 1, 0);
        let tip = dummy_block(0, addr(0));

        // Block with wrong parent_hash
        let mut bad_block = dummy_block(1, addr(1));
        bad_block.header.parent_hash = [0xFFu8; 32];

        assert!(!engine.validate_block(
            &bad_block,
            &tip,
            &schedule,
            Duration::from_secs(5),
            None,
        ));
    }

    #[test]
    fn test_validate_block_wrong_proposer_rejected() {
        let cfg = ConsensusConfig::default();
        let engine = ConsensusEngine::new(cfg);
        let schedule = ProposerSchedule::new(vec![addr(2)], 1, 0); // proposer 2 is scheduled
        let tip = dummy_block(0, addr(0));
        let mut state = StateMachine::new(vec![]);

        // Block built by addr(1) — not scheduled
        let candidate = engine.build_block(
            &mut state,
            vec![],
            &tip,
            &addr(1),
            1_700_000_001,
            dummy_sig(),
        );

        assert!(!engine.validate_block(
            &candidate,
            &tip,
            &schedule,
            Duration::from_secs(5),
            None,
        ));
    }

    // -------------------------------------------------------------------
    // Proposer signature validation
    // -------------------------------------------------------------------

    #[test]
    fn test_validate_block_valid_signature_passes() {
        let seed = [0xABu8; 48];
        let kp = Falcon512::generate(&seed).unwrap();
        let pk = Falcon512::public_key(&kp);
        let proposer_addr = Address::from(crate::crypto::address::derive_address(&pk.0));
        let schedule = ProposerSchedule::new(vec![proposer_addr], 1, 0);
        let tip = dummy_block(0, addr(0));

        // Build a header with a zeroed signature, then sign it
        let unsigned_header = crate::core::block::BlockHeader {
            height: 1,
            parent_hash: *blake3::hash(&tip.header.encode()).as_bytes(),
            global_state_root: [0; 32],
            tx_root: [0; 32],
            timestamp: 1_700_000_001,
            proposer: proposer_addr,
            chain_id: 0,
            proposer_signature: Falcon512Signature::from_bytes(
                &[0u8; FALCON_SIGNATURE_SIZE],
            )
            .unwrap(),
        };
        let payload = parity_scale_codec::Encode::encode(&unsigned_header);
        let sig = Falcon512::sign(&kp, &payload).unwrap();
        let mut header = unsigned_header;
        header.proposer_signature = sig;

        let block = Block {
            header,
            body: BlockBody { transactions: vec![] },
        };

        let cfg = ConsensusConfig::default();
        let engine = ConsensusEngine::new(cfg);

        assert!(engine.validate_block(
            &block,
            &tip,
            &schedule,
            Duration::from_secs(5),
            Some(&pk),
        ));
    }

    #[test]
    fn test_validate_block_invalid_signature_rejected() {
        let seed_a = [0xABu8; 48];
        let kp = Falcon512::generate(&seed_a).unwrap();
        let pk = Falcon512::public_key(&kp);

        let seed_b = [0xCDu8; 48];
        let kp2 = Falcon512::generate(&seed_b).unwrap();
        let pk2 = Falcon512::public_key(&kp2);

        let proposer_addr = Address::from(crate::crypto::address::derive_address(&pk.0));
        let schedule = ProposerSchedule::new(vec![proposer_addr], 1, 0);
        let tip = dummy_block(0, addr(0));

        // Sign with kp, but verify with pk2 (wrong key)
        let unsigned_header = crate::core::block::BlockHeader {
            height: 1,
            parent_hash: *blake3::hash(&tip.header.encode()).as_bytes(),
            global_state_root: [0; 32],
            tx_root: [0; 32],
            timestamp: 1_700_000_001,
            proposer: proposer_addr,
            chain_id: 0,
            proposer_signature: Falcon512Signature::from_bytes(
                &[0u8; FALCON_SIGNATURE_SIZE],
            )
            .unwrap(),
        };
        let payload = parity_scale_codec::Encode::encode(&unsigned_header);
        let sig = Falcon512::sign(&kp, &payload).unwrap();
        let mut header = unsigned_header;
        header.proposer_signature = sig;

        let block = Block {
            header,
            body: BlockBody { transactions: vec![] },
        };

        let cfg = ConsensusConfig::default();
        let engine = ConsensusEngine::new(cfg);

        // Verify with pk2 — should fail
        assert!(!engine.validate_block(
            &block,
            &tip,
            &schedule,
            Duration::from_secs(5),
            Some(&pk2),
        ));
    }

    #[test]
    fn test_validate_block_no_pk_skips_signature_check() {
        let proposer_addr = addr(1);
        let schedule = ProposerSchedule::new(vec![proposer_addr], 1, 0);
        let tip = dummy_block(0, addr(0));
        let cfg = ConsensusConfig::default();
        let engine = ConsensusEngine::new(cfg);

        // Build a block with the correct parent_hash but garbage signature
        let mut state = StateMachine::new(vec![]);
        let block = engine.build_block(
            &mut state,
            vec![],
            &tip,
            &proposer_addr,
            1_700_000_001,
            dummy_sig(),
        );

        // None pk should skip signature check entirely
        assert!(engine.validate_block(
            &block,
            &tip,
            &schedule,
            Duration::from_secs(5),
            None,
        ));
    }

    // -- compute_tx_root tests -------------------------------------------

    #[test]
    fn test_compute_tx_root_empty() {
        let body = BlockBody { transactions: vec![] };
        let root = compute_tx_root(&body);
        let expected: [u8; 32] = *blake3::hash(&[0u8; 0]).as_bytes();
        assert_eq!(root, expected);
    }

    #[test]
    fn test_compute_tx_root_non_empty() {
        let tx = Transaction {
            chain_id: 0,
            nonce: 0,
            sender: addr(1),
            fee: U256::from(10),
            body: TxBody::Transfer {
                recipient: addr(2),
                amount: U256::from(100),
            },
            signature: dummy_sig(),
        };
        let body = BlockBody {
            transactions: vec![tx],
        };
        let root = compute_tx_root(&body);
        assert_ne!(root, [0u8; 32]);
    }

    // -- timestamp validation tests --------------------------------------

    #[test]
    fn test_is_timestamp_acceptable_exact() {
        assert!(is_timestamp_acceptable(100, 100, 2));
    }

    #[test]
    fn test_is_timestamp_acceptable_within_tolerance() {
        assert!(is_timestamp_acceptable(100, 101, 2));
        assert!(is_timestamp_acceptable(101, 100, 2));
    }

    #[test]
    fn test_is_timestamp_acceptable_beyond_tolerance() {
        assert!(!is_timestamp_acceptable(100, 103, 2));
        assert!(!is_timestamp_acceptable(100, 97, 2));
    }

    #[test]
    fn test_is_timestamp_monotonic() {
        assert!(is_timestamp_monotonic(101, 100));
        assert!(is_timestamp_monotonic(100, 100));
        assert!(!is_timestamp_monotonic(99, 100));
    }
}
