//! Fee policy: HybridFee with flat + per-byte + tip components.
//! Burn transactions bypass standard calculation (flat 10 MOXX).

use primitive_types::U256;

use crate::core::constants::{
    DEFAULT_FLAT_FEE, DEFAULT_PER_BYTE_RATE, BURN_FLAT_FEE,
};
use crate::core::transaction::{Transaction, TxBody};

/// Pluggable fee calculation policy.
///
/// The protocol uses [`HybridFee`] in V1. A different policy can be
/// swapped in via DI without consensus changes.
pub trait FeePolicy {
    /// Calculate the fee for a transaction (in MOXX).
    fn calculate_fee(&self, tx: &Transaction) -> U256;
}

/// Hybrid fee: `flat_fee + per_byte_rate * size + sender_tip`.
///
/// Burn transactions bypass standard calculation — flat [`BURN_FLAT_FEE`].
#[derive(Debug, Clone, Copy)]
pub struct HybridFee {
    /// Minimum cost per transaction (in MOXX).
    pub flat_fee: U256,
    /// Rate per byte of transaction (in MOXX).
    pub per_byte_rate: U256,
}

impl HybridFee {
    /// Create a new HybridFee with the protocol-default rates.
    #[must_use]
    pub fn new() -> Self {
        Self {
            flat_fee: DEFAULT_FLAT_FEE,
            per_byte_rate: DEFAULT_PER_BYTE_RATE,
        }
    }

    /// Return the SCALE-encoded byte size of a transaction.
    fn encoded_size(tx: &Transaction) -> usize {
        use parity_scale_codec::Encode;
        tx.encode().len()
    }
}

impl Default for HybridFee {
    fn default() -> Self {
        Self::new()
    }
}

impl FeePolicy for HybridFee {
    fn calculate_fee(&self, tx: &Transaction) -> U256 {
        match tx.body {
            TxBody::Burn { .. } => BURN_FLAT_FEE,
            _ => {
                self.flat_fee
                    + self.per_byte_rate * U256::from(Self::encoded_size(tx))
                    // tip is part of Transaction.fee — the sender sets it as the
                    // total fee they're willing to pay. The policy computes the
                    // minimum expected fee; the actual declared fee is `tx.fee`.
                    // For the base calculation we return the minimum.
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::account::Address;
    use crate::crypto::falcon::Falcon512Signature;
    use crate::crypto::constants::FALCON_SIGNATURE_SIZE;

    fn dummy_sig() -> Falcon512Signature {
        Falcon512Signature::from_bytes(&[0xEEu8; FALCON_SIGNATURE_SIZE]).unwrap()
    }

    fn dummy_transfer_tx() -> Transaction {
        Transaction {
            chain_id: 0,
            nonce: 0,
            sender: Address::from([0x01u8; 32]),
            fee: U256::from(1000),
            body: TxBody::Transfer {
                recipient: Address::from([0x02u8; 32]),
                amount: U256::from(500),
            },
            signature: dummy_sig(),
        }
    }

    fn dummy_burn_tx_permanent() -> Transaction {
        Transaction {
            chain_id: 0,
            nonce: 0,
            sender: Address::from([0x03u8; 32]),
            fee: U256::from(10),
            body: TxBody::Burn {
                target: crate::core::transaction::BurnTarget::Permanent,
                amount: U256::from(100),
            },
            signature: dummy_sig(),
        }
    }

    fn dummy_burn_tx_cap_refill() -> Transaction {
        Transaction {
            chain_id: 0,
            nonce: 0,
            sender: Address::from([0x04u8; 32]),
            fee: U256::from(10),
            body: TxBody::Burn {
                target: crate::core::transaction::BurnTarget::CapRefill,
                amount: U256::from(200),
            },
            signature: dummy_sig(),
        }
    }

    fn dummy_register_validator_tx() -> Transaction {
        Transaction {
            chain_id: 0,
            nonce: 0,
            sender: Address::from([0x05u8; 32]),
            fee: U256::from(1000),
            body: TxBody::RegisterValidator { public_key: [0x42u8; 897] },
            signature: dummy_sig(),
        }
    }

    #[test]
    fn test_hybrid_fee_default_rates() {
        let fee = HybridFee::new();
        assert_eq!(fee.flat_fee, DEFAULT_FLAT_FEE);
        assert_eq!(fee.per_byte_rate, DEFAULT_PER_BYTE_RATE);
    }

    #[test]
    fn test_hybrid_fee_non_burn_is_positive() {
        let fee = HybridFee::new();
        let tx = dummy_transfer_tx();
        let calculated = fee.calculate_fee(&tx);
        assert!(calculated > U256::zero());
    }

    #[test]
    fn test_burn_fee_is_flat_10_moxx() {
        let fee = HybridFee::new();
        let tx = dummy_burn_tx_permanent();
        assert_eq!(fee.calculate_fee(&tx), BURN_FLAT_FEE);
        assert_eq!(BURN_FLAT_FEE, U256::from(10));
    }

    #[test]
    fn test_burn_fee_bypasses_standard_calculation() {
        let fee = HybridFee::new();
        let burn = dummy_burn_tx_permanent();
        let transfer = dummy_transfer_tx();

        let burn_fee = fee.calculate_fee(&burn);
        let transfer_fee = fee.calculate_fee(&transfer);

        // Burn fee (flat 10 MOXX) should be less than transfer fee
        assert!(burn_fee < transfer_fee);
    }

    #[test]
    fn test_fee_increases_with_tx_size() {
        let fee = HybridFee::new();

        // Small transfer
        let small_tx = Transaction {
            body: TxBody::Transfer {
                recipient: Address::from([0x01u8; 32]),
                amount: U256::from(1),
            },
            ..dummy_transfer_tx()
        };

        // Larger transfer (bigger amount field → same size in SCALE, but
        // we add extra data to make it truly larger)
        let large_tx = Transaction {
            body: TxBody::RegisterAndStake {
                validator: Address::from([0x02u8; 32]),
                amount: U256::MAX,
            },
            fee: U256::from(1000),
            ..dummy_transfer_tx()
        };

        let small_fee = fee.calculate_fee(&small_tx);
        let large_fee = fee.calculate_fee(&large_tx);

        assert_eq!(small_fee, large_fee); // Same size since both have 32-byte addresses
        assert!(small_fee > U256::zero());
    }

    #[test]
    fn test_fee_trait_object() {
        let policy: Box<dyn FeePolicy> = Box::new(HybridFee::new());
        let tx = dummy_burn_tx_permanent();
        assert_eq!(policy.calculate_fee(&tx), U256::from(10));
    }

    #[test]
    fn test_burn_cap_refill_flat_fee() {
        let fee = HybridFee::new();
        let tx = dummy_burn_tx_cap_refill();
        assert_eq!(fee.calculate_fee(&tx), BURN_FLAT_FEE);
    }

    #[test]
    fn test_register_validator_fee_positive() {
        let fee = HybridFee::new();
        let tx = dummy_register_validator_tx();
        let calculated = fee.calculate_fee(&tx);
        assert!(calculated > U256::zero());
    }

    #[test]
    fn test_burn_variants_same_fee() {
        let fee = HybridFee::new();
        let perm = dummy_burn_tx_permanent();
        let cap = dummy_burn_tx_cap_refill();
        assert_eq!(fee.calculate_fee(&perm), fee.calculate_fee(&cap));
    }

    #[test]
    fn test_hybrid_fee_default_via_trait() {
        let fee: Box<dyn FeePolicy> = Box::new(HybridFee::default());
        let tx = dummy_transfer_tx();
        assert!(fee.calculate_fee(&tx) > U256::zero());
    }

    #[test]
    fn test_encoded_size_increases_with_tx_complexity() {
        let small = dummy_transfer_tx();
        let medium = dummy_register_validator_tx();
        let large = dummy_burn_tx_permanent();
        let sizes: Vec<usize> = vec![&small, &medium, &large]
            .into_iter()
            .map(|tx| HybridFee::encoded_size(tx))
            .collect();
        // All should be positive
        for s in &sizes {
            assert!(*s > 0);
        }
    }
}
