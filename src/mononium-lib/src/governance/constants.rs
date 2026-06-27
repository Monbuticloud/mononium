//! Governance protocol constants and parameter bounds.

use primitive_types::U256;
use std::collections::HashMap;

use crate::core::constants::ONE_MONEX;
use crate::governance::types::GovernanceParam;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Deposit required to submit a proposal (100 MONEX).
pub const PROPOSAL_DEPOSIT: U256 = U256([4003012203950112768, 542101086242752, 0, 0]);

/// Number of eras the voting window remains open after submission.
pub const VOTING_WINDOW_ERAS: u64 = 7;

/// Maximum number of active proposals a single proposer can have.
pub const MAX_ACTIVE_PROPOSALS_PER_PROPOSER: usize = 5;

/// Maximum number of proposals per era (global).
pub const MAX_PROPOSALS_PER_ERA: usize = 50;

/// Quorum: at least ⅔ of total active stake must participate.
///
/// Uses `≥` (NOT strict `>`).
pub fn quorum_met(participating: U256, total_active: U256) -> bool {
    participating * U256::from(3) >= total_active * U256::from(2)
}

/// Approval threshold: more than 50% of participating stake.
///
/// Uses `>` (NOT `≥` — a tie is a rejection).
pub fn threshold_met(approve: U256, total_participating: U256) -> bool {
    approve * U256::from(2) > total_participating
}

/// Maximum title byte length.
pub const TITLE_MAX_BYTES: usize = 256;

/// Maximum description byte length.
pub const DESC_MAX_BYTES: usize = 4096;

// ---------------------------------------------------------------------------
// Parameter bounds
// ---------------------------------------------------------------------------

/// Bounds for each governance-mutable parameter.
///
/// Returns `(&str, (min, max))` — the param's display name and its
/// inclusive lower / upper bound.
#[must_use]
pub fn param_bounds() -> HashMap<GovernanceParam, (U256, U256)> {
    let mut m = HashMap::new();

    macro_rules! bound {
        ($variant:ident, $min:expr, $max:expr) => {
            m.insert(
                GovernanceParam::$variant,
                (U256::from($min), U256::from($max)),
            );
        };
    }

    bound!(MaxValidators, 1, 1_000);
    bound!(EraLength, 100, 10_000);
    bound!(BlockSizeCapBytes, 1024, 2_097_152);
    bound!(BlockTxCap, 1, 10_000);
    bound!(
        FlatFee,
        0,
        1_000_000_000_000_000_000_000_000_000_000_000u128
    ); // 100 MONEX
    bound!(
        PerByteRate,
        0,
        10_000_000_000_000_000_000_000_000_000_000_000u128
    ); // 1 MONEX
    bound!(
        AntiSpamDeposit,
        0,
        1_000_000_000_000_000_000_000_000_000_000_000u128
    ); // 100 MONEX
    bound!(
        MissedSlotPenalty,
        0,
        100_000_000_000_000_000_000_000_000_000_000u128
    ); // 10 MONEX
    bound!(SupplyCeilingRate, 0, 20);
    bound!(SupplyHeadroomRate, 0, 20);

    m
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // quorum
    // -----------------------------------------------------------------------

    #[test]
    fn test_quorum_met_exactly_two_thirds() {
        // 200 out of 300 = 2/3 exactly → quorum met (≥)
        assert!(quorum_met(U256::from(200), U256::from(300)));
    }

    #[test]
    fn test_quorum_met_above_two_thirds() {
        assert!(quorum_met(U256::from(250), U256::from(300)));
    }

    #[test]
    fn test_quorum_not_met_below_two_thirds() {
        assert!(!quorum_met(U256::from(199), U256::from(300)));
    }

    // -----------------------------------------------------------------------
    // threshold
    // -----------------------------------------------------------------------

    #[test]
    fn test_threshold_met_majority() {
        assert!(threshold_met(U256::from(101), U256::from(200)));
    }

    #[test]
    fn test_threshold_not_met_tie() {
        // 100 approve out of 200 participating = tie → NOT met
        assert!(!threshold_met(U256::from(100), U256::from(200)));
    }

    #[test]
    fn test_threshold_not_met_minority() {
        assert!(!threshold_met(U256::from(50), U256::from(200)));
    }

    // -----------------------------------------------------------------------
    // param_bounds
    // -----------------------------------------------------------------------

    #[test]
    fn test_param_bounds_contains_all_variants() {
        use GovernanceParam::*;
        let bounds = param_bounds();
        for param in &[
            MaxValidators,
            EraLength,
            BlockSizeCapBytes,
            BlockTxCap,
            FlatFee,
            PerByteRate,
            AntiSpamDeposit,
            MissedSlotPenalty,
            SupplyCeilingRate,
            SupplyHeadroomRate,
        ] {
            assert!(bounds.contains_key(param), "missing bound for {param:?}");
        }
    }

    #[test]
    fn test_param_bounds_min_less_than_max() {
        for (param, (min, max)) in param_bounds() {
            assert!(min < max, "param {param:?}: min {min} >= max {max}");
        }
    }
}
