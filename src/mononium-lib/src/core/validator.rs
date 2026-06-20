//! Validator lifecycle types.
//!
//! Defines the status machine and entry type for validators on the
//! Mononium network. Used by the state machine for staking operations
//! and by the consensus engine for proposer election.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_status_registered() {
        let status = ValidatorStatus::Registered;
        assert_eq!(format!("{status:?}"), "Registered");
    }

    #[test]
    fn test_validator_status_staked_roundtrip() {
        let status = ValidatorStatus::Staked { stake: U256::from(100) };
        let encoded = status.encode();
        let decoded = ValidatorStatus::decode(&mut &encoded[..]).unwrap();
        assert_eq!(status, decoded);
    }

    #[test]
    fn test_validator_status_active() {
        let status = ValidatorStatus::Active;
        assert_eq!(format!("{status:?}"), "Active");
    }

    #[test]
    fn test_validator_status_unstaking() {
        let status = ValidatorStatus::Unstaking {
            release_era: 168,
            amount: U256::from(500),
        };
        let encoded = status.encode();
        let decoded = ValidatorStatus::decode(&mut &encoded[..]).unwrap();
        assert_eq!(status, decoded);
    }

    #[test]
    fn test_validator_status_frozen() {
        let status = ValidatorStatus::Frozen { frozen_until: 720 };
        let encoded = status.encode();
        let decoded = ValidatorStatus::decode(&mut &encoded[..]).unwrap();
        assert_eq!(status, decoded);
    }

    #[test]
    fn test_validator_status_thawed() {
        let status = ValidatorStatus::Thawed;
        assert_eq!(format!("{status:?}"), "Thawed");
    }

    #[test]
    fn test_validator_entry_roundtrip() {
        let entry = ValidatorEntry {
            address: [0xABu8; 32],
            public_key: [0xBCu8; 897],
            stake: U256::from(10_000),
            status: ValidatorStatus::Staked { stake: U256::from(10_000) },
            registration_era: 0,
        };
        let encoded = entry.encode();
        let decoded = ValidatorEntry::decode(&mut &encoded[..]).unwrap();
        assert_eq!(entry, decoded);
    }

    #[test]
    fn test_validator_entry_edge_cases() {
        // Zero stake + registered
        let entry = ValidatorEntry {
            address: [0u8; 32],
            public_key: [0u8; 897],
            stake: U256::zero(),
            status: ValidatorStatus::Registered,
            registration_era: 0,
        };
        let encoded = entry.encode();
        let decoded = ValidatorEntry::decode(&mut &encoded[..]).unwrap();
        assert_eq!(entry, decoded);

        // Max stake + active
        let entry = ValidatorEntry {
            address: [0xFFu8; 32],
            public_key: [0xFEu8; 897],
            stake: U256::MAX,
            status: ValidatorStatus::Active,
            registration_era: u64::MAX,
        };
        let encoded = entry.encode();
        let decoded = ValidatorEntry::decode(&mut &encoded[..]).unwrap();
        assert_eq!(entry, decoded);
    }
}
