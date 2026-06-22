//! Consensus engine: validator election, proposer schedule, era management,
//! and supply policy.

pub mod election;
pub mod era;
pub mod finality;
pub mod fork_choice;
pub mod proposer;
pub mod slashing;
pub mod supply;

use std::time::Duration;

use crate::core::constants::MAX_VALIDATORS;

// ---------------------------------------------------------------------------
// ConsensusConfig
// ---------------------------------------------------------------------------

/// Global consensus configuration.
///
/// Swappable via DI (e.g., devnet uses `FixedSupply`, mainnet uses
/// `CappedInflation`).
#[derive(Debug, Clone)]
pub struct ConsensusConfig {
    /// Block production interval.
    pub block_time: Duration,
    /// Number of blocks per era (validator set recalculation interval).
    pub era_length: u64,
    /// Maximum number of active validators.
    pub max_validators: usize,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            block_time: Duration::from_secs(5),
            era_length: 720,
            max_validators: MAX_VALIDATORS,
        }
    }
}

impl ConsensusConfig {
    /// Create a new consensus configuration.
    #[must_use]
    pub const fn new(block_time: Duration, era_length: u64, max_validators: usize) -> Self {
        Self {
            block_time,
            era_length,
            max_validators,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = ConsensusConfig::default();
        assert_eq!(cfg.block_time, Duration::from_secs(5));
        assert_eq!(cfg.era_length, 720);
        assert_eq!(cfg.max_validators, MAX_VALIDATORS);
    }

    #[test]
    fn test_custom_config() {
        let cfg = ConsensusConfig::new(Duration::from_secs(3), 360, 10);
        assert_eq!(cfg.block_time, Duration::from_secs(3));
        assert_eq!(cfg.era_length, 360);
        assert_eq!(cfg.max_validators, 10);
    }
}
