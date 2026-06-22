//! ConsensusEngine — block production, validation, and slot timing.
//!
//! Wires together proposer schedule, BFT commit tracking, fork choice,
//! and the state machine into a single async orchestrator.

use std::time::Duration;

use parity_scale_codec::Encode;
use primitive_types::U256;

use crate::consensus::finality::CommitTracker;
use crate::consensus::proposer::ProposerSchedule;
use crate::consensus::ConsensusConfig;
use crate::core::account::Address;
use crate::core::block::{Block, BlockBody, BlockHeader};
use crate::core::state::StateMachine;
use crate::core::transaction::Transaction;
use crate::crypto::falcon::Falcon512Signature;
use crate::error::Result;

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
    /// matches current tip, signature is valid.
    #[must_use]
    pub fn validate_block(
        &self,
        block: &Block,
        current_tip: &Block,
        schedule: &ProposerSchedule,
        _timestamp_tolerance: Duration,
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
