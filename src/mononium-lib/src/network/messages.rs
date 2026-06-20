//! Gossip message types (SCALE-encoded for wire transport).
//!
//! All messages sent over gossipsub topics are wrapped in [`GossipMessage`].
//! Variants correspond to the four standard topics: txs, blocks, votes,
//! and evidence.

use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::core::block::{Block, BlockHeader, CommitVote};
use crate::core::transaction::Transaction;

// ---------------------------------------------------------------------------
// Serde helper for Falcon-512 signature (666 bytes)
// ---------------------------------------------------------------------------

mod sig_serde {
    use serde::{Deserialize, Deserializer, Serializer, de::Error as _};

    pub fn serialize<S: Serializer>(key: &[u8; 666], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(key))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 666], D::Error> {
        let s = String::deserialize(deserializer)?;
        let bytes = hex::decode(&s).map_err(D::Error::custom)?;
        if bytes.len() != 666 {
            return Err(D::Error::custom("expected 666 bytes"));
        }
        let mut arr = [0u8; 666];
        arr.copy_from_slice(&bytes);
        Ok(arr)
    }
}

/// Equivocation evidence — two signed blocks at the same height/parent.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct EquivocationEvidence {
    pub header_a: BlockHeader,
    #[serde(with = "sig_serde")]
    pub signature_a: [u8; 666],
    pub header_b: BlockHeader,
    #[serde(with = "sig_serde")]
    pub signature_b: [u8; 666],
    pub proposer: [u8; 32],
}

/// Unified gossip message — one per gossipsub topic.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum GossipMessage {
    #[codec(index = 0)]
    Txs(Vec<Transaction>),
    #[codec(index = 1)]
    Block(Box<Block>),
    #[codec(index = 2)]
    Vote(CommitVote),
    #[codec(index = 3)]
    Evidence(Box<EquivocationEvidence>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::CommitVote;

    fn dummy_tx() -> Transaction {
        Transaction {
            chain_id: 0,
            nonce: 0,
            sender: crate::core::account::Address::from([0x11u8; 32]),
            fee: primitive_types::U256::from(100),
            body: crate::core::transaction::TxBody::Transfer {
                recipient: crate::core::account::Address::from([0x22u8; 32]),
                amount: primitive_types::U256::from(500),
            },
            signature: crate::crypto::falcon::Falcon512Signature::from_bytes(
                &[0xABu8; crate::crypto::constants::FALCON_SIGNATURE_SIZE],
            )
            .unwrap(),
        }
    }

    fn dummy_commit_vote() -> CommitVote {
        CommitVote {
            height: 1,
            block_hash: [0xCCu8; 32],
            validator: crate::core::account::Address::from([0xDDu8; 32]),
            signature: crate::crypto::falcon::Falcon512Signature::from_bytes(
                &[0xEEu8; crate::crypto::constants::FALCON_SIGNATURE_SIZE],
            )
            .unwrap(),
        }
    }

    #[test]
    fn test_gossip_txs_scale_roundtrip() {
        let msg = GossipMessage::Txs(vec![dummy_tx()]);
        let encoded = msg.encode();
        let decoded = GossipMessage::decode(&mut &encoded[..]).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_gossip_txs_json_roundtrip() {
        let msg = GossipMessage::Txs(vec![dummy_tx()]);
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: GossipMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_gossip_block_scale_roundtrip() {
        let block = Block {
            header: crate::core::block::BlockHeader {
                height: 1,
                parent_hash: [0u8; 32],
                global_state_root: [0u8; 32],
                tx_root: [0u8; 32],
                timestamp: 1_700_000_000,
                proposer: crate::core::account::Address::from([0xEEu8; 32]),
                chain_id: 0,
            },
            body: crate::core::block::BlockBody { transactions: vec![dummy_tx()] },
        };
        let msg = GossipMessage::Block(Box::new(block));
        let encoded = msg.encode();
        let decoded = GossipMessage::decode(&mut &encoded[..]).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_gossip_vote_scale_roundtrip() {
        let msg = GossipMessage::Vote(dummy_commit_vote());
        let encoded = msg.encode();
        let decoded = GossipMessage::decode(&mut &encoded[..]).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn test_gossip_vote_json_roundtrip() {
        let msg = GossipMessage::Vote(dummy_commit_vote());
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: GossipMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, decoded);
    }
}
