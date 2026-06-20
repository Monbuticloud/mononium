//! Proposer selection strategies.
//!
//! Defines the [`ProposerSelection`] trait and the V1 [`RoundRobin`]
//! implementation. Per ADR-003, VRF leader election (V2+) can be swapped
//! in via DI.

use crate::core::account::Address;

// ---------------------------------------------------------------------------
// ProposerSelection trait
// ---------------------------------------------------------------------------

/// Strategy for selecting which validator proposes the next block.
pub trait ProposerSelection: Send + Sync {
    /// Given a slot number and active validator set, return the proposer.
    fn select_proposer(&self, slot: u64, active_set: &[Address]) -> Address;
}

// ---------------------------------------------------------------------------
// RoundRobin
// ---------------------------------------------------------------------------

/// Deterministic round-robin proposer schedule.
///
/// The proposer for slot `N` is `active_set[N % active_set.len()]`.
/// Schedule resets at era boundaries when the active set changes.
#[derive(Debug, Default, Clone, Copy)]
pub struct RoundRobin;

impl RoundRobin {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl ProposerSelection for RoundRobin {
    fn select_proposer(&self, slot: u64, active_set: &[Address]) -> Address {
        assert!(!active_set.is_empty(), "RoundRobin requires non-empty active set");
        active_set[slot as usize % active_set.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(b: u8) -> Address {
        Address::from([b; 32])
    }

    #[test]
    fn test_round_robin_cycles() {
        let rr = RoundRobin::new();
        let set = vec![addr(1), addr(2), addr(3)];

        assert_eq!(rr.select_proposer(0, &set), addr(1));
        assert_eq!(rr.select_proposer(1, &set), addr(2));
        assert_eq!(rr.select_proposer(2, &set), addr(3));
        assert_eq!(rr.select_proposer(3, &set), addr(1)); // wraps
        assert_eq!(rr.select_proposer(4, &set), addr(2));
    }

    #[test]
    fn test_round_robin_single_validator() {
        let rr = RoundRobin::new();
        let set = vec![addr(5)];
        for slot in 0..10 {
            assert_eq!(rr.select_proposer(slot, &set), addr(5));
        }
    }

    #[test]
    fn test_round_robin_large_slot_number() {
        let rr = RoundRobin::new();
        let set = vec![addr(1), addr(2)];
        // slot = u64::MAX
        assert_eq!(
            rr.select_proposer(u64::MAX, &set),
            set[(u64::MAX as usize) % 2]
        );
    }

    #[test]
    #[should_panic(expected = "non-empty")]
    fn test_round_robin_empty_set_panics() {
        let rr = RoundRobin::new();
        rr.select_proposer(0, &[]);
    }

    #[test]
    fn test_round_robin_two_validators_alternate() {
        let rr = RoundRobin::new();
        let set = vec![addr(1), addr(2)];
        for slot in 0..20 {
            let expected = if slot % 2 == 0 { addr(1) } else { addr(2) };
            assert_eq!(rr.select_proposer(slot, &set), expected);
        }
    }
}
