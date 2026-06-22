//! Shared application state for RPC and REST endpoints.

use std::sync::{Arc, RwLock};

use crate::consensus::engine::ConsensusEngine;
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
pub struct AppState {
    pub storage: Arc<dyn StorageEngine>,
    pub state_machine: Arc<RwLock<StateMachine>>,
    pub mempool: Arc<RwLock<Mempool>>,
    pub p2p: Arc<P2pHandle>,
    pub consensus: Arc<ConsensusEngine>,
    pub chain_id: u64,
    pub genesis_hash: [u8; 32],
}
