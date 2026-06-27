//! Proposer selection strategies.
//!
//! Defines the [`ProposerSelection`] trait, the V1 [`RoundRobin`]
//! implementation (per ADR-003), and the [`ProposerSchedule`] wrapper
//! that binds an active set to an era with a start height offset.

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
        assert!(
            !active_set.is_empty(),
            "RoundRobin requires non-empty active set"
        );
        active_set[slot as usize % active_set.len()]
    }
}

// ---------------------------------------------------------------------------
// ProposerSchedule
// ---------------------------------------------------------------------------

/// An era-bound proposer schedule that wraps a [`ProposerSelection`]
/// strategy with a known active set and start height.
///
/// Created at era boundaries when the validator set changes.
/// The schedule is deterministic for a given (active_set, start_height)
/// pair.
#[derive(Debug, Clone)]
pub struct ProposerSchedule {
    active_set: Vec<Address>,
    era: u64,
    start_height: u64,
}

impl ProposerSchedule {
    /// Create a new proposer schedule for the given era.
    #[must_use]
    pub fn new(active_set: Vec<Address>, era: u64, start_height: u64) -> Self {
        Self {
            active_set,
            era,
            start_height,
        }
    }

    /// Return the proposer address for a given block height.
    ///
    /// # Panics
    ///
    /// Panics if the active set is empty (should be guarded by caller).
    #[must_use]
    pub fn proposer_for_height(&self, height: u64) -> Address {
        assert!(
            !self.active_set.is_empty(),
            "ProposerSchedule requires non-empty active set"
        );
        let slot = height.saturating_sub(self.start_height);
        self.active_set[slot as usize % self.active_set.len()]
    }

    /// Returns `true` if `proposer` is the scheduled proposer for `height`.
    #[must_use]
    pub fn is_scheduled_proposer(&self, proposer: &Address, height: u64) -> bool {
        self.proposer_for_height(height) == *proposer
    }

    // -- accessors --------------------------------------------------------

    /// The active validator set for this schedule.
    #[must_use]
    pub fn active_set(&self) -> &[Address] {
        &self.active_set
    }

    /// The era this schedule was created for.
    #[must_use]
    pub const fn era(&self) -> u64 {
        self.era
    }

    /// The block height this schedule starts at.
    #[must_use]
    pub const fn start_height(&self) -> u64 {
        self.start_height
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(b: u8) -> Address {
        Address::from([b; 32])
    }

    // ---- RoundRobin tests (existing) -----------------------------------

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

    // ---- ProposerSchedule tests ---------------------------------------

    #[test]
    fn test_proposer_schedule_cycles_with_start_height() {
        let set = vec![addr(1), addr(2), addr(3)];
        let sched = ProposerSchedule::new(set, 1, 100);

        // height 100 → slot 0 → addr(1)
        assert_eq!(sched.proposer_for_height(100), addr(1));
        assert_eq!(sched.proposer_for_height(101), addr(2));
        assert_eq!(sched.proposer_for_height(102), addr(3));
        assert_eq!(sched.proposer_for_height(103), addr(1)); // wraps
    }

    #[test]
    fn test_proposer_schedule_single_validator() {
        let set = vec![addr(42)];
        let sched = ProposerSchedule::new(set, 1, 50);
        for h in 50..100 {
            assert_eq!(sched.proposer_for_height(h), addr(42));
        }
    }

    #[test]
    fn test_proposer_schedule_with_start_height_zero() {
        let set = vec![addr(1), addr(2)];
        let sched = ProposerSchedule::new(set, 0, 0);
        assert_eq!(sched.proposer_for_height(0), addr(1));
        assert_eq!(sched.proposer_for_height(1), addr(2));
        assert_eq!(sched.proposer_for_height(2), addr(1));
    }

    #[test]
    fn test_is_scheduled_proposer() {
        let set = vec![addr(10), addr(20), addr(30)];
        let sched = ProposerSchedule::new(set, 1, 0);

        assert!(sched.is_scheduled_proposer(&addr(10), 0));
        assert!(sched.is_scheduled_proposer(&addr(20), 1));
        assert!(sched.is_scheduled_proposer(&addr(30), 2));
        assert!(!sched.is_scheduled_proposer(&addr(99), 0));
    }

    #[test]
    fn test_proposer_schedule_accessors() {
        let set = vec![addr(1)];
        let sched = ProposerSchedule::new(set.clone(), 2, 200);
        assert_eq!(sched.active_set(), &set);
        assert_eq!(sched.era(), 2);
        assert_eq!(sched.start_height(), 200);
    }

    #[test]
    #[should_panic(expected = "non-empty")]
    fn test_proposer_schedule_empty_set_panics() {
        let sched = ProposerSchedule::new(vec![], 1, 0);
        sched.proposer_for_height(0);
    }
}
