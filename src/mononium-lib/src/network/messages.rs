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

// ---------------------------------------------------------------------------
// Sync protocol message types (libp2p Request-Response)
// ---------------------------------------------------------------------------

/// Direction for block sync requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum SyncDirection {
    /// Sync forward from `start_height` toward tip.
    #[codec(index = 0)]
    Forward,
    /// Sync backward from `start_height` toward genesis.
    #[codec(index = 1)]
    Backward,
}

/// Request a batch of blocks via the sync protocol.
///
/// - `max_blocks` must be in [1, 500].
/// - `known_block_hash` anchors the request: the responder MUST verify
///   the anchor hash matches the block at `start_height - 1`. If it does
///   not match, the response is empty (peer is on a different fork).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct BlockSyncRequest {
    pub start_height: u64,
    /// Max blocks to return (capped at 500 by protocol).
    pub max_blocks: u16,
    pub direction: SyncDirection,
    /// Optional anchor: BLAKE3 hash of block at `start_height - 1`.
    pub known_block_hash: Option<[u8; 32]>,
}

/// Response to a [`BlockSyncRequest`].
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct BlockSyncResponse {
    /// Blocks in order (may be empty if anchor mismatch or no blocks).
    pub blocks: Vec<Block>,
    /// Responder's highest known block height.
    pub highest_height: u64,
    /// Rolling BLAKE3 batch hash over all response blocks (per ADR-018).
    pub batch_hash: [u8; 32],
}

/// Request specific blocks by their BLAKE3 hashes (max 100 hashes).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct BlockByHashRequest {
    pub block_hashes: Vec<[u8; 32]>,
}

/// Response to a [`BlockByHashRequest`].
///
/// Blocks are returned in request order. Missing blocks are omitted
/// (caller infers absence from position).
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct BlockByHashResponse {
    pub blocks: Vec<Block>,
}

// ---------------------------------------------------------------------------
// Batch hash: rolling BLAKE3 over block sequence (ADR-018)
// ---------------------------------------------------------------------------

/// Compute a rolling BLAKE3 batch hash over a sequence of blocks.
///
/// Per ADR-018: `batch_hash = H(prev_hash || encode(block_1) || ... )`
/// where `prev_hash` is the genesis header hash (or the anchor at the
/// start of the range).
///
/// For an empty block list, returns `genesis_hash` unchanged.
#[must_use]
pub fn compute_batch_hash(genesis_hash: &[u8; 32], blocks: &[Block]) -> [u8; 32] {
    if blocks.is_empty() {
        return *genesis_hash;
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(genesis_hash);
    for block in blocks {
        let encoded = parity_scale_codec::Encode::encode(block);
        hasher.update(&encoded);
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(hasher.finalize().as_bytes());
    hash
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// Maximum blocks allowed in a single sync request/response batch.
pub const MAX_SYNC_BLOCKS: u16 = 500;

/// Maximum block hashes allowed in a single `BlockByHashRequest`.
pub const MAX_BY_HASH_BLOCKS: usize = 100;

/// Validate a `BlockSyncRequest`'s limits.
///
/// # Errors
///
/// Returns an error message if `max_blocks` is 0 or exceeds 500.
pub fn validate_sync_request(req: &BlockSyncRequest) -> Result<(), String> {
    if req.max_blocks == 0 || req.max_blocks > MAX_SYNC_BLOCKS {
        return Err(format!(
            "max_blocks must be 1..={MAX_SYNC_BLOCKS}, got {}",
            req.max_blocks,
        ));
    }
    Ok(())
}

/// Validate a `BlockByHashRequest`'s limits.
///
/// # Errors
///
/// Returns an error message if `block_hashes` is empty or exceeds 100.
pub fn validate_by_hash_request(req: &BlockByHashRequest) -> Result<(), String> {
    if req.block_hashes.is_empty() || req.block_hashes.len() > MAX_BY_HASH_BLOCKS {
        return Err(format!(
            "block_hashes must be 1..={MAX_BY_HASH_BLOCKS}, got {}",
            req.block_hashes.len(),
        ));
    }
    Ok(())
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
    use crate::core::block::{BlockBody, BlockHeader};
    use crate::core::account::Address;
    use proptest::prelude::*;

    fn dummy_block() -> Block {
        Block {
            header: BlockHeader {
                height: 1,
                parent_hash: [0; 32],
                global_state_root: [0; 32],
                tx_root: [0; 32],
                timestamp: 1_700_000_000,
                proposer: Address::from([0x01; 32]),
                chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![] },
        }
    }

    // -----------------------------------------------------------------------
    // SyncDirection
    // -----------------------------------------------------------------------

    #[test]
    fn test_sync_direction_scale_roundtrip() {
        for dir in &[SyncDirection::Forward, SyncDirection::Backward] {
            let encoded = dir.encode();
            let decoded = SyncDirection::decode(&mut &encoded[..]).unwrap();
            assert_eq!(*dir, decoded);
        }
    }

    // -----------------------------------------------------------------------
    // BlockSyncRequest
    // -----------------------------------------------------------------------

    #[test]
    fn test_block_sync_request_scale_roundtrip() {
        let req = BlockSyncRequest {
            start_height: 100,
            max_blocks: 50,
            direction: SyncDirection::Forward,
            known_block_hash: Some([0xAB; 32]),
        };
        let encoded = req.encode();
        let decoded = BlockSyncRequest::decode(&mut &encoded[..]).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_block_sync_request_no_anchor_roundtrip() {
        let req = BlockSyncRequest {
            start_height: 0,
            max_blocks: 1,
            direction: SyncDirection::Backward,
            known_block_hash: None,
        };
        let encoded = req.encode();
        let decoded = BlockSyncRequest::decode(&mut &encoded[..]).unwrap();
        assert_eq!(req, decoded);
    }

    // -----------------------------------------------------------------------
    // BlockSyncResponse
    // -----------------------------------------------------------------------

    #[test]
    fn test_block_sync_response_scale_roundtrip() {
        let resp = BlockSyncResponse {
            blocks: vec![dummy_block()],
            highest_height: 500,
            batch_hash: [0xCD; 32],
        };
        let encoded = resp.encode();
        let decoded = BlockSyncResponse::decode(&mut &encoded[..]).unwrap();
        assert_eq!(resp, decoded);
    }

    #[test]
    fn test_block_sync_response_empty_blocks_roundtrip() {
        let resp = BlockSyncResponse {
            blocks: vec![],
            highest_height: 0,
            batch_hash: [0; 32],
        };
        let encoded = resp.encode();
        let decoded = BlockSyncResponse::decode(&mut &encoded[..]).unwrap();
        assert_eq!(resp, decoded);
    }

    // -----------------------------------------------------------------------
    // BlockByHashRequest / Response
    // -----------------------------------------------------------------------

    #[test]
    fn test_block_by_hash_request_scale_roundtrip() {
        let req = BlockByHashRequest {
            block_hashes: vec![[0xAA; 32], [0xBB; 32]],
        };
        let encoded = req.encode();
        let decoded = BlockByHashRequest::decode(&mut &encoded[..]).unwrap();
        assert_eq!(req, decoded);
    }

    #[test]
    fn test_block_by_hash_response_scale_roundtrip() {
        let resp = BlockByHashResponse {
            blocks: vec![dummy_block()],
        };
        let encoded = resp.encode();
        let decoded = BlockByHashResponse::decode(&mut &encoded[..]).unwrap();
        assert_eq!(resp, decoded);
    }

    // -----------------------------------------------------------------------
    // EquivocationEvidence — JSON serde (covers custom sig_serde for [u8; 666])
    // -----------------------------------------------------------------------

    #[test]
    fn test_equivocation_evidence_json_roundtrip() {
        let evidence = EquivocationEvidence {
            header_a: BlockHeader {
                height: 1,
                parent_hash: [0xAAu8; 32],
                global_state_root: [0xBBu8; 32],
                tx_root: [0xCCu8; 32],
                timestamp: 100,
                proposer: Address::from([0xDDu8; 32]),
                chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(
                    &[0xEEu8; crate::crypto::constants::FALCON_SIGNATURE_SIZE],
                ).unwrap(),
            },
            signature_a: [0x11u8; 666],
            header_b: BlockHeader {
                height: 1,
                parent_hash: [0xAAu8; 32],
                global_state_root: [0xBBu8; 32],
                tx_root: [0xCCu8; 32],
                timestamp: 101,
                proposer: Address::from([0xDDu8; 32]),
                chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(
                    &[0xEEu8; crate::crypto::constants::FALCON_SIGNATURE_SIZE],
                ).unwrap(),
            },
            signature_b: [0x22u8; 666],
            proposer: [0xFFu8; 32],
        };
        let json = serde_json::to_string(&evidence).unwrap();
        let decoded: EquivocationEvidence = serde_json::from_str(&json).unwrap();
        assert_eq!(evidence, decoded);
    }

    #[test]
    fn test_equivocation_evidence_json_invalid_sig_length() {
        // Build a valid serialization then swap in wrong-length sig hex
        let evidence = EquivocationEvidence {
            header_a: BlockHeader {
                height: 1, parent_hash: [0; 32], global_state_root: [0; 32],
                tx_root: [0; 32], timestamp: 0, proposer: Address::from([0; 32]),
                chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(
                    &[0; crate::crypto::constants::FALCON_SIGNATURE_SIZE],
                ).unwrap(),
            },
            signature_a: [0x11u8; 666],
            header_b: BlockHeader {
                height: 1, parent_hash: [0; 32], global_state_root: [0; 32],
                tx_root: [0; 32], timestamp: 0, proposer: Address::from([0; 32]),
                chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(
                    &[0; crate::crypto::constants::FALCON_SIGNATURE_SIZE],
                ).unwrap(),
            },
            signature_b: [0x22u8; 666],
            proposer: [0xFFu8; 32],
        };
        let mut json = serde_json::to_value(&evidence).unwrap();
        // Replace signature_a with 2 bytes (should be 666)
        json["signature_a"] = serde_json::Value::String("ab".to_string());
        let result: std::result::Result<EquivocationEvidence, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_equivocation_evidence_json_invalid_hex() {
        let evidence = EquivocationEvidence {
            header_a: BlockHeader {
                height: 1, parent_hash: [0; 32], global_state_root: [0; 32],
                tx_root: [0; 32], timestamp: 0, proposer: Address::from([0; 32]),
                chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(
                    &[0; crate::crypto::constants::FALCON_SIGNATURE_SIZE],
                ).unwrap(),
            },
            signature_a: [0x11u8; 666],
            header_b: BlockHeader {
                height: 1, parent_hash: [0; 32], global_state_root: [0; 32],
                tx_root: [0; 32], timestamp: 0, proposer: Address::from([0; 32]),
                chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(
                    &[0; crate::crypto::constants::FALCON_SIGNATURE_SIZE],
                ).unwrap(),
            },
            signature_b: [0x22u8; 666],
            proposer: [0xFFu8; 32],
        };
        let mut json = serde_json::to_value(&evidence).unwrap();
        json["signature_a"] = serde_json::Value::String("not-hex!".to_string());
        let result: std::result::Result<EquivocationEvidence, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_sync_request_ok() {
        let req = BlockSyncRequest {
            start_height: 1,
            max_blocks: 100,
            direction: SyncDirection::Forward,
            known_block_hash: None,
        };
        assert!(validate_sync_request(&req).is_ok());
    }

    #[test]
    fn test_validate_sync_request_zero_blocks() {
        let req = BlockSyncRequest {
            start_height: 1, max_blocks: 0, direction: SyncDirection::Forward, known_block_hash: None,
        };
        assert!(validate_sync_request(&req).is_err());
    }

    #[test]
    fn test_validate_sync_request_exceeds_max() {
        let req = BlockSyncRequest {
            start_height: 1, max_blocks: 501, direction: SyncDirection::Forward, known_block_hash: None,
        };
        assert!(validate_sync_request(&req).is_err());
    }

    #[test]
    fn test_validate_sync_request_max_allowed() {
        let req = BlockSyncRequest {
            start_height: 1, max_blocks: 500, direction: SyncDirection::Forward, known_block_hash: None,
        };
        assert!(validate_sync_request(&req).is_ok());
    }

    #[test]
    fn test_validate_by_hash_request_ok() {
        let req = BlockByHashRequest {
            block_hashes: vec![[0; 32]],
        };
        assert!(validate_by_hash_request(&req).is_ok());
    }

    #[test]
    fn test_validate_by_hash_request_empty() {
        let req = BlockByHashRequest { block_hashes: vec![] };
        assert!(validate_by_hash_request(&req).is_err());
    }

    #[test]
    fn test_validate_by_hash_request_exceeds_max() {
        let req = BlockByHashRequest {
            block_hashes: vec![[0; 32]; 101],
        };
        assert!(validate_by_hash_request(&req).is_err());
    }

    #[test]
    fn test_validate_by_hash_request_max_allowed() {
        let req = BlockByHashRequest {
            block_hashes: vec![[0; 32]; 100],
        };
        assert!(validate_by_hash_request(&req).is_ok());
    }

    // -----------------------------------------------------------------------
    // compute_batch_hash
    // -----------------------------------------------------------------------

    #[test]
    fn test_compute_batch_hash_empty_returns_genesis() {
        let genesis = [0xFE; 32];
        let hash = compute_batch_hash(&genesis, &[]);
        assert_eq!(hash, genesis);
    }

    #[test]
    fn test_compute_batch_hash_deterministic() {
        let genesis = [0x01; 32];
        let block = dummy_block();
        let hash1 = compute_batch_hash(&genesis, &[block.clone()]);
        let hash2 = compute_batch_hash(&genesis, &[block]);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_compute_batch_hash_different_height_different_hash() {
        let genesis = [0x01; 32];
        let b1 = dummy_block();
        let mut b2 = dummy_block();
        b2.header.height = 2;
        let hash1 = compute_batch_hash(&genesis, &[b1]);
        let hash2 = compute_batch_hash(&genesis, &[b2]);
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_compute_batch_hash_multi_block() {
        let genesis = [0x01; 32];
        let b1 = dummy_block();
        let mut b2 = dummy_block();
        b2.header.height = 2;
        let hash1 = compute_batch_hash(&genesis, &[b1.clone(), b2.clone()]);
        let hash2 = compute_batch_hash(&genesis, &[b1, b2]);
        assert_eq!(hash1, hash2);
    }

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
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
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

    // ── Property-based tests ───────────────────────────────────────

    proptest! {
        #[test]
        fn proptest_compute_batch_hash_deterministic(
            genesis_byte in any::<u8>(),
        ) {
            let genesis = [genesis_byte; 32];
            let blocks = vec![dummy_block()];
            let h1 = compute_batch_hash(&genesis, &blocks);
            let h2 = compute_batch_hash(&genesis, &blocks);
            assert_eq!(h1, h2);
        }

        #[test]
        fn proptest_compute_batch_hash_empty_returns_genesis(
            genesis_byte in any::<u8>(),
        ) {
            let genesis = [genesis_byte; 32];
            let hash = compute_batch_hash(&genesis, &[]);
            assert_eq!(hash, genesis);
        }

        #[test]
        fn proptest_compute_batch_hash_differs_for_different_blocks(
            genesis_byte in any::<u8>(),
        ) {
            let genesis = [genesis_byte; 32];
            let mut b1 = dummy_block();
            b1.header.height = 1;
            let mut b2 = dummy_block();
            b2.header.height = 2;
            let h1 = compute_batch_hash(&genesis, &[b1]);
            let h2 = compute_batch_hash(&genesis, &[b2]);
            assert_ne!(h1, h2);
        }

        #[test]
        fn proptest_block_sync_request_scale_roundtrip(
            start_height in any::<u64>(),
            max_blocks in 1u16..=500u16,
            has_known_hash in proptest::bool::ANY,
        ) {
            let req = BlockSyncRequest {
                start_height,
                max_blocks,
                direction: SyncDirection::Forward,
                known_block_hash: if has_known_hash { Some([0xAB; 32]) } else { None },
            };
            let encoded = req.encode();
            let decoded = BlockSyncRequest::decode(&mut &encoded[..]).unwrap();
            assert_eq!(req, decoded);
        }

        #[test]
        fn proptest_block_by_hash_request_scale_roundtrip(
            count in 1usize..=5usize,
        ) {
            let hashes: Vec<[u8; 32]> = (0..count)
                .map(|i| [(i * 17) as u8; 32])
                .collect();
            let req = BlockByHashRequest { block_hashes: hashes.clone() };
            let encoded = req.encode();
            let decoded = BlockByHashRequest::decode(&mut &encoded[..]).unwrap();
            assert_eq!(req, decoded);
        }
    }
}
