//! Token supply policies.
//!
//! Per ADR-005: Dev networks use [`FixedSupply`] (no inflation). Mainnet
//! uses [`CappedInflation`] (capped inflation with asymptotic approach).

use primitive_types::U256;

use crate::core::constants::{BASE_MAX_SUPPLY, SUPPLY_CEILING_RATE, SUPPLY_HEADROOM_RATE};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Approximate blocks per year (365 days at 5s/block).
pub const BLOCKS_PER_YEAR: u64 = 6_307_200;

/// Denominator for rate parameters (expressed in 1/10000).
const BASIS_POINTS: U256 = U256([10000, 0, 0, 0]);

// ---------------------------------------------------------------------------
// SupplyPolicy trait
// ---------------------------------------------------------------------------

/// Determines the block reward (in MOXX) at a given height.
///
/// `current_supply` and `effective_max` are typically snapshotted at era
/// boundaries and passed in for the block rewards of that era.
pub trait SupplyPolicy: Send + Sync {
    /// The block reward at the given height.
    ///
    /// * `current_supply` — total minted supply so far (ignored by `FixedSupply`).
    /// * `effective_max` — maximum supply including cap-refill (ignored by `FixedSupply`).
    fn block_reward(&self, height: u64, current_supply: U256, effective_max: U256) -> U256;
}

// ---------------------------------------------------------------------------
// FixedSupply (dev networks)
// ---------------------------------------------------------------------------

/// No inflation — all MONEX minted at genesis. Block rewards are always zero.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixedSupply;

impl FixedSupply {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl SupplyPolicy for FixedSupply {
    fn block_reward(&self, _height: u64, _current_supply: U256, _effective_max: U256) -> U256 {
        U256::zero()
    }
}

// ---------------------------------------------------------------------------
// CappedInflation (mainnet)
// ---------------------------------------------------------------------------

/// Capped inflation with asymptotic approach to `effective_max_supply`.
///
/// Formula (per era boundary, flat across the era):
///
/// ```text
/// headroom         = effective_max - current_supply
/// annual_reward    = min(headroom_rate × headroom, ceiling_rate × effective_max)
/// block_reward     = annual_reward / blocks_per_year
/// ```
///
/// The rates use basis points (1/10000), so headroom_rate = 500 → 5.0%
/// and ceiling_rate = 350 → 3.5%.
#[derive(Debug, Clone)]
pub struct CappedInflation {
    /// Maximum supply cap (10B MONEX in MOXX).
    pub base_max_supply: U256,
    /// Annual ceiling rate in basis points (350 = 3.5%).
    pub ceiling_rate: u32,
    /// Annual headroom rate in basis points (500 = 5.0%).
    pub headroom_rate: u32,
}

impl CappedInflation {
    /// Create the mainnet supply policy with default parameters.
    #[must_use]
    pub fn new() -> Self {
        Self {
            base_max_supply: BASE_MAX_SUPPLY,
            ceiling_rate: SUPPLY_CEILING_RATE,
            headroom_rate: SUPPLY_HEADROOM_RATE,
        }
    }

    /// Create a policy with custom parameters (for tests / dev variants).
    #[allow(dead_code)]
    #[must_use]
    pub fn with_params(
        base_max_supply: U256,
        ceiling_rate: u32,
        headroom_rate: u32,
    ) -> Self {
        Self {
            base_max_supply,
            ceiling_rate,
            headroom_rate,
        }
    }

    /// Compute the block reward given current supply and effective max supply.
    fn compute_reward(
        current_supply: U256,
        effective_max: U256,
        ceiling_rate: u32,
        headroom_rate: u32,
    ) -> U256 {
        if current_supply >= effective_max {
            return U256::zero();
        }

        let headroom = effective_max - current_supply;

        // annual_reward_headroom = headroom * headroom_rate / 10000
        let annual_headroom = headroom * U256::from(headroom_rate) / BASIS_POINTS;
        // annual_reward_ceiling = effective_max * ceiling_rate / 10000
        let annual_ceiling = effective_max * U256::from(ceiling_rate) / BASIS_POINTS;

        // annual_reward = min(headroom_term, ceiling_term)
        let annual_reward = std::cmp::min(annual_headroom, annual_ceiling);

        // block_reward = annual_reward / blocks_per_year
        annual_reward / U256::from(BLOCKS_PER_YEAR)
    }
}

impl Default for CappedInflation {
    fn default() -> Self {
        Self::new()
    }
}

impl SupplyPolicy for CappedInflation {
    fn block_reward(&self, _height: u64, current_supply: U256, effective_max: U256) -> U256 {
        Self::compute_reward(
            current_supply,
            effective_max,
            self.ceiling_rate,
            self.headroom_rate,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // FixedSupply
    // -----------------------------------------------------------------------

    #[test]
    fn test_fixed_supply_always_zero() {
        let fs = FixedSupply::new();
        assert_eq!(fs.block_reward(0, U256::zero(), U256::from(1000)), U256::zero());
        assert_eq!(fs.block_reward(1_000_000, U256::from(500), U256::from(1000)), U256::zero());
    }

    // -----------------------------------------------------------------------
    // CappedInflation — basic math
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_supply_is_flat_phase() {
        let effective_max = U256::from(10_000_000_000u64); // 10B (simplified units)
        let reward = CappedInflation::compute_reward(
            U256::zero(),
            effective_max,
            350,  // 3.5%
            500,  // 5.0%
        );
        // At supply=0: headroom = 10B, ceiling term = 350M/yr, headroom term = 500M/yr
        // min = 350M/yr → 350M / 6.3M ≈ 55.5
        assert!(reward > U256::zero());
        // Should be ~55 per block for 10B cap
        assert_eq!(reward, U256::from(55)); // 350_000_000 / 6_307_200 ≈ 55
    }

    #[test]
    fn test_at_cap_returns_zero() {
        let effective_max = U256::from(10_000_000_000u64);
        let reward = CappedInflation::compute_reward(
            effective_max,
            effective_max,
            350,
            500,
        );
        assert_eq!(reward, U256::zero());
    }

    #[test]
    fn test_above_cap_returns_zero() {
        let max = U256::from(1_000);
        let reward = CappedInflation::compute_reward(
            U256::from(1_500),
            max,
            350,
            500,
        );
        assert_eq!(reward, U256::zero());
    }

    #[test]
    fn test_headroom_term_dominates_when_supply_high() {
        let max = U256::from(1_000_000_000u64);
        let supply = U256::from(900_000_000u64); // 90% minted
        let reward = CappedInflation::compute_reward(supply, max, 350, 500);
        // headroom = 100M, headroom_term = 5% * 100M = 5M, ceiling_term = 3.5% * 1B = 35M
        // min = 5M → 5M / 6.3M ≈ 0.79 → floor = 0
        assert_eq!(reward, U256::zero()); // below 1 unit
    }

    #[test]
    fn test_ceiling_term_dominates_at_low_supply() {
        let max = U256::from(1_000_000_000u64);
        let supply = U256::from(100_000_000u64); // 10% minted
        let reward = CappedInflation::compute_reward(supply, max, 350, 500);
        // headroom = 900M, headroom_term = 5% * 900M = 45M, ceiling_term = 3.5% * 1B = 35M
        // min = 35M → 35M / 6.3M ≈ 5.55
        assert!(reward > U256::zero());
    }

    #[test]
    fn test_new_uses_default_params() {
        let ci = CappedInflation::new();
        assert_eq!(ci.base_max_supply, BASE_MAX_SUPPLY);
        assert_eq!(ci.ceiling_rate, SUPPLY_CEILING_RATE);
        assert_eq!(ci.headroom_rate, SUPPLY_HEADROOM_RATE);
    }

    #[test]
    fn test_big_supply_comparison() {
        // Use MOXX-scale values to match mainnet constants
        // 10B MONEX = 10^10 * 10^32 = 10^42 MOXX → can't fit in u64 TEST
        // Use smaller values for arithmetic correctness
        let max = U256::from(10_000_000_000_000u64); // 10T for test
        let supply = U256::from(3_000_000_000_000u64); // 30% minted
        let reward = CappedInflation::compute_reward(supply, max, 350, 500);
        // At 30%: headroom = 7T, headroom_term = 5% * 7T = 350B, ceiling = 3.5% * 10T = 350B
        // min = 350B / 6.3M ≈ 55,500
        assert!(reward > U256::zero());
        // Both terms are equal at 30%
        let expected = U256::from(350_000_000_000u64) / U256::from(BLOCKS_PER_YEAR);
        assert_eq!(reward, expected);
    }

    // -----------------------------------------------------------------------
    // SupplyPolicy trait dispatch
    // -----------------------------------------------------------------------

    #[test]
    fn test_fixed_via_trait() {
        let fs: Box<dyn SupplyPolicy> = Box::new(FixedSupply::new());
        assert_eq!(fs.block_reward(0, U256::zero(), U256::zero()), U256::zero());
    }
}
