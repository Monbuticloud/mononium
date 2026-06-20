//! Transaction types and serialization.
//!
//! All protocol types support both SCALE (wire) and JSON (RPC) encoding.
//! Signatures use Falcon-512 ([`crate::crypto::falcon::Falcon512Signature`]).

use primitive_types::U256;
use serde::{Deserialize, Serialize};
use parity_scale_codec::{Decode, Encode};

use crate::core::account::Address;
use crate::crypto::falcon::Falcon512Signature;

// ---------------------------------------------------------------------------
// BurnTarget enum
// ---------------------------------------------------------------------------

/// Target address for a Burn transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum BurnTarget {
    /// Burn to `0x00..00` — permanently destroyed.
    Permanent,
    /// Burn to `0x00..01` — cap-refill sink.
    CapRefill,
}

// ---------------------------------------------------------------------------
// TxBody enum
// ---------------------------------------------------------------------------

/// The body of a transaction — specifies the operation to execute.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum TxBody {
    /// Transfer MONEX to another account.
    Transfer {
        recipient: Address,
        amount: U256,
    },
    /// Declare intent to validate (one-time, prerequisite for staking).
    RegisterValidator,
    /// Lock MONEX to become / activate a validator.
    Stake {
        validator: Address,
        amount: U256,
    },
    /// Convenience: registers + stakes atomically.
    RegisterAndStake {
        validator: Address,
        amount: U256,
    },
    /// Begin withdrawal from validator set (168-era cooldown).
    Unstake {
        validator: Address,
        amount: U256,
    },
    /// Send MONEX to permanent burn or cap-refill sink.
    Burn {
        target: BurnTarget,
        amount: U256,
    },
}

// ---------------------------------------------------------------------------
// Transaction envelope
// ---------------------------------------------------------------------------

/// A signed transaction ready for the mempool or block.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct Transaction {
    /// Network identifier (prevents replay across networks).
    pub chain_id: u64,
    /// Sender's next valid nonce.
    pub nonce: u64,
    /// Sender's address.
    pub sender: Address,
    /// Fee the sender is willing to pay (in MOXX).
    pub fee: U256,
    /// Transaction body — the operation to execute.
    pub body: TxBody,
    /// Falcon-512 signature over SCALE(chain_id || nonce || sender || fee || body).
    pub signature: Falcon512Signature,
}

// ---------------------------------------------------------------------------
// SCALE + JSON trait impls for Falcon512Signature
// ---------------------------------------------------------------------------

impl parity_scale_codec::Encode for Falcon512Signature {
    fn encode_to<W: parity_scale_codec::Output + ?Sized>(&self, dest: &mut W) {
        dest.write(self.as_ref());
    }
}

impl parity_scale_codec::Decode for Falcon512Signature {
    fn decode<I: parity_scale_codec::Input>(input: &mut I) -> std::result::Result<Self, parity_scale_codec::Error> {
        let mut bytes = vec![0u8; crate::crypto::constants::FALCON_SIGNATURE_SIZE];
        input.read(&mut bytes)?;
        Falcon512Signature::from_bytes(&bytes).map_err(|_| "invalid signature".into())
    }
}

impl serde::Serialize for Falcon512Signature {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(self.as_ref()))
    }
}

impl<'de> serde::Deserialize<'de> for Falcon512Signature {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
        Falcon512Signature::from_bytes(&bytes).map_err(|_| serde::de::Error::custom("invalid signature length"))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tx_body_transfer_scale_roundtrip() {
        let body = TxBody::Transfer {
            recipient: Address::from([0x11u8; 32]),
            amount: U256::from(1000),
        };
        let encoded = body.encode();
        let decoded = TxBody::decode(&mut &encoded[..]).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_tx_body_register_validator_scale_roundtrip() {
        let body = TxBody::RegisterValidator;
        let encoded = body.encode();
        let decoded = TxBody::decode(&mut &encoded[..]).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_tx_body_stake_scale_roundtrip() {
        let body = TxBody::Stake {
            validator: Address::from([0x22u8; 32]),
            amount: U256::from(5000),
        };
        let encoded = body.encode();
        let decoded = TxBody::decode(&mut &encoded[..]).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_tx_body_register_and_stake_scale_roundtrip() {
        let body = TxBody::RegisterAndStake {
            validator: Address::from([0x33u8; 32]),
            amount: U256::from(10_000),
        };
        let encoded = body.encode();
        let decoded = TxBody::decode(&mut &encoded[..]).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_tx_body_unstake_scale_roundtrip() {
        let body = TxBody::Unstake {
            validator: Address::from([0x44u8; 32]),
            amount: U256::from(2000),
        };
        let encoded = body.encode();
        let decoded = TxBody::decode(&mut &encoded[..]).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_tx_body_burn_permanent_scale_roundtrip() {
        let body = TxBody::Burn {
            target: BurnTarget::Permanent,
            amount: U256::from(100),
        };
        let encoded = body.encode();
        let decoded = TxBody::decode(&mut &encoded[..]).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_tx_body_burn_cap_refill_scale_roundtrip() {
        let body = TxBody::Burn {
            target: BurnTarget::CapRefill,
            amount: U256::from(200),
        };
        let encoded = body.encode();
        let decoded = TxBody::decode(&mut &encoded[..]).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_burn_target_permanent_scale_roundtrip() {
        let target = BurnTarget::Permanent;
        let encoded = target.encode();
        let decoded = BurnTarget::decode(&mut &encoded[..]).unwrap();
        assert_eq!(target, decoded);
    }

    #[test]
    fn test_burn_target_cap_refill_scale_roundtrip() {
        let target = BurnTarget::CapRefill;
        let encoded = target.encode();
        let decoded = BurnTarget::decode(&mut &encoded[..]).unwrap();
        assert_eq!(target, decoded);
    }

    // -----------------------------------------------------------------------
    // TxBody JSON serde
    // -----------------------------------------------------------------------

    #[test]
    fn test_tx_body_transfer_json_roundtrip() {
        let body = TxBody::Transfer {
            recipient: Address::from([0x11u8; 32]),
            amount: U256::from(1000),
        };
        let json = serde_json::to_string(&body).unwrap();
        let decoded: TxBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_tx_body_register_validator_json() {
        let body = TxBody::RegisterValidator;
        let json = serde_json::to_string(&body).unwrap();
        let decoded: TxBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_tx_body_stake_json_roundtrip() {
        let body = TxBody::Stake {
            validator: Address::from([0x22u8; 32]),
            amount: U256::from(50_000),
        };
        let json = serde_json::to_string(&body).unwrap();
        let decoded: TxBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_tx_body_register_and_stake_json_roundtrip() {
        let body = TxBody::RegisterAndStake {
            validator: Address::from([0x33u8; 32]),
            amount: U256::from(75_000),
        };
        let json = serde_json::to_string(&body).unwrap();
        let decoded: TxBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_tx_body_unstake_json_roundtrip() {
        let body = TxBody::Unstake {
            validator: Address::from([0x44u8; 32]),
            amount: U256::from(10_000),
        };
        let json = serde_json::to_string(&body).unwrap();
        let decoded: TxBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_tx_body_burn_permanent_json_roundtrip() {
        let body = TxBody::Burn {
            target: BurnTarget::Permanent,
            amount: U256::from(500),
        };
        let json = serde_json::to_string(&body).unwrap();
        let decoded: TxBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, decoded);
    }

    // -----------------------------------------------------------------------
    // Transaction envelope SCALE
    // -----------------------------------------------------------------------

    fn dummy_signature() -> Falcon512Signature {
        Falcon512Signature::from_bytes(&[0xABu8; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap()
    }

    #[test]
    fn test_transaction_scale_roundtrip() {
        let tx = Transaction {
            chain_id: 0,
            nonce: 1,
            sender: Address::from([0x55u8; 32]),
            fee: U256::from(100),
            body: TxBody::Transfer {
                recipient: Address::from([0x66u8; 32]),
                amount: U256::from(500),
            },
            signature: dummy_signature(),
        };
        let encoded = tx.encode();
        let decoded = Transaction::decode(&mut &encoded[..]).unwrap();
        assert_eq!(tx, decoded);
    }

    #[test]
    fn test_transaction_json_roundtrip() {
        let tx = Transaction {
            chain_id: 1,
            nonce: 42,
            sender: Address::from([0x77u8; 32]),
            fee: U256::from(250),
            body: TxBody::Stake {
                validator: Address::from([0x88u8; 32]),
                amount: U256::from(10_000),
            },
            signature: dummy_signature(),
        };
        let json = serde_json::to_string(&tx).unwrap();
        let decoded: Transaction = serde_json::from_str(&json).unwrap();
        assert_eq!(tx, decoded);
    }

    #[test]
    fn test_transaction_different_chain_ids() {
        let tx_a = Transaction {
            chain_id: 0,
            nonce: 0,
            sender: Address::from([0x99u8; 32]),
            fee: U256::zero(),
            body: TxBody::RegisterValidator,
            signature: dummy_signature(),
        };
        let tx_b = Transaction {
            chain_id: 1,
            nonce: 0,
            sender: Address::from([0x99u8; 32]),
            fee: U256::zero(),
            body: TxBody::RegisterValidator,
            signature: dummy_signature(),
        };
        assert_ne!(tx_a.encode(), tx_b.encode());
    }
}


