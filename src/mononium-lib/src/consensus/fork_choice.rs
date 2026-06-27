//! Fork-choice rule: heaviest chain by unique proposer stake weight.
//!
//! Per ADR-003 (DI pattern), VRF-based fork choice can be swapped in later.
//! For Phase 2, the canonical chain is the one with the greatest total
//! unique-proposer backing stake.

use std::collections::{HashMap, HashSet};

use primitive_types::U256;

use crate::core::account::Address;
use crate::core::block::Block;

// ---------------------------------------------------------------------------
// ForkChoice
// ---------------------------------------------------------------------------

/// Outcome of a fork-choice comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForkChoiceResult {
    /// Chain A is heavier (prefer A).
    PreferA,
    /// Chain B is heavier (prefer B).
    PreferB,
    /// Equal weight — keep existing canonical chain.
    Equal,
}

/// Heaviest-chain fork-choice rule.
pub struct ForkChoice;

impl ForkChoice {
    /// Compare two chains and return the preferred one.
    ///
    /// `stake_weights` maps proposer addresses to their active stake.
    /// The chain with the greater `total_stake_backing` wins.
    /// Equal weight → `Equal` (keep existing canonical).
    #[must_use]
    pub fn select_canonical(
        chain_a: &[Block],
        chain_b: &[Block],
        stake_weights: &HashMap<Address, U256>,
    ) -> ForkChoiceResult {
        let weight_a = Self::total_stake_backing(chain_a, stake_weights);
        let weight_b = Self::total_stake_backing(chain_b, stake_weights);

        if weight_a > weight_b {
            ForkChoiceResult::PreferA
        } else if weight_b > weight_a {
            ForkChoiceResult::PreferB
        } else {
            ForkChoiceResult::Equal
        }
    }

    /// Sum the active stake of every **unique proposer** in the chain.
    ///
    /// A proposer that appears multiple times is counted only once
    /// (prevents a single high-stake validator from dominating the weight
    /// by proposing many blocks).
    #[must_use]
    pub fn total_stake_backing(chain: &[Block], stake_weights: &HashMap<Address, U256>) -> U256 {
        let unique_proposers: HashSet<&Address> =
            chain.iter().map(|b| &b.header.proposer).collect();

        unique_proposers
            .iter()
            .filter_map(|addr| stake_weights.get(addr))
            .fold(U256::zero(), |acc, w| acc + w)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::BlockBody;
    use crate::core::block::BlockHeader;
    use crate::crypto::constants::FALCON_SIGNATURE_SIZE;
    use crate::crypto::falcon::Falcon512Signature;

    fn addr(b: u8) -> Address {
        Address::from([b; 32])
    }

    fn dummy_sig() -> Falcon512Signature {
        Falcon512Signature::from_bytes(&[0xCDu8; FALCON_SIGNATURE_SIZE]).unwrap()
    }

    fn block(proposer: Address) -> Block {
        Block {
            header: BlockHeader {
                height: 1,
                parent_hash: [0; 32],
                global_state_root: [0; 32],
                tx_root: [0; 32],
                timestamp: 1_700_000_000,
                proposer,
                chain_id: 0,
                proposer_signature: dummy_sig(),
            },
            body: BlockBody {
                transactions: vec![],
            },
        }
    }

    fn hashmap(data: &[(u8, u64)]) -> HashMap<Address, U256> {
        data.iter()
            .map(|&(b, w)| (addr(b), U256::from(w)))
            .collect()
    }

    #[test]
    fn test_heavier_chain_wins() {
        // A: proposers (1, 2, 3) each with stake 100  →  total = 300
        // B: proposers (4, 5)             each with stake 200  →  total = 400
        let chain_a = vec![block(addr(1)), block(addr(2)), block(addr(3))];
        let chain_b = vec![block(addr(4)), block(addr(5))];
        let stakes = hashmap(&[(1, 100), (2, 100), (3, 100), (4, 200), (5, 200)]);

        assert_eq!(
            ForkChoice::select_canonical(&chain_a, &chain_b, &stakes),
            ForkChoiceResult::PreferB
        );
    }

    #[test]
    fn test_lighter_chain_loses() {
        let chain_a = vec![block(addr(1)), block(addr(2))];
        let chain_b = vec![block(addr(3))];
        let stakes = hashmap(&[(1, 100), (2, 100), (3, 100)]);

        assert_eq!(
            ForkChoice::select_canonical(&chain_a, &chain_b, &stakes),
            ForkChoiceResult::PreferA
        );
    }

    #[test]
    fn test_equal_weight_returns_equal() {
        let chain_a = vec![block(addr(1))];
        let chain_b = vec![block(addr(2))];
        let stakes = hashmap(&[(1, 100), (2, 100)]);

        assert_eq!(
            ForkChoice::select_canonical(&chain_a, &chain_b, &stakes),
            ForkChoiceResult::Equal
        );
    }

    #[test]
    fn test_unique_proposer_not_duplicate() {
        // Same proposer 10 times → counts once
        let chain_a = vec![block(addr(1)); 10];
        let chain_b = vec![block(addr(2)); 1];
        let stakes = hashmap(&[(1, 50), (2, 100)]);

        // chain_a unique weight = 50, chain_b = 100
        assert_eq!(
            ForkChoice::select_canonical(&chain_a, &chain_b, &stakes),
            ForkChoiceResult::PreferB
        );
    }

    #[test]
    fn test_unknown_proposer_has_zero_weight() {
        let chain = vec![block(addr(1)), block(addr(99))];
        let stakes = hashmap(&[(1, 100)]);
        // addr(99) has no stake → weight is just 100 (unique count)
        assert_eq!(
            ForkChoice::total_stake_backing(&chain, &stakes),
            U256::from(100)
        );
    }

    #[test]
    fn test_empty_chain_returns_zero() {
        let stakes = hashmap(&[(1, 100)]);
        assert_eq!(ForkChoice::total_stake_backing(&[], &stakes), U256::zero());
    }

    #[test]
    fn test_empty_stakes_returns_zero() {
        let chain = vec![block(addr(1))];
        let stakes = HashMap::new();
        assert_eq!(
            ForkChoice::total_stake_backing(&chain, &stakes),
            U256::zero()
        );
    }
}
