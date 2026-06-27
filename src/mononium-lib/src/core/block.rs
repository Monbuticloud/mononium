//! Block types: Block, BlockHeader, BlockBody, CommitVote.
//!
//! All types support both SCALE (wire) and JSON (RPC) encoding.
//!
//! **Votes** are not included in the block body. They are gossipped and
//! stored independently in `block_votes`.

use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

use crate::core::account::Address;
use crate::core::transaction::Transaction;

// ---------------------------------------------------------------------------
// BlockHeader
// ---------------------------------------------------------------------------

/// Header of a block.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct BlockHeader {
    /// Block height (genesis = 0).
    pub height: u64,
    /// BLAKE3 hash of the parent block header.
    pub parent_hash: [u8; 32],
    /// Merkle root of per-shard SMT roots.
    pub global_state_root: [u8; 32],
    /// BLAKE3 Merkle root of all transaction hashes in this block.
    pub tx_root: [u8; 32],
    /// Unix timestamp (seconds) when the block was proposed.
    pub timestamp: u64,
    /// Address of the proposing validator.
    pub proposer: Address,
    /// Network identifier (prevents replay across networks).
    pub chain_id: u64,
    /// Falcon-512 signature by the proposer over SCALE-encoded header fields.
    pub proposer_signature: crate::crypto::falcon::Falcon512Signature,
}

// ---------------------------------------------------------------------------
// BlockBody
// ---------------------------------------------------------------------------

/// The body of a block containing ordered transactions.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct BlockBody {
    /// Transactions in execution order.
    pub transactions: Vec<Transaction>,
}

// ---------------------------------------------------------------------------
// Block
// ---------------------------------------------------------------------------

/// A complete block: header + body.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct Block {
    /// Block header.
    pub header: BlockHeader,
    /// Block body (transactions).
    pub body: BlockBody,
}

// ---------------------------------------------------------------------------
// CommitVote
// ---------------------------------------------------------------------------

/// A validator's commit vote for a block.
///
/// Votes are **not** included in the block. They are gossipped on
/// `mononium/votes/{chain_id}` and stored in the `block_votes` database.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct CommitVote {
    /// The height being voted on.
    pub height: u64,
    /// BLAKE3 hash of the block being voted for.
    pub block_hash: [u8; 32],
    /// Address of the voting validator.
    pub validator: Address,
    /// Falcon-512 signature over SCALE(height || block_hash).
    pub signature: crate::crypto::falcon::Falcon512Signature,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Helper: minimal block header with a dummy proposer signature.
/// Override specific fields with `..` syntax in tests.
#[cfg(test)]
#[must_use]
pub fn test_header() -> BlockHeader {
    use crate::crypto::constants::FALCON_SIGNATURE_SIZE;
    use crate::crypto::falcon::Falcon512Signature;
    let dummy_sig = Falcon512Signature::from_bytes(&[0xCDu8; FALCON_SIGNATURE_SIZE]).unwrap();
    BlockHeader {
        height: 0,
        parent_hash: [0u8; 32],
        global_state_root: [0u8; 32],
        tx_root: [0u8; 32],
        timestamp: 0,
        proposer: Address::from([0u8; 32]),
        chain_id: 0,
        proposer_signature: dummy_sig,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::constants::FALCON_SIGNATURE_SIZE;
    use crate::crypto::falcon::Falcon512Signature;
    use primitive_types::U256;

    fn dummy_sig() -> Falcon512Signature {
        Falcon512Signature::from_bytes(&[0xCDu8; FALCON_SIGNATURE_SIZE]).unwrap()
    }

    #[test]
    fn test_block_header_scale_roundtrip() {
        let h = BlockHeader {
            height: 42,
            parent_hash: [0xAAu8; 32],
            global_state_root: [0xBBu8; 32],
            tx_root: [0xCCu8; 32],
            timestamp: 1_700_000_000,
            proposer: Address::from([0xABu8; 32]),
            chain_id: 0,
            proposer_signature: dummy_sig(),
        };
        let encoded = h.encode();
        let decoded = BlockHeader::decode(&mut &encoded[..]).unwrap();
        assert_eq!(h, decoded);
    }

    #[test]
    fn test_block_header_json_roundtrip() {
        let h = BlockHeader {
            height: 99,
            parent_hash: [0xAAu8; 32],
            global_state_root: [0xBBu8; 32],
            tx_root: [0xCCu8; 32],
            timestamp: 1_700_000_001,
            proposer: Address::from([0xABu8; 32]),
            chain_id: 1,
            proposer_signature: dummy_sig(),
        };
        let json = serde_json::to_string(&h).unwrap();
        let decoded: BlockHeader = serde_json::from_str(&json).unwrap();
        assert_eq!(h, decoded);
    }

    #[test]
    fn test_block_scale_roundtrip() {
        let block = Block {
            header: BlockHeader {
                height: 1,
                parent_hash: [0x01u8; 32],
                global_state_root: [0x02u8; 32],
                tx_root: [0x03u8; 32],
                timestamp: 1_700_000_000,
                proposer: Address::from([0x04u8; 32]),
                chain_id: 0,
                proposer_signature: dummy_sig(),
            },
            body: BlockBody {
                transactions: vec![Transaction {
                    chain_id: 0,
                    nonce: 0,
                    sender: Address::from([0x05u8; 32]),
                    fee: U256::from(100),
                    body: crate::core::transaction::TxBody::Transfer {
                        recipient: Address::from([0x06u8; 32]),
                        amount: U256::from(500),
                    },
                    signature: dummy_sig(),
                }],
            },
        };
        let encoded = block.encode();
        let decoded = Block::decode(&mut &encoded[..]).unwrap();
        assert_eq!(block, decoded);
    }

    #[test]
    fn test_commit_vote_scale_roundtrip() {
        let vote = CommitVote {
            height: 42,
            block_hash: [0xAAu8; 32],
            validator: Address::from([0xBBu8; 32]),
            signature: dummy_sig(),
        };
        let encoded = vote.encode();
        let decoded = CommitVote::decode(&mut &encoded[..]).unwrap();
        assert_eq!(vote, decoded);
    }

    #[test]
    fn test_commit_vote_json_roundtrip() {
        let vote = CommitVote {
            height: 99,
            block_hash: [0xCCu8; 32],
            validator: Address::from([0xDDu8; 32]),
            signature: dummy_sig(),
        };
        let json = serde_json::to_string(&vote).unwrap();
        let decoded: CommitVote = serde_json::from_str(&json).unwrap();
        assert_eq!(vote, decoded);
    }

    #[test]
    fn test_block_height_determines_encoding() {
        let b1 = Block {
            header: BlockHeader {
                height: 1,
                parent_hash: [0u8; 32],
                global_state_root: [0u8; 32],
                tx_root: [0u8; 32],
                timestamp: 0,
                proposer: Address::from([0u8; 32]),
                chain_id: 0,
                proposer_signature: dummy_sig(),
            },
            body: BlockBody {
                transactions: vec![],
            },
        };
        let b2 = Block {
            header: BlockHeader {
                height: 2,
                ..b1.header.clone()
            },
            body: BlockBody {
                transactions: vec![],
            },
        };
        assert_ne!(b1.encode(), b2.encode());
    }

    #[test]
    fn test_block_json_roundtrip() {
        let block = Block {
            header: BlockHeader {
                height: 7,
                parent_hash: [0x07u8; 32],
                global_state_root: [0x08u8; 32],
                tx_root: [0x09u8; 32],
                timestamp: 1_700_000_005,
                proposer: Address::from([0x0Au8; 32]),
                chain_id: 1,
                proposer_signature: dummy_sig(),
            },
            body: BlockBody {
                transactions: vec![],
            },
        };
        let json = serde_json::to_string(&block).unwrap();
        let decoded: Block = serde_json::from_str(&json).unwrap();
        assert_eq!(block, decoded);
    }

    #[test]
    fn test_block_body_empty_roundtrip() {
        let body = BlockBody {
            transactions: vec![],
        };
        let encoded = body.encode();
        let decoded = BlockBody::decode(&mut &encoded[..]).unwrap();
        assert_eq!(body, decoded);
    }

    #[test]
    fn test_block_body_json_roundtrip() {
        let body = BlockBody {
            transactions: vec![],
        };
        let json = serde_json::to_string(&body).unwrap();
        let decoded: BlockBody = serde_json::from_str(&json).unwrap();
        assert_eq!(body, decoded);
    }
}
