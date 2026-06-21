//! BFT commit finality tracking.
//!
//! [`CommitTracker`] accumulates validator commit votes and determines when
//! a block has reached >⅔ finality.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use primitive_types::U256;

use crate::core::account::Address;
use crate::core::block::CommitVote;

// ---------------------------------------------------------------------------
// CommitTracker
// ---------------------------------------------------------------------------

/// Tracks CommitVotes and determines when blocks reach >⅔ finality.
///
/// Constructed at an era boundary with the active validator set and their
/// stake weights.  Votes are accepted only from validators in that set.
pub struct CommitTracker {
    /// Per-height votes received so far.
    commits: BTreeMap<u64, Vec<CommitVote>>,
    /// Validator address → stake weight.
    stake_weights: HashMap<Address, U256>,
    /// Sum of all active stake at construction time.
    total_active_stake: U256,
    /// Heights that have reached >⅔ finality.
    finalized: BTreeSet<u64>,
}

impl CommitTracker {
    /// Create a new tracker with the active validator stake weights.
    #[must_use]
    pub fn new(stake_weights: HashMap<Address, U256>) -> Self {
        todo!()
    }

    /// Record a vote for a block.
    ///
    /// Returns `true` if the vote was accepted and processed.
    /// Returns `false` (no-op) if:
    /// - The validator is not in the active set.
    /// - The validator already voted for this height.
    /// - The height is already final.
    pub fn add_vote(&mut self, vote: CommitVote) -> bool {
        let _ = vote;
        todo!()
    }

    /// Sum of unique validator stake that has voted at `height`.
    #[must_use]
    pub fn cumulative_weight(&self, height: u64) -> U256 {
        let _ = height;
        todo!()
    }

    /// Whether the block at `height` has reached >⅔ finality.
    #[must_use]
    pub fn is_final(&self, height: u64) -> bool {
        let _ = height;
        todo!()
    }

    /// Highest height that has been finalised.
    #[must_use]
    pub fn last_finalized_height(&self) -> u64 {
        todo!()
    }

    /// Ratio of participating stake to total active stake at `height`.
    #[must_use]
    pub fn finality_ratio(&self, height: u64) -> f64 {
        let _ = height;
        todo!()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::account::Address;
    use crate::crypto::falcon::Falcon512Signature;

    fn addr(b: u8) -> Address {
        Address::from([b; 32])
    }

    fn vote(height: u64, validator: Address) -> CommitVote {
        CommitVote {
            height,
            block_hash: [0; 32],
            validator,
            signature: Falcon512Signature::from_bytes(&[0u8; 809]).unwrap(),
        }
    }

    fn weights(validators: &[(Address, u64)]) -> HashMap<Address, U256> {
        validators
            .iter()
            .map(|(addr, stake)| (*addr, U256::from(*stake)))
            .collect()
    }

    // -----------------------------------------------------------------------
    // construction
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_tracker_has_no_commits() {
        let w = weights(&[(addr(0xAA), 100)]);
        let tracker = CommitTracker::new(w);
        assert!(!tracker.is_final(1));
        assert_eq!(tracker.last_finalized_height(), 0);
    }
}
