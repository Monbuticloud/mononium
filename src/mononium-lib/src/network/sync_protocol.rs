//! Sync protocol — libp2p Request-Response handlers.
//!
//! Two protocols:
//! - `/mononium/sync/1.0` — `BlockSyncRequest` → `BlockSyncResponse`
//! - `/mononium/hash-sync/1.0` — `BlockByHashRequest` → `BlockByHashResponse`
//!
//! Uses JSON encoding on the wire via `request_response::json::Behaviour`.

use libp2p::StreamProtocol;
use libp2p_request_response::{self as request_response, json, ProtocolSupport};

use crate::network::messages::{
    BlockByHashRequest, BlockByHashResponse, BlockSyncRequest, BlockSyncResponse,
};

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
