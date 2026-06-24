//! Validator lifecycle types.
//!
//! Defines the status machine and entry type for validators on the
//! Mononium network. Used by the state machine for staking operations
//! and by the consensus engine for proposer election.

use primitive_types::U256;
use serde::{Deserialize, Serialize};
use parity_scale_codec::{Decode, Encode};

use crate::core::account::Address;

// ---------------------------------------------------------------------------
// Serde helper for Falcon-512 public key (897 bytes)
// ---------------------------------------------------------------------------

mod pubkey_serde {
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    pub fn serialize<S: Serializer>(key: &[u8; 897], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(key))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 897], D::Error> {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(&s).map_err(D::Error::custom)?;
        if bytes.len() != 897 {
            return Err(D::Error::custom("expected 897 bytes"));
        }
        let mut arr = [0u8; 897];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }
}

// ---------------------------------------------------------------------------
// ValidatorStatus
// ---------------------------------------------------------------------------

/// The lifecycle status of a validator.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum ValidatorStatus {
    /// Registered but not yet staked (declared intent).
    #[codec(index = 0)]
    Registered,
    /// Staked with a balance but not in the active set.
    #[codec(index = 1)]
    Staked {
        stake: U256,
    },
    /// Currently in the active validator set — proposes and votes.
    #[codec(index = 2)]
    Active,
    /// Withdrawal initiated — cooldown in progress.
    #[codec(index = 3)]
    Unstaking {
        release_era: u64,
        amount: U256,
    },
    /// Frozen after slashing — cannot propose, vote, or receive rewards.
    #[codec(index = 4)]
    Frozen {
        frozen_until: u64,
    },
    /// Previously frozen, now eligible to re-enter the candidate pool.
    #[codec(index = 5)]
    Thawed,
}

// ---------------------------------------------------------------------------
// ValidatorEntry
// ---------------------------------------------------------------------------

/// A registered validator's full on-chain record.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct ValidatorEntry {
    /// On-chain address (32 bytes).
    pub address: Address,
    /// Falcon-512 public key (897 bytes).
    #[serde(with = "pubkey_serde")]
    pub public_key: [u8; 897],
    /// Total stake locked to this validator (includes self-stake and
    /// delegations).
    pub stake: U256,
    /// Current lifecycle status.
    pub status: ValidatorStatus,
    /// Era in which this validator was first registered.
    pub registration_era: u64,
}

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
    fn test_validator_status_staked_scale_roundtrip() {
        let status = ValidatorStatus::Staked { stake: U256::from(100) };
        let encoded = status.encode();
        let decoded = ValidatorStatus::decode(&mut &encoded[..]).unwrap();
        assert_eq!(status, decoded);
    }

    #[test]
    fn test_validator_status_staked_json_roundtrip() {
        let status = ValidatorStatus::Staked { stake: U256::from(100) };
        let json = serde_json::to_string(&status).unwrap();
        let decoded: ValidatorStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, decoded);
    }

    #[test]
    fn test_validator_status_active() {
        let status = ValidatorStatus::Active;
        assert_eq!(format!("{status:?}"), "Active");
    }

    #[test]
    fn test_validator_status_unstaking_scale_roundtrip() {
        let status = ValidatorStatus::Unstaking {
            release_era: 168,
            amount: U256::from(500),
        };
        let encoded = status.encode();
        let decoded = ValidatorStatus::decode(&mut &encoded[..]).unwrap();
        assert_eq!(status, decoded);
    }

    #[test]
    fn test_validator_status_unstaking_json_roundtrip() {
        let status = ValidatorStatus::Unstaking {
            release_era: 168,
            amount: U256::from(500),
        };
        let json = serde_json::to_string(&status).unwrap();
        let decoded: ValidatorStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, decoded);
    }

    #[test]
    fn test_validator_status_frozen_scale_roundtrip() {
        let status = ValidatorStatus::Frozen { frozen_until: 720 };
        let encoded = status.encode();
        let decoded = ValidatorStatus::decode(&mut &encoded[..]).unwrap();
        assert_eq!(status, decoded);
    }

    #[test]
    fn test_validator_status_frozen_json_roundtrip() {
        let status = ValidatorStatus::Frozen { frozen_until: 720 };
        let json = serde_json::to_string(&status).unwrap();
        let decoded: ValidatorStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, decoded);
    }

    #[test]
    fn test_validator_status_thawed() {
        let status = ValidatorStatus::Thawed;
        assert_eq!(format!("{status:?}"), "Thawed");
    }

    #[test]
    fn test_validator_entry_scale_roundtrip() {
        let entry = ValidatorEntry {
            address: Address::from([0xABu8; 32]),
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
    fn test_validator_entry_json_roundtrip() {
        let entry = ValidatorEntry {
            address: Address::from([0xABu8; 32]),
            public_key: [0xBCu8; 897],
            stake: U256::from(10_000),
            status: ValidatorStatus::Staked { stake: U256::from(10_000) },
            registration_era: 0,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let decoded: ValidatorEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, decoded);
    }

    #[test]
    fn test_validator_entry_json_wrong_pubkey_length() {
        let entry = ValidatorEntry {
            address: Address::from([0xABu8; 32]),
            public_key: [0xBCu8; 897],
            stake: U256::from(10_000),
            status: ValidatorStatus::Staked { stake: U256::from(10_000) },
            registration_era: 0,
        };
        let mut json = serde_json::to_value(&entry).unwrap();
        // Replace public_key hex with a 2-byte value
        json["public_key"] = serde_json::Value::String("abcd".to_string());
        let result: std::result::Result<ValidatorEntry, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_validator_entry_edge_cases() {
        // Zero stake + registered
        let entry = ValidatorEntry {
            address: Address::from([0u8; 32]),
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
            address: Address::from([0xFFu8; 32]),
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
