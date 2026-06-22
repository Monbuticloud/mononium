//! Era calculation and bootstrap phase management.
//!
//! Per ADR-014: era length is 720 blocks. Era 0 (bootstrap) uses `Open`
//! election mode — no stake required. Era 1+ uses standard `TopN` election.

use primitive_types::U256;

use crate::consensus::election::ElectionMode;
use crate::core::constants::MAX_VALIDATORS;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Number of blocks in one era.
pub const ERA_LENGTH: u64 = 720;

/// Penalty per missed slot (0.08 MONEX = 8 × 10^30 MOXX).
pub const MISSED_SLOT_PENALTY: U256 = U256([3722066015023005696, 433680868994, 0, 0]);

// ---------------------------------------------------------------------------
// Era calculation
// ---------------------------------------------------------------------------

/// Return the era number for a given block height.
///
/// Era 0 starts at block 0. Era 1 starts at block `ERA_LENGTH`.
/// Blocks within an era: `height % ERA_LENGTH` gives the intra-era index.
#[must_use]
pub fn era_at_height(height: u64) -> u64 {
    height / ERA_LENGTH
}

/// Return `true` if `height` is the **first block of an era**
/// (i.e. an era boundary — election runs _before_ this block).
///
/// The proposer of this block must run the election and commit the new
/// active set as part of block execution.
///
/// Block 0 is the start of era 0 (genesis).
#[must_use]
pub fn is_era_boundary(height: u64) -> bool {
    height > 0 && height % ERA_LENGTH == 0
}

/// Return the election mode for a given era.
///
/// Era 0 → `Open` (no minimum stake, all registered validators active).
/// Era 1+ → `TopN` with the configured `max_validators` limit.
#[must_use]
pub fn election_mode_for_era(era: u64, max_validators: Option<usize>) -> ElectionMode {
    if era == 0 {
        ElectionMode::Open
    } else {
        ElectionMode::TopN {
            max_validators: max_validators.unwrap_or(MAX_VALIDATORS),
        }
    }
}

/// Return the block height at which era `target` starts.
#[must_use]
pub fn era_start_height(target: u64) -> u64 {
    target * ERA_LENGTH
}

/// Return the block height of the last block in era `target`.
#[must_use]
pub fn era_end_height(target: u64) -> u64 {
    (target + 1) * ERA_LENGTH - 1
}

/// Compute the total penalty for `missed` slots at the standard rate.
///
/// Each missed slot costs `MISSED_SLOT_PENALTY` MOXX (0.08 MONEX).
/// The penalty is applied at era boundaries (never mid-era).
#[must_use]
pub fn compute_missed_slot_penalty(missed: u64) -> U256 {
    MISSED_SLOT_PENALTY * U256::from(missed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_era_0_at_genesis() {
        assert_eq!(era_at_height(0), 0);
        assert_eq!(era_at_height(1), 0);
        assert_eq!(era_at_height(719), 0);
    }

    #[test]
    fn test_era_1_starts_at_720() {
        assert_eq!(era_at_height(720), 1);
        assert_eq!(era_at_height(721), 1);
        assert_eq!(era_at_height(1439), 1);
    }

    #[test]
    fn test_era_2_at_1440() {
        assert_eq!(era_at_height(1440), 2);
    }

    #[test]
    fn test_is_era_boundary() {
        assert!(!is_era_boundary(0));   // genesis
        assert!(!is_era_boundary(1));
        assert!(is_era_boundary(720));
        assert!(is_era_boundary(1440));
        assert!(!is_era_boundary(1441));
    }

    #[test]
    fn test_election_mode_era_0() {
        let mode = election_mode_for_era(0, None);
        assert_eq!(mode, ElectionMode::Open);
    }

    #[test]
    fn test_election_mode_era_1_uses_top_n() {
        let mode = election_mode_for_era(1, Some(21));
        assert_eq!(mode, ElectionMode::TopN { max_validators: 21 });
    }

    #[test]
    fn test_election_mode_defaults_to_max_validators() {
        let mode = election_mode_for_era(1, None);
        assert_eq!(mode, ElectionMode::TopN { max_validators: MAX_VALIDATORS });
    }

    #[test]
    fn test_era_start_height() {
        assert_eq!(era_start_height(0), 0);
        assert_eq!(era_start_height(1), 720);
        assert_eq!(era_start_height(2), 1440);
    }

    #[test]
    fn test_era_end_height() {
        assert_eq!(era_end_height(0), 719);
        assert_eq!(era_end_height(1), 1439);
        assert_eq!(era_end_height(2), 2159);
    }

    #[test]
    fn test_large_era_number() {
        let era = era_at_height(100_000_000);
        assert_eq!(era, 100_000_000 / 720);
        assert!(is_era_boundary(720 * 100_000));
    }

    // -- missed slot penalty ---------------------------------------------

    #[test]
    fn test_missed_slot_penalty_constant() {
        // 0.08 MONEX = 8 * 10^30 MOXX
        let expected = U256([3722066015023005696, 433680868994, 0, 0]);
        assert_eq!(MISSED_SLOT_PENALTY, expected);
    }

    #[test]
    fn test_compute_penalty_zero_missed() {
        assert_eq!(compute_missed_slot_penalty(0), U256::zero());
    }

    #[test]
    fn test_compute_penalty_one_missed() {
        assert_eq!(compute_missed_slot_penalty(1), MISSED_SLOT_PENALTY);
    }

    #[test]
    fn test_compute_penalty_ten_missed() {
        // 10 missed * 0.08 MONEX = 0.8 MONEX = 8 * 10^29 MOXX
        let expected = MISSED_SLOT_PENALTY * U256::from(10);
        assert_eq!(compute_missed_slot_penalty(10), expected);
    }

    #[test]
    fn test_compute_penalty_full_era_missed() {
        // 720 missed * 0.08 MONEX = 57.6 MONEX
        let expected = MISSED_SLOT_PENALTY * U256::from(720);
        assert_eq!(compute_missed_slot_penalty(720), expected);
    }
}
