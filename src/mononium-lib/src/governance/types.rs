//! Governance data types.
//!
//! All types support SCALE encoding (wire format) and JSON (RPC).

use parity_scale_codec::{Decode, Encode};
use primitive_types::U256;
use serde::{Deserialize, Serialize};

use crate::core::account::Address;

// ---------------------------------------------------------------------------
// GovernanceParam — the 10 mutable protocol parameters
// ---------------------------------------------------------------------------

/// All protocol parameters that can be changed via governance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Encode, Decode, Serialize, Deserialize)]
pub enum GovernanceParam {
    #[codec(index = 0)]
    MaxValidators,
    #[codec(index = 1)]
    EraLength,
    #[codec(index = 2)]
    BlockSizeCapBytes,
    #[codec(index = 3)]
    BlockTxCap,
    #[codec(index = 4)]
    FlatFee,
    #[codec(index = 5)]
    PerByteRate,
    #[codec(index = 6)]
    AntiSpamDeposit,
    #[codec(index = 7)]
    MissedSlotPenalty,
    #[codec(index = 8)]
    SupplyCeilingRate,
    #[codec(index = 9)]
    SupplyHeadroomRate,
}

// ---------------------------------------------------------------------------
// GovernanceAction
// ---------------------------------------------------------------------------

/// An action that a proposal can execute if approved.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum GovernanceAction {
    #[codec(index = 0)]
    UpdateParam {
        param: GovernanceParam,
        new_value: U256,
    },
    #[codec(index = 1)]
    IncreaseShards { new_count: u16, effective_era: u64 },
}

// ---------------------------------------------------------------------------
// ProposalStatus
// ---------------------------------------------------------------------------

/// Life-cycle status of a governance proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Encode, Decode, Serialize, Deserialize)]
pub enum ProposalStatus {
    #[codec(index = 0)]
    Active,
    #[codec(index = 1)]
    Approved,
    #[codec(index = 2)]
    Rejected,
    #[codec(index = 3)]
    Expired,
    #[codec(index = 4)]
    Cancelled,
}

// ---------------------------------------------------------------------------
// Proposal
// ---------------------------------------------------------------------------

/// A governance proposal submitted by a staked account.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct Proposal {
    /// Unique identifier: `BLAKE3(proposer || nonce || title)`.
    pub proposal_id: [u8; 32],
    /// Address of the proposer.
    pub proposer: Address,
    /// Short title (max 256 bytes).
    pub title: Vec<u8>,
    /// Longer description (max 4 096 bytes).
    pub description: Vec<u8>,
    /// One or more actions to execute if approved.
    pub actions: Vec<GovernanceAction>,
    /// Deposit held while the proposal is active.
    pub deposit: U256,
    /// Era in which this proposal was submitted.
    pub submission_era: u64,
    /// Current status.
    pub status: ProposalStatus,
}

// ---------------------------------------------------------------------------
// Vote
// ---------------------------------------------------------------------------

/// A single vote on a governance proposal.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct Vote {
    /// The proposal being voted on.
    pub proposal_id: [u8; 32],
    /// Address of the voting account.
    pub voter: Address,
    /// Approve (true) or reject (false).
    pub approve: bool,
    /// Stake weight at the time of voting (snapshotted).
    pub weight: U256,
    /// Block height when the vote was cast.
    pub block_height: u64,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // GovernanceParam SCALE roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_governance_param_scale_roundtrip_all() {
        let variants = vec![
            GovernanceParam::MaxValidators,
            GovernanceParam::EraLength,
            GovernanceParam::BlockSizeCapBytes,
            GovernanceParam::BlockTxCap,
            GovernanceParam::FlatFee,
            GovernanceParam::PerByteRate,
            GovernanceParam::AntiSpamDeposit,
            GovernanceParam::MissedSlotPenalty,
            GovernanceParam::SupplyCeilingRate,
            GovernanceParam::SupplyHeadroomRate,
        ];
        for v in &variants {
            let encoded = v.encode();
            let decoded = GovernanceParam::decode(&mut &encoded[..]).unwrap();
            assert_eq!(*v, decoded);
        }
    }

    // -----------------------------------------------------------------------
    // GovernanceAction SCALE roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_governance_action_update_param_roundtrip() {
        let action = GovernanceAction::UpdateParam {
            param: GovernanceParam::MaxValidators,
            new_value: U256::from(200),
        };
        let encoded = action.encode();
        let decoded = GovernanceAction::decode(&mut &encoded[..]).unwrap();
        assert_eq!(action, decoded);
    }

    #[test]
    fn test_governance_action_increase_shards_roundtrip() {
        let action = GovernanceAction::IncreaseShards {
            new_count: 4,
            effective_era: 100,
        };
        let encoded = action.encode();
        let decoded = GovernanceAction::decode(&mut &encoded[..]).unwrap();
        assert_eq!(action, decoded);
    }

    // -----------------------------------------------------------------------
    // ProposalStatus SCALE roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_proposal_status_scale_roundtrip_all() {
        for status in &[
            ProposalStatus::Active,
            ProposalStatus::Approved,
            ProposalStatus::Rejected,
            ProposalStatus::Expired,
            ProposalStatus::Cancelled,
        ] {
            let encoded = status.encode();
            let decoded = ProposalStatus::decode(&mut &encoded[..]).unwrap();
            assert_eq!(*status, decoded);
        }
    }

    // -----------------------------------------------------------------------
    // Proposal SCALE roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_proposal_scale_roundtrip() {
        let p = Proposal {
            proposal_id: [0xAA; 32],
            proposer: Address::from([0xBB; 32]),
            title: b"Test Proposal".to_vec(),
            description: b"Description".to_vec(),
            actions: vec![GovernanceAction::UpdateParam {
                param: GovernanceParam::EraLength,
                new_value: U256::from(360),
            }],
            deposit: U256::from(100),
            submission_era: 5,
            status: ProposalStatus::Active,
        };
        let encoded = p.encode();
        let decoded = Proposal::decode(&mut &encoded[..]).unwrap();
        assert_eq!(p, decoded);
    }

    // -----------------------------------------------------------------------
    // Vote SCALE roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_vote_scale_roundtrip() {
        let v = Vote {
            proposal_id: [0xCC; 32],
            voter: Address::from([0xDD; 32]),
            approve: true,
            weight: U256::from(500),
            block_height: 42,
        };
        let encoded = v.encode();
        let decoded = Vote::decode(&mut &encoded[..]).unwrap();
        assert_eq!(v, decoded);
    }

    // -----------------------------------------------------------------------
    // JSON roundtrip
    // -----------------------------------------------------------------------

    #[test]
    fn test_proposal_json_roundtrip() {
        let p = Proposal {
            proposal_id: [0xAA; 32],
            proposer: Address::from([0xBB; 32]),
            title: b"JSON Test".to_vec(),
            description: b"Desc".to_vec(),
            actions: vec![GovernanceAction::IncreaseShards {
                new_count: 8,
                effective_era: 50,
            }],
            deposit: U256::from(200),
            submission_era: 3,
            status: ProposalStatus::Approved,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: Proposal = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn test_vote_json_roundtrip() {
        let v = Vote {
            proposal_id: [0xCC; 32],
            voter: Address::from([0xDD; 32]),
            approve: false,
            weight: U256::from(1000),
            block_height: 99,
        };
        let json = serde_json::to_string(&v).unwrap();
        let back: Vote = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }
}
