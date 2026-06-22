//! JSON-RPC WebSocket server using jsonrpsee.
//!
//! Provides 16 query methods + 3 subscriptions.
//! All methods take [`AppState`] as shared context.

use std::sync::Arc;

use jsonrpsee::server::ServerBuilder;
use jsonrpsee::RpcModule;

use crate::core::account::Address;
use crate::core::block::{Block, BlockHeader};
use crate::core::transaction::Transaction;
use crate::rpc::state::AppState;

/// JSON-RPC method result type.
type RpcResult = Result<serde_json::Value, jsonrpsee::types::ErrorObjectOwned>;

/// Start the JSON-RPC WebSocket server on the given address.
pub async fn start_rpc_server(
    addr: &str,
    state: Arc<AppState>,
) -> Result<jsonrpsee::server::ServerHandle, Box<dyn std::error::Error>> {
    let server = ServerBuilder::default()
        .build(addr)
        .await
        .map_err(|e| format!("failed to start RPC server: {e}"))?;

    let mut module = RpcModule::new(state);

    register_chain_methods(&mut module)?;
    register_state_methods(&mut module)?;
    register_block_methods(&mut module)?;
    register_tx_methods(&mut module)?;
    register_network_methods(&mut module)?;
    register_governance_methods(&mut module)?;
    register_subscription_methods(&mut module)?;

    let handle = server.start(module);
    Ok(handle)
}

// ── Helpers ──────────────────────────────────────────────────────

/// Helper: decode a hex address (with or without 0x prefix) into Address.
fn parse_address(hex_str: &str) -> Result<Address, jsonrpsee::types::ErrorObjectOwned> {
    let s = hex_str.trim_start_matches("0x");
    let bytes = hex::decode(s).map_err(|e| {
        jsonrpsee::types::ErrorObject::owned(-2, format!("invalid hex: {e}"), None::<()>)
    })?;
    if bytes.len() != 32 {
        return Err(jsonrpsee::types::ErrorObject::owned(-2, "address must be 32 bytes", None::<()>));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(Address::from(arr))
}

/// Helper: decode a block from storage at the given height.
fn load_block(storage: &dyn crate::storage::StorageEngine, height: u64)
    -> Option<Block>
{
    let key = height.to_be_bytes();
    let bytes = storage.get(crate::storage::tables::BLOCKS, &key).ok()??;
    parity_scale_codec::Decode::decode(&mut &bytes[..]).ok()
}

/// Helper: load the account for an address.
fn load_account(state: &AppState, addr: &Address) -> Option<crate::core::account::Account> {
    let sm = state.state_machine.read().ok()?;
    sm.get_account(addr)
}

// ── Chain methods ────────────────────────────────────────────────

fn register_chain_methods(
    module: &mut RpcModule<Arc<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    module.register_method("chain_get_health", |_params, app, _ext| -> RpcResult {
        let finalized = app.consensus.commit_tracker.as_ref()
            .map(|ct| ct.last_finalized_height());
        Ok(serde_json::json!({
            "status": "ok",
            "height": app.consensus.current_height,
            "chain_id": app.chain_id,
            "peers": 0,  // sync-only; network_peers method for live count
            "finalized_height": finalized.unwrap_or(0),
        }))
    })?;

    module.register_method("chain_get_height", |_params, app, _ext| -> RpcResult {
        Ok(serde_json::json!(app.consensus.current_height))
    })?;

    module.register_method("chain_get_genesis", |_params, app, _ext| -> RpcResult {
        Ok(serde_json::json!(hex::encode(app.genesis_hash)))
    })?;

    module.register_method("era_current", |_params, app, _ext| -> RpcResult {
        let era = crate::consensus::era::era_at_height(app.consensus.current_height);
        Ok(serde_json::json!(era))
    })?;

    Ok(())
}

// ── State methods ────────────────────────────────────────────────

fn register_state_methods(
    module: &mut RpcModule<Arc<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    module.register_method("state_get_balance", |params, app, _ext| -> RpcResult {
        let addr_str: String = params.one::<String>()?;
        let addr = parse_address(&addr_str)?;
        let bal = load_account(app, &addr).map(|a| a.balance).unwrap_or_default();
        Ok(serde_json::json!({"balance": format!("{bal:#x}")}))
    })?;

    module.register_method("state_get_nonce", |params, app, _ext| -> RpcResult {
        let addr_str: String = params.one::<String>()?;
        let addr = parse_address(&addr_str)?;
        let nonce = load_account(app, &addr).map(|a| a.nonce).unwrap_or(0);
        Ok(serde_json::json!({"nonce": nonce}))
    })?;

    module.register_method("validator_stake", |params, app, _ext| -> RpcResult {
        let addr_str: String = params.one::<String>()?;
        let addr = parse_address(&addr_str)?;
        let sm = app.state_machine.read().map_err(|e| {
            jsonrpsee::types::ErrorObject::owned(-1, format!("lock error: {e}"), None::<()>)
        })?;
        let stake = sm.validator_stake(&addr).unwrap_or_default();
        Ok(serde_json::json!({"stake": format!("{stake:#x}")}))
    })?;

    module.register_method("validator_set", |_params, app, _ext| -> RpcResult {
        let sm = app.state_machine.read().map_err(|e| {
            jsonrpsee::types::ErrorObject::owned(-1, format!("lock error: {e}"), None::<()>)
        })?;
        let validators: Vec<serde_json::Value> = sm.active_set()
            .iter()
            .map(|addr| serde_json::json!({
                "address": hex::encode(addr.as_ref()),
                "stake": format!("{:x}", sm.validator_stake(addr).unwrap_or_default()),
            }))
            .collect();
        Ok(serde_json::json!(validators))
    })?;

    Ok(())
}

// ── Block methods ────────────────────────────────────────────────

fn register_block_methods(
    module: &mut RpcModule<Arc<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    module.register_method("block_latest", |_params, app, _ext| -> RpcResult {
        let height = app.consensus.current_height;
        match load_block(&*app.storage, height) {
            Some(block) => Ok(serde_json::to_value(&block).unwrap_or_default()),
            None => Ok(serde_json::json!(null)),
        }
    })?;

    module.register_method("block_header", |params, app, _ext| -> RpcResult {
        let raw: serde_json::Value = params.one::<serde_json::Value>()?;
        let height = block_id_to_height(&raw, app)?;
        match load_block(&*app.storage, height) {
            Some(block) => Ok(serde_json::to_value(&block.header).unwrap_or_default()),
            None => Err(jsonrpsee::types::ErrorObject::owned(-4, "block not found", None::<()>)),
        }
    })?;

    module.register_method("block_get", |params, app, _ext| -> RpcResult {
        let raw: serde_json::Value = params.one::<serde_json::Value>()?;
        let height = block_id_to_height(&raw, app)?;
        match load_block(&*app.storage, height) {
            Some(block) => Ok(serde_json::to_value(&block).unwrap_or_default()),
            None => Err(jsonrpsee::types::ErrorObject::owned(-4, "block not found", None::<()>)),
        }
    })?;

    Ok(())
}

/// Convert a BlockId JSON value to a u64 height.
/// Supports: number (height), string "latest", hex string (hash — walks storage).
fn block_id_to_height(
    raw: &serde_json::Value,
    app: &AppState,
) -> Result<u64, jsonrpsee::types::ErrorObjectOwned> {
    match raw {
        serde_json::Value::Number(n) => {
            n.as_u64().ok_or_else(|| {
                jsonrpsee::types::ErrorObject::owned(-2, "invalid block height", None::<()>)
            })
        }
        serde_json::Value::String(s) if s == "latest" => {
            Ok(app.consensus.current_height)
        }
        serde_json::Value::String(s) => {
            // Hash-based lookup: walk storage from current height backward
            let s = s.trim_start_matches("0x");
            let hash = hex::decode(s).map_err(|e| {
                jsonrpsee::types::ErrorObject::owned(-2, format!("invalid hex hash: {e}"), None::<()>)
            })?;
            if hash.len() != 32 {
                return Err(jsonrpsee::types::ErrorObject::owned(-2, "hash must be 32 bytes", None::<()>));
            }
            // Linear scan backward from current height (acceptable for localnet)
            let mut h = app.consensus.current_height;
            while h > 0 {
                if let Some(block) = load_block(&*app.storage, h) {
                    let block_hash = blake3::hash(&parity_scale_codec::Encode::encode(&block.header));
                    if block_hash.as_bytes() == &hash[..] {
                        return Ok(h);
                    }
                }
                h -= 1;
            }
            Err(jsonrpsee::types::ErrorObject::owned(-4, "block not found by hash", None::<()>))
        }
        _ => Err(jsonrpsee::types::ErrorObject::owned(-2, "block id must be number, 'latest', or hex hash", None::<()>)),
    }
}

// ── Transaction methods ──────────────────────────────────────────

fn register_tx_methods(
    module: &mut RpcModule<Arc<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    module.register_method("tx_submit", |params, app, _ext| -> RpcResult {
        let hex_tx: String = params.one::<String>()?;
        let raw = hex::decode(hex_tx.trim_start_matches("0x")).map_err(|e| {
            jsonrpsee::types::ErrorObject::owned(-3, format!("invalid tx hex: {e}"), None::<()>)
        })?;
        let tx: Transaction = parity_scale_codec::Decode::decode(&mut &raw[..]).map_err(|e| {
            jsonrpsee::types::ErrorObject::owned(-3, format!("invalid tx data: {e}"), None::<()>)
        })?;
        let tx_hash = blake3::hash(&parity_scale_codec::Encode::encode(&tx));
        let tx_id = hex::encode(tx_hash.as_bytes());
        {
            let mut mp = app.mempool.write().map_err(|e| {
                jsonrpsee::types::ErrorObject::owned(-1, format!("mempool lock: {e}"), None::<()>)
            })?;
            mp.insert(tx);
        }
        Ok(serde_json::json!({"tx_hash": format!("0x{tx_id}")}))
    })?;

    // tx_status is limited without persistent tx tracking — returns pending or unknown
    module.register_method("tx_status", |params, app, _ext| -> RpcResult {
        let _tx_hash: String = params.one::<String>()?;
        // Without tx-result tracking, we can only report unknown
        // TODO: track tx results in StateMachine or a separate table
        Ok(serde_json::json!({"status": "unknown"}))
    })?;

    Ok(())
}

// ── Network methods ──────────────────────────────────────────────

fn register_network_methods(
    module: &mut RpcModule<Arc<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    module.register_method("network_peers", |_params, app, _ext| -> RpcResult {
        let peers = tokio::runtime::Handle::current().block_on(app.p2p.connected_peers());
        let peers_json: Vec<serde_json::Value> = peers
            .iter()
            .map(|pid| serde_json::json!({
                "peer_id": pid.to_string(),
            }))
            .collect();
        Ok(serde_json::json!(peers_json))
    })?;

    Ok(())
}

// ── Governance methods ──────────────────────────────────────────

fn register_governance_methods(
    module: &mut RpcModule<Arc<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    module.register_method("governance_proposals", |_params, app, _ext| -> RpcResult {
        // List active proposals via governance state
        // Without full SMT iteration, return empty list for now
        Ok(serde_json::json!([]))
    })?;

    module.register_method("governance_params", |_params, app, _ext| -> RpcResult {
        // Read governance params from SMT
        // Without param enumeration, return defaults
        Ok(serde_json::json!([]))
    })?;

    Ok(())
}

// ── Subscription methods ─────────────────────────────────────────

fn register_subscription_methods(
    module: &mut RpcModule<Arc<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    module.register_subscription(
        "subscribe_blocks",
        "blocks",
        "unsubscribe_blocks",
        |_params, pending: jsonrpsee::server::PendingSubscriptionSink, app: Arc<Arc<AppState>>, _ext: jsonrpsee::server::Extensions| async move {
            if let Ok(sink) = pending.accept().await {
                let mut rx = app.block_events.subscribe();
                loop {
                    match rx.recv().await {
                        Ok(header) => {
                            let val = serde_json::to_value(&header).unwrap_or_default();
                            if let Ok(msg) = jsonrpsee::server::SubscriptionMessage::from_json(&val) {
                                if sink.send(msg).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("block subscription lagged by {n} messages");
                        }
                    }
                }
            }
        },
    )?;

    module.register_subscription(
        "subscribe_finality",
        "finality",
        "unsubscribe_finality",
        |_params, pending: jsonrpsee::server::PendingSubscriptionSink, app: Arc<Arc<AppState>>, _ext: jsonrpsee::server::Extensions| async move {
            if let Ok(sink) = pending.accept().await {
                let mut rx = app.finality_events.subscribe();
                loop {
                    match rx.recv().await {
                        Ok(height) => {
                            let val = serde_json::json!({"height": height});
                            if let Ok(msg) = jsonrpsee::server::SubscriptionMessage::from_json(&val) {
                                if sink.send(msg).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("finality subscription lagged by {n} messages");
                        }
                    }
                }
            }
        },
    )?;

    module.register_subscription(
        "subscribe_votes",
        "votes",
        "unsubscribe_votes",
        |_params, pending: jsonrpsee::server::PendingSubscriptionSink, app: Arc<Arc<AppState>>, _ext: jsonrpsee::server::Extensions| async move {
            if let Ok(sink) = pending.accept().await {
                let mut rx = app.vote_events.subscribe();
                loop {
                    match rx.recv().await {
                        Ok(vote) => {
                            let val = serde_json::to_value(&vote).unwrap_or_default();
                            if let Ok(msg) = jsonrpsee::server::SubscriptionMessage::from_json(&val) {
                                if sink.send(msg).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("vote subscription lagged by {n} messages");
                        }
                    }
                }
            }
        },
    )?;

    Ok(())
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpsee::core::client::{ClientT, SubscriptionClientT};

    fn test_state() -> Arc<AppState> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rpc_test.db");
        let storage: Arc<dyn crate::storage::StorageEngine> = Arc::new(
            <crate::storage::redb::RedbEngine as crate::storage::StorageEngine>::open(&path)
                .unwrap(),
        );

        let state_machine = Arc::new(std::sync::RwLock::new(
            crate::core::state::StateMachine::new(
                Vec::<(Address, crate::core::account::Account)>::new(),
            ),
        ));
        let mempool = Arc::new(std::sync::RwLock::new(crate::mempool::Mempool::new(
            crate::mempool::MempoolConfig::default(),
        )));
        let consensus = Arc::new(crate::consensus::engine::ConsensusEngine::new(
            crate::consensus::ConsensusConfig::default(),
        ));

        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel(64);
        let (event_tx, _) = tokio::sync::broadcast::channel(64);
        let local_peer_id = libp2p::PeerId::random();
        let p2p = Arc::new(crate::network::P2pHandle {
            cmd_tx,
            local_peer_id,
            event_tx,
        });

        Arc::new(AppState::new(
            storage,
            state_machine,
            mempool,
            p2p,
            consensus,
            0,
            [0u8; 32],
        ))
    }

    async fn build_server(state: &Arc<AppState>) -> (jsonrpsee::server::ServerHandle, std::net::SocketAddr) {
        let server = ServerBuilder::default()
            .build("127.0.0.1:0")
            .await
            .unwrap();
        let addr = server.local_addr().unwrap();
        let mut module = RpcModule::new(state.clone());
        register_chain_methods(&mut module).unwrap();
        register_state_methods(&mut module).unwrap();
        register_block_methods(&mut module).unwrap();
        register_tx_methods(&mut module).unwrap();
        register_network_methods(&mut module).unwrap();
        register_governance_methods(&mut module).unwrap();
        register_subscription_methods(&mut module).unwrap();
        let handle = server.start(module);
        (handle, addr)
    }

    async fn rpc_call(
        addr: std::net::SocketAddr,
        method: &str,
        params: jsonrpsee::core::params::ArrayParams,
    ) -> serde_json::Value {
        let client = jsonrpsee::ws_client::WsClientBuilder::default()
            .build(&format!("ws://{addr}"))
            .await
            .unwrap();
        client.request(method, params).await.unwrap()
    }

    #[tokio::test]
    async fn test_chain_get_health() {
        let state = test_state();
        let (_handle, addr) = build_server(&state).await;
        let resp: serde_json::Value = rpc_call(addr, "chain_get_health", jsonrpsee::core::params::ArrayParams::new()).await;
        assert_eq!(resp["status"], "ok");
        assert_eq!(resp["height"], 0);
        assert_eq!(resp["chain_id"], 0);
        assert!(resp["peers"].is_number());
    }

    #[tokio::test]
    async fn test_chain_get_height() {
        let state = test_state();
        let (_handle, addr) = build_server(&state).await;
        let resp: serde_json::Value = rpc_call(addr, "chain_get_height", jsonrpsee::core::params::ArrayParams::new()).await;
        assert_eq!(resp, 0);
    }

    #[tokio::test]
    async fn test_chain_get_genesis() {
        let state = test_state();
        let (_handle, addr) = build_server(&state).await;
        let resp: serde_json::Value = rpc_call(addr, "chain_get_genesis", jsonrpsee::core::params::ArrayParams::new()).await;
        let hex_str = resp.as_str().unwrap();
        assert_eq!(hex_str.len(), 64); // 32 bytes in hex
    }

    #[tokio::test]
    async fn test_era_current() {
        let state = test_state();
        let (_handle, addr) = build_server(&state).await;
        let resp: serde_json::Value = rpc_call(addr, "era_current", jsonrpsee::core::params::ArrayParams::new()).await;
        assert_eq!(resp, 0);
    }

    #[tokio::test]
    async fn test_state_get_balance_unknown() {
        let state = test_state();
        let (_handle, addr) = build_server(&state).await;
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert("0xabababababababababababababababababababababababababababababababab").unwrap();
        let resp: serde_json::Value = rpc_call(addr, "state_get_balance", params).await;
        assert_eq!(resp["balance"], "0x0");
    }

    #[tokio::test]
    async fn test_state_get_nonce_unknown() {
        let state = test_state();
        let (_handle, addr) = build_server(&state).await;
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert("0xabababababababababababababababababababababababababababababababab").unwrap();
        let resp: serde_json::Value = rpc_call(addr, "state_get_nonce", params).await;
        assert_eq!(resp["nonce"], 0);
    }

    #[tokio::test]
    async fn test_block_latest_returns_null_when_no_blocks() {
        let state = test_state();
        let (_handle, addr) = build_server(&state).await;
        let resp: serde_json::Value = rpc_call(addr, "block_latest", jsonrpsee::core::params::ArrayParams::new()).await;
        assert!(resp.is_null());
    }

    #[tokio::test]
    async fn test_validator_set_empty() {
        let state = test_state();
        let (_handle, addr) = build_server(&state).await;
        let resp: serde_json::Value = rpc_call(addr, "validator_set", jsonrpsee::core::params::ArrayParams::new()).await;
        assert!(resp.is_array());
        assert!(resp.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_tx_submit_invalid_hex() {
        let state = test_state();
        let (_handle, addr) = build_server(&state).await;
        let mut params = jsonrpsee::core::params::ArrayParams::new();
        params.insert("not-hex").unwrap();
        let result = {
            let client = jsonrpsee::ws_client::WsClientBuilder::default()
                .build(&format!("ws://{addr}"))
                .await
                .unwrap();
            let r: Result<serde_json::Value, jsonrpsee::core::client::Error> =
                client.request("tx_submit", params).await;
            r
        };
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_subscribe_blocks_returns_subscription_id() {
        let state = test_state();
        let (_handle, addr) = build_server(&state).await;

        let client = jsonrpsee::ws_client::WsClientBuilder::default()
            .build(&format!("ws://{addr}"))
            .await
            .unwrap();

        // Subscribe — this should succeed if the method is registered
        let stream = client
            .subscribe::<serde_json::Value, _>(
                "subscribe_blocks",
                jsonrpsee::core::params::ArrayParams::new(),
                "unsubscribe_blocks",
            )
            .await;
        assert!(stream.is_ok(), "subscribe failed: {:?}", stream.err());

        // Send a block event via the broadcast channel to test notification delivery
        let sig = crate::crypto::falcon::Falcon512Signature::from_bytes(
            &[0u8; crate::crypto::constants::FALCON_SIGNATURE_SIZE],
        )
        .unwrap();
        let header = crate::core::block::BlockHeader {
            height: 42,
            parent_hash: [0u8; 32],
            global_state_root: [0u8; 32],
            tx_root: [0u8; 32],
            timestamp: 0,
            proposer: crate::core::account::Address::from([0u8; 32]),
            chain_id: 0,
            proposer_signature: sig,
        };
        state.block_events.send(header).unwrap();

        // Read from the subscription stream
        let mut stream = stream.unwrap();
        let notification = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            stream.next(),
        )
        .await;
        assert!(notification.is_ok(), "timed out waiting for block event");
        if let Ok(Some(Ok(val))) = notification {
            assert_eq!(val["height"], 42);
        }
    }
}
