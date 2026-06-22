//! Sync protocol — libp2p Request-Response handlers.
//!
//! Two protocols:
//! - `/mononium/sync/1.0` — `BlockSyncRequest` → `BlockSyncResponse`
//! - `/mononium/hash-sync/1.0` — `BlockByHashRequest` → `BlockByHashResponse`
//!
//! Uses JSON encoding on the wire via `request_response::json::Behaviour`.

use libp2p::StreamProtocol;
use libp2p_request_response::{self as request_response, json, ProtocolSupport};

use parity_scale_codec::{Decode, Encode};

use crate::core::block::{Block, BlockBody};
use crate::network::messages::{
    compute_batch_hash, validate_by_hash_request, validate_sync_request, BlockByHashRequest,
    BlockByHashResponse, BlockSyncRequest, BlockSyncResponse, SyncDirection, MAX_SYNC_BLOCKS,
};
use crate::storage::tables;
use crate::storage::StorageEngine;

/// The sync protocol name.
pub const SYNC_PROTOCOL: &str = "/mononium/sync/1.0";

/// The hash-sync protocol name.
pub const HASH_SYNC_PROTOCOL: &str = "/mononium/hash-sync/1.0";

/// Build a `request_response::Behaviour` for the two sync protocols.
///
/// Both protocols are registered with full inbound + outbound support.
pub fn build_sync_behaviour() -> json::Behaviour<SyncRequest, SyncResponse> {
    json::Behaviour::new(
        [
            (
                StreamProtocol::new(SYNC_PROTOCOL),
                ProtocolSupport::Full,
            ),
            (
                StreamProtocol::new(HASH_SYNC_PROTOCOL),
                ProtocolSupport::Full,
            ),
        ],
        request_response::Config::default()
            .with_request_timeout(std::time::Duration::from_secs(30)),
    )
}

/// Unified request type for both sync protocols.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SyncRequest {
    /// Block range sync.
    BlockSync(BlockSyncRequest),
    /// Block by hash lookup.
    BlockByHash(BlockByHashRequest),
}

/// Unified response type for both sync protocols.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SyncResponse {
    /// Block range sync response.
    BlockSync(BlockSyncResponse),
    /// Block by hash response.
    BlockByHash(BlockByHashResponse),
}

// ---------------------------------------------------------------------------
// Request serving — pure functions backed by a StorageEngine
// ---------------------------------------------------------------------------

// (implementation will be added in the GREEN commit)
pub fn serve_sync_request(
    _request: &SyncRequest,
    _storage: &dyn StorageEngine,
    _genesis_hash: &[u8; 32],
    _highest_known_height: u64,
) -> Option<SyncResponse> {
    todo!("serve_sync_request — GREEN commit");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::account::Address;
    use crate::crypto::constants::FALCON_SIGNATURE_SIZE;
    use crate::crypto::falcon::Falcon512Signature;
    use crate::storage::redb::RedbEngine;
    use parity_scale_codec::Encode as _;
    use tempfile::TempDir;

    fn dummy_header(height: u64) -> crate::core::block::BlockHeader {
        crate::core::block::BlockHeader {
            height,
            parent_hash: [0; 32],
            global_state_root: [0; 32],
            tx_root: [0; 32],
            timestamp: 1_700_000_000 + height,
            proposer: Address::from([0x01; 32]),
            chain_id: 0,
            proposer_signature: Falcon512Signature::from_bytes(
                &[0xCD; FALCON_SIGNATURE_SIZE],
            )
            .unwrap(),
        }
    }

    fn store_block(engine: &RedbEngine, height: u64) {
        let block = Block {
            header: dummy_header(height),
            body: BlockBody { transactions: vec![] },
        };
        let key = height.to_be_bytes();
        let encoded = block.encode();
        engine.put(tables::BLOCKS, &key, &encoded).unwrap();
    }

    // -----------------------------------------------------------------------
    // BlockSync — Forward
    // -----------------------------------------------------------------------

    #[test]
    fn test_serve_block_sync_forward() {
        let dir = TempDir::with_prefix("mononium-serve-test-").unwrap();
        let db_path = dir.path().join("test.redb");
        let engine = RedbEngine::open(&db_path).unwrap();

        for h in 1..=10u64 {
            store_block(&engine, h);
        }

        let genesis_hash = [0xFE; 32];
        let request = SyncRequest::BlockSync(BlockSyncRequest {
            start_height: 3,
            max_blocks: 5,
            direction: SyncDirection::Forward,
            known_block_hash: None,
        });

        let response = serve_sync_request(&request, &engine, &genesis_hash, 10);
        let SyncResponse::BlockSync(resp) = response.unwrap() else {
            panic!("expected BlockSync");
        };

        assert_eq!(resp.blocks.len(), 5);
        assert_eq!(resp.blocks[0].header.height, 3);
        assert_eq!(resp.blocks[4].header.height, 7);
        assert_eq!(resp.highest_height, 10);
    }

    // -----------------------------------------------------------------------
    // BlockSync — Backward
    // -----------------------------------------------------------------------

    #[test]
    fn test_serve_block_sync_backward() {
        let dir = TempDir::with_prefix("mononium-serve-test-").unwrap();
        let db_path = dir.path().join("test.redb");
        let engine = RedbEngine::open(&db_path).unwrap();

        for h in 1..=10u64 {
            store_block(&engine, h);
        }

        let request = SyncRequest::BlockSync(BlockSyncRequest {
            start_height: 10,
            max_blocks: 4,
            direction: SyncDirection::Backward,
            known_block_hash: None,
        });

        let response = serve_sync_request(&request, &engine, &[0; 32], 10);
        let SyncResponse::BlockSync(resp) = response.unwrap() else {
            panic!("expected BlockSync");
        };

        // Backward returns blocks in ascending order: heights 6,7,8,9
        assert_eq!(resp.blocks.len(), 4);
        assert_eq!(resp.blocks[0].header.height, 6);
        assert_eq!(resp.blocks[3].header.height, 9);
    }

    // -----------------------------------------------------------------------
    // BlockSync — runs out of blocks (fewer than requested)
    // -----------------------------------------------------------------------

    #[test]
    fn test_serve_block_sync_partial() {
        let dir = TempDir::with_prefix("mononium-serve-test-").unwrap();
        let db_path = dir.path().join("test.redb");
        let engine = RedbEngine::open(&db_path).unwrap();

        for h in 1..=3u64 {
            store_block(&engine, h);
        }

        let request = SyncRequest::BlockSync(BlockSyncRequest {
            start_height: 2,
            max_blocks: 100,
            direction: SyncDirection::Forward,
            known_block_hash: None,
        });

        let response = serve_sync_request(&request, &engine, &[0; 32], 3);
        let SyncResponse::BlockSync(resp) = response.unwrap() else {
            panic!("expected BlockSync");
        };

        assert_eq!(resp.blocks.len(), 2); // heights 2,3
    }

    // -----------------------------------------------------------------------
    // BlockSync — known_block_hash fork mismatch
    // -----------------------------------------------------------------------

    #[test]
    fn test_serve_block_sync_fork_mismatch() {
        let dir = TempDir::with_prefix("mononium-serve-test-").unwrap();
        let db_path = dir.path().join("test.redb");
        let engine = RedbEngine::open(&db_path).unwrap();

        // Store a block at height 5
        store_block(&engine, 5);

        // Request with known_block_hash that doesn't match
        let request = SyncRequest::BlockSync(BlockSyncRequest {
            start_height: 6,
            max_blocks: 5,
            direction: SyncDirection::Forward,
            known_block_hash: Some([0xFF; 32]), // doesn't match real hash
        });

        let response = serve_sync_request(&request, &engine, &[0xFE; 32], 5);
        let SyncResponse::BlockSync(resp) = response.unwrap() else {
            panic!("expected BlockSync");
        };

        assert!(resp.blocks.is_empty(), "fork mismatch should return empty");
    }

    // -----------------------------------------------------------------------
    // BlockSync — invalid request (max_blocks = 0)
    // -----------------------------------------------------------------------

    #[test]
    fn test_serve_block_sync_invalid_request_returns_none() {
        let dir = TempDir::with_prefix("mononium-serve-test-").unwrap();
        let db_path = dir.path().join("test.redb");
        let engine = RedbEngine::open(&db_path).unwrap();

        let request = SyncRequest::BlockSync(BlockSyncRequest {
            start_height: 1,
            max_blocks: 0,
            direction: SyncDirection::Forward,
            known_block_hash: None,
        });

        let response = serve_sync_request(&request, &engine, &[0; 32], 0);
        assert!(response.is_none(), "invalid request should return None");
    }

    // -----------------------------------------------------------------------
    // BlockByHash
    // -----------------------------------------------------------------------

    #[test]
    fn test_serve_block_by_hash() {
        let dir = TempDir::with_prefix("mononium-serve-test-").unwrap();
        let db_path = dir.path().join("test.redb");
        let engine = RedbEngine::open(&db_path).unwrap();

        // Store a block and its hash index
        let block = Block {
            header: dummy_header(42),
            body: BlockBody { transactions: vec![] },
        };
        let encoded = block.encode();
        let hash: [u8; 32] = *blake3::hash(&encoded).as_bytes();
        let height_key = 42u64.to_be_bytes();
        engine.put(tables::BLOCKS, &height_key, &encoded).unwrap();
        engine.put(tables::BLOCK_HASHES, &hash, &height_key).unwrap();

        let request = SyncRequest::BlockByHash(BlockByHashRequest {
            block_hashes: vec![hash],
        });

        let response = serve_sync_request(&request, &engine, &[0; 32], 0);
        let SyncResponse::BlockByHash(resp) = response.unwrap() else {
            panic!("expected BlockByHash");
        };

        assert_eq!(resp.blocks.len(), 1);
        assert_eq!(resp.blocks[0].header.height, 42);
    }

    // -----------------------------------------------------------------------
    // BlockByHash — missing hash returns no blocks
    // -----------------------------------------------------------------------

    #[test]
    fn test_serve_block_by_hash_missing() {
        let dir = TempDir::with_prefix("mononium-serve-test-").unwrap();
        let db_path = dir.path().join("test.redb");
        let engine = RedbEngine::open(&db_path).unwrap();

        let request = SyncRequest::BlockByHash(BlockByHashRequest {
            block_hashes: vec![[0xDD; 32]],
        });

        let response = serve_sync_request(&request, &engine, &[0; 32], 0);
        let SyncResponse::BlockByHash(resp) = response.unwrap() else {
            panic!("expected BlockByHash");
        };

        assert!(resp.blocks.is_empty());
    }
}
