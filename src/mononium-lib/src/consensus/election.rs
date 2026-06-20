//! Validator election strategies.
//!
//! Defines the [`ValidatorElection`] trait and the V1 [`TopNElection`]
//! implementation. Per ADR-002, Phragmén (V2+) can be swapped in via DI.

use primitive_types::U256;
use async_trait::async_trait;

use crate::core::account::Address;

// ---------------------------------------------------------------------------
// ValidatorCandidate
// ---------------------------------------------------------------------------

/// A candidate in the validator election pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorCandidate {
    /// Validator's on-chain address.
    pub address: Address,
    /// Total stake backing this validator (in MOXX).
    pub stake: U256,
}

impl ValidatorCandidate {
    /// Create a new election candidate.
    #[must_use]
    pub const fn new(address: Address, stake: U256) -> Self {
        Self { address, stake }
    }
}

// ---------------------------------------------------------------------------
// ElectionMode
// ---------------------------------------------------------------------------

/// Determines validator election behaviour for a given era.
///
/// Era 0 uses `Open` (no minimum stake). Era 1+ uses `TopN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElectionMode {
    /// Era 0 only — no minimum stake, all registered validators are active.
    Open,
    /// Standard Top-N election by stake descending.
    TopN {
        /// Maximum number of active validators.
        max_validators: usize,
    },
}

// ---------------------------------------------------------------------------
// ValidatorElection trait
// ---------------------------------------------------------------------------

/// Strategy for electing validators from a candidate pool.
#[async_trait]
pub trait ValidatorElection: Send + Sync {
    /// Select up to `max` validators from the candidate pool.
    ///
    /// Returns elected validator addresses in priority order (used for
    /// proposer schedule construction).
    async fn elect(&self, candidates: &[ValidatorCandidate], max: usize) -> Vec<Address>;
}

// ---------------------------------------------------------------------------
// TopNElection
// ---------------------------------------------------------------------------

/// Elects the top N candidates by stake (descending). Ties are broken by
/// address bytes (ascending) for determinism.
#[derive(Debug, Default, Clone, Copy)]
pub struct TopNElection;

impl TopNElection {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidatorElection for TopNElection {
    async fn elect(&self, candidates: &[ValidatorCandidate], max: usize) -> Vec<Address> {
        if candidates.is_empty() || max == 0 {
            return Vec::new();
        }

        let mut sorted: Vec<&ValidatorCandidate> = candidates.iter().collect();
        sorted.sort_by(|a, b| {
            // Highest stake first
            b.stake
                .cmp(&a.stake)
                // Tie-break by address (ascending) for determinism
                .then_with(|| a.address.as_bytes().cmp(b.address.as_bytes()))
        });

        sorted
            .into_iter()
            .take(max)
            .map(|c| c.address)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(b: u8) -> Address {
        Address::from([b; 32])
    }

    fn candidate(b: u8, stake: u64) -> ValidatorCandidate {
        ValidatorCandidate::new(addr(b), U256::from(stake))
    }

    #[tokio::test]
    async fn test_top_n_sorts_by_stake_desc() {
        let election = TopNElection::new();
        let candidates = vec![candidate(1, 50), candidate(2, 200), candidate(3, 100)];
        let elected = election.elect(&candidates, 10).await;
        assert_eq!(elected.len(), 3);
        assert_eq!(elected[0], addr(2)); // 200
        assert_eq!(elected[1], addr(3)); // 100
        assert_eq!(elected[2], addr(1)); // 50
    }

    #[tokio::test]
    async fn test_top_n_respects_max() {
        let election = TopNElection::new();
        let candidates = vec![candidate(1, 100), candidate(2, 200), candidate(3, 300)];
        let elected = election.elect(&candidates, 2).await;
        assert_eq!(elected.len(), 2);
        assert_eq!(elected[0], addr(3)); // 300
        assert_eq!(elected[1], addr(2)); // 200
    }

    #[tokio::test]
    async fn test_top_n_tie_break_by_address() {
        let election = TopNElection::new();
        // Lower address byte = first
        let candidates = vec![candidate(3, 100), candidate(1, 100), candidate(2, 100)];
        let elected = election.elect(&candidates, 10).await;
        // Same stake → address ascending
        assert_eq!(elected[0], addr(1));
        assert_eq!(elected[1], addr(2));
        assert_eq!(elected[2], addr(3));
    }

    #[tokio::test]
    async fn test_top_n_empty_candidates() {
        let election = TopNElection::new();
        let elected = election.elect(&[], 10).await;
        assert!(elected.is_empty());
    }

    #[tokio::test]
    async fn test_top_n_zero_max() {
        let election = TopNElection::new();
        let candidates = vec![candidate(1, 100)];
        let elected = election.elect(&candidates, 0).await;
        assert!(elected.is_empty());
    }

    #[tokio::test]
    async fn test_top_n_more_candidates_than_max() {
        let election = TopNElection::new();
        let candidates: Vec<_> = (0..10).map(|i| candidate(i, i as u64 * 10)).collect();
        let elected = election.elect(&candidates, 3).await;
        assert_eq!(elected.len(), 3);
        // Highest 3 stakes: 90, 80, 70
        assert_eq!(elected[0], addr(9));
        assert_eq!(elected[1], addr(8));
        assert_eq!(elected[2], addr(7));
    }

    #[tokio::test]
    async fn test_top_n_single_candidate() {
        let election = TopNElection::new();
        let candidates = vec![candidate(5, 500)];
        let elected = election.elect(&candidates, 1).await;
        assert_eq!(elected, vec![addr(5)]);
    }
}
