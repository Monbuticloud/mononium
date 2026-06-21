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
        let total_active_stake = stake_weights.values().copied().fold(U256::zero(), |acc, w| acc.saturating_add(w));
        Self {
            commits: BTreeMap::new(),
            stake_weights,
            total_active_stake,
            finalized: BTreeSet::new(),
        }
    }

    /// Record a vote for a block.
    ///
    /// Returns `true` if the vote was accepted and processed.
    /// Returns `false` (no-op) if:
    /// - The validator is not in the active set.
    /// - The validator already voted for this height.
    /// - The height is already final.
    pub fn add_vote(&mut self, vote: CommitVote) -> bool {
        let height = vote.height;
        // Reject if height already final
        if self.finalized.contains(&height) {
            return false;
        }
        // Reject if validator not in active set
        if !self.stake_weights.contains_key(&vote.validator) {
            return false;
        }
        // Reject duplicate vote at same height
        let votes = self.commits.entry(height).or_default();
        if votes.iter().any(|v| v.validator == vote.validator) {
            return false;
        }
        votes.push(vote);

        // Check if threshold is crossed (cumulative * 3 > total * 2)
        let cumul = self.cumulative_weight(height);
        if cumul.checked_mul(U256::from(3)).map_or(false, |c3| {
            self.total_active_stake.checked_mul(U256::from(2)).map_or(true, |t2| c3 > t2)
        }) {
            self.finalized.insert(height);
        }
        true
    }

    /// Sum of unique validator stake that has voted at `height`.
    #[must_use]
    pub fn cumulative_weight(&self, height: u64) -> U256 {
        let Some(votes) = self.commits.get(&height) else {
            return U256::zero();
        };
        votes
            .iter()
            .filter_map(|v| self.stake_weights.get(&v.validator))
            .copied()
            .fold(U256::zero(), |acc, w| acc.saturating_add(w))
    }

    /// Whether the block at `height` has reached >⅔ finality.
    #[must_use]
    pub fn is_final(&self, height: u64) -> bool {
        self.finalized.contains(&height)
    }

    /// Highest height that has been finalised.
    #[must_use]
    pub fn last_finalized_height(&self) -> u64 {
        self.finalized.last().copied().unwrap_or(0)
    }

    /// Ratio of participating stake to total active stake at `height`.
    #[must_use]
    pub fn finality_ratio(&self, height: u64) -> f64 {
        let total = self.total_active_stake;
        if total.is_zero() {
            return 0.0;
        }
        let weight = self.cumulative_weight(height);
        // Exact conversion for values ≤ 2¹²⁸; lossy but acceptable for diagnostics.
        let weight_f = weight.low_u128() as f64;
        let total_f = total.low_u128() as f64;
        if total_f == 0.0 {
            0.0
        } else {
            weight_f / total_f
        }
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

    // -----------------------------------------------------------------------
    // add_vote
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_vote_accepts_validator_vote() {
        let mut tracker = CommitTracker::new(weights(&[(addr(0xAA), 100)]));
        assert!(tracker.add_vote(vote(1, addr(0xAA))));
    }

    #[test]
    fn test_add_vote_rejects_non_active() {
        let mut tracker = CommitTracker::new(weights(&[(addr(0xAA), 100)]));
        assert!(!tracker.add_vote(vote(1, addr(0xBB))));
    }

    #[test]
    fn test_add_vote_rejects_duplicate() {
        let mut tracker = CommitTracker::new(weights(&[(addr(0xAA), 100)]));
        assert!(tracker.add_vote(vote(1, addr(0xAA))));
        assert!(!tracker.add_vote(vote(1, addr(0xAA))));
    }

    #[test]
    fn test_add_vote_different_heights_allowed() {
        let mut tracker = CommitTracker::new(weights(&[(addr(0xAA), 100)]));
        assert!(tracker.add_vote(vote(1, addr(0xAA))));
        assert!(tracker.add_vote(vote(2, addr(0xAA))));
    }

    // -----------------------------------------------------------------------
    // cumulative_weight
    // -----------------------------------------------------------------------

    #[test]
    fn test_cumulative_weight_zero_for_no_votes() {
        let tracker = CommitTracker::new(weights(&[(addr(0xAA), 100)]));
        assert_eq!(tracker.cumulative_weight(1), U256::zero());
    }

    #[test]
    fn test_cumulative_weight_sums_unique_validators() {
        let mut tracker = CommitTracker::new(weights(&[
            (addr(0xAA), 100),
            (addr(0xBB), 200),
        ]));
        tracker.add_vote(vote(1, addr(0xAA)));
        assert_eq!(tracker.cumulative_weight(1), U256::from(100));
        tracker.add_vote(vote(1, addr(0xBB)));
        assert_eq!(tracker.cumulative_weight(1), U256::from(300));
    }

    #[test]
    fn test_cumulative_weight_ignores_duplicate() {
        let mut tracker = CommitTracker::new(weights(&[(addr(0xAA), 100)]));
        tracker.add_vote(vote(1, addr(0xAA)));
        tracker.add_vote(vote(1, addr(0xAA))); // duplicate
        assert_eq!(tracker.cumulative_weight(1), U256::from(100));
    }

    // -----------------------------------------------------------------------
    // finality (>⅔ threshold)
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_final_false_below_threshold() {
        // 3 validators, each 100 stake → total = 300, ⅔ = 200, >⅔ = 201
        let vals = &[(addr(0xAA), 100), (addr(0xBB), 100), (addr(0xCC), 100)];
        let mut tracker = CommitTracker::new(weights(vals));
        tracker.add_vote(vote(1, addr(0xAA))); // 100 / 300 → not final
        assert!(!tracker.is_final(1));
    }

    #[test]
    fn test_is_final_true_when_exceeds_two_thirds() {
        // 3 validators, each 100 stake → total = 300, >⅔ = >200
        let vals = &[(addr(0xAA), 100), (addr(0xBB), 100), (addr(0xCC), 100)];
        let mut tracker = CommitTracker::new(weights(vals));
        tracker.add_vote(vote(1, addr(0xAA)));
        tracker.add_vote(vote(1, addr(0xBB)));
        tracker.add_vote(vote(1, addr(0xCC))); // 300 > 200 → final
        assert!(tracker.is_final(1));
    }

    #[test]
    fn test_is_final_false_at_exactly_two_thirds() {
        // 2 validators, 100 + 50 = 150 total, >⅔ = >100
        // One validator with 100 votes → 100 is NOT > 100
        let vals = &[(addr(0xAA), 100), (addr(0xBB), 50)];
        let mut tracker = CommitTracker::new(weights(vals));
        tracker.add_vote(vote(1, addr(0xAA))); // 100 out of 150 → 66.67% NOT final
        assert!(!tracker.is_final(1));
    }

    #[test]
    fn test_is_final_crosses_at_two_validators_out_of_three() {
        // 3 validators, 50 + 30 + 20 = 100 total, >⅔ = >66
        let vals = &[(addr(0xAA), 50), (addr(0xBB), 30), (addr(0xCC), 20)];
        let mut tracker = CommitTracker::new(weights(vals));
        tracker.add_vote(vote(1, addr(0xAA))); // 50 → not final
        tracker.add_vote(vote(1, addr(0xBB))); // 80 > 66 → final
        assert!(tracker.is_final(1));
    }

    // -----------------------------------------------------------------------
    // finality_ratio
    // -----------------------------------------------------------------------

    #[test]
    fn test_finality_ratio_zero_with_no_votes() {
        let tracker = CommitTracker::new(weights(&[(addr(0xAA), 100)]));
        assert!((tracker.finality_ratio(1) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_finality_ratio_reflects_weight() {
        let vals = &[(addr(0xAA), 75), (addr(0xBB), 25)];
        let mut tracker = CommitTracker::new(weights(vals));
        tracker.add_vote(vote(1, addr(0xAA)));
        let ratio = tracker.finality_ratio(1);
        assert!((ratio - 0.75).abs() < 1e-10);
    }
}
