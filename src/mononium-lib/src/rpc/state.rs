//! Shared application state for RPC and REST endpoints.

use std::sync::{Arc, RwLock};

use crate::consensus::engine::ConsensusEngine;
use crate::core::block::{BlockHeader, CommitVote};
use crate::core::state::StateMachine;
use crate::mempool::Mempool;
use crate::network::P2pHandle;
use crate::storage::StorageEngine;

/// Shared state for JSON-RPC and REST servers.
///
/// All fields are thread-safe (Send + Sync):
/// - `storage`: read-only database access (Arc<dyn StorageEngine>)
/// - `state_machine`: state behind RwLock for concurrent reads
/// - `mempool`: tx pool behind RwLock
/// - `p2p`: cloneable network handle
/// - `consensus`: read-only engine state
/// - `block_events`, `finality_events`, `vote_events`: broadcast channels for subscriptions
pub struct AppState {
    pub storage: Arc<dyn StorageEngine>,
    pub state_machine: Arc<RwLock<StateMachine>>,
    pub mempool: Arc<RwLock<Mempool>>,
    pub p2p: Arc<P2pHandle>,
    pub consensus: Arc<ConsensusEngine>,
    pub chain_id: u64,
    pub genesis_hash: [u8; 32],

    /// Broadcast channel: new blocks (headers) for `subscribe_blocks`.
    pub block_events: tokio::sync::broadcast::Sender<BlockHeader>,
    /// Broadcast channel: finality events for `subscribe_finality`.
    pub finality_events: tokio::sync::broadcast::Sender<u64>,
    /// Broadcast channel: commit votes for `subscribe_votes`.
    pub vote_events: tokio::sync::broadcast::Sender<CommitVote>,
}

impl AppState {
    /// Create a new AppState with default broadcast channels (capacity 256).
    pub fn new(
        storage: Arc<dyn StorageEngine>,
        state_machine: Arc<RwLock<StateMachine>>,
        mempool: Arc<RwLock<Mempool>>,
        p2p: Arc<P2pHandle>,
        consensus: Arc<ConsensusEngine>,
        chain_id: u64,
        genesis_hash: [u8; 32],
    ) -> Self {
        let (block_events, _) = tokio::sync::broadcast::channel(256);
        let (finality_events, _) = tokio::sync::broadcast::channel(256);
        let (vote_events, _) = tokio::sync::broadcast::channel(256);
        Self {
            storage,
            state_machine,
            mempool,
            p2p,
            consensus,
            chain_id,
            genesis_hash,
            block_events,
            finality_events,
            vote_events,
        }
    }
}
