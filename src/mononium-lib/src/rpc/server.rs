//! JSON-RPC WebSocket server using jsonrpsee.
//!
//! Provides 16 query methods + 3 subscriptions.
//! All methods take [`AppState`] as shared context.

use std::sync::Arc;

use jsonrpsee::server::ServerBuilder;
use jsonrpsee::RpcModule;

use crate::rpc::state::AppState;

/// JSON-RPC method result type.
type RpcResult = Result<serde_json::Value, jsonrpsee::types::ErrorObjectOwned>;

/// Start the JSON-RPC WebSocket server on the given address.
///
/// Returns a handle that can be used to stop the server gracefully.
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

    let handle = server.start(module);
    Ok(handle)
}

/// Chain-level queries.
fn register_chain_methods(
    module: &mut RpcModule<Arc<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    module.register_method("chain_get_health", |_params, app, _ext| -> RpcResult {
        let height = app.consensus.current_height;
        Ok(serde_json::json!({
            "status": "ok",
            "height": height,
            "chain_id": app.chain_id,
        }))
    })?;

    module.register_method("chain_get_height", |_params, app, _ext| -> RpcResult {
        Ok(serde_json::json!(app.consensus.current_height))
    })?;

    Ok(())
}

/// State queries (balance, nonce, etc.).
fn register_state_methods(
    module: &mut RpcModule<Arc<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    module.register_method("state_get_balance", |params, app, _ext| -> RpcResult {
        let addr: String = params.one::<String>()?;
        let addr_str = addr.trim_start_matches("0x");
        let addr_bytes = hex::decode(addr_str).map_err(|e| {
            jsonrpsee::types::ErrorObject::owned(-2, format!("invalid hex address: {e}"), None::<()>)
        })?;
        if addr_bytes.len() != 32 {
            return Err(jsonrpsee::types::ErrorObject::owned(-2, "address must be 32 bytes", None::<()>));
        }
        let mut addr_arr = [0u8; 32];
        addr_arr.copy_from_slice(&addr_bytes);
        let account = {
            let sm = app.state_machine.read().map_err(|e| {
                jsonrpsee::types::ErrorObject::owned(-1, format!("lock error: {e}"), None::<()>)
            })?;
            let address = crate::core::account::Address::from(addr_arr);
            sm.get_account(&address).map(|a| a.balance)
        };
        match account {
            Some(bal) => Ok(serde_json::json!({"balance": format!("{bal:#x}")})),
            None => Ok(serde_json::json!({"balance": "0x0"})),
        }
    })?;

    module.register_method("state_get_nonce", |params, app, _ext| -> RpcResult {
        let addr: String = params.one::<String>()?;
        let addr_str = addr.trim_start_matches("0x");
        let addr_bytes = hex::decode(addr_str).map_err(|e| {
            jsonrpsee::types::ErrorObject::owned(-2, format!("invalid hex address: {e}"), None::<()>)
        })?;
        if addr_bytes.len() != 32 {
            return Err(jsonrpsee::types::ErrorObject::owned(-2, "address must be 32 bytes", None::<()>));
        }
        let mut addr_arr = [0u8; 32];
        addr_arr.copy_from_slice(&addr_bytes);
        let nonce = {
            let sm = app.state_machine.read().map_err(|e| {
                jsonrpsee::types::ErrorObject::owned(-1, format!("lock error: {e}"), None::<()>)
            })?;
            let address = crate::core::account::Address::from(addr_arr);
            sm.get_account(&address).map(|a| a.nonce)
        };
        Ok(serde_json::json!({"nonce": nonce.unwrap_or(0)}))
    })?;

    Ok(())
}

/// Block queries.
fn register_block_methods(
    module: &mut RpcModule<Arc<AppState>>,
) -> Result<(), Box<dyn std::error::Error>> {
    module.register_method("block_latest", |_params, app, _ext| -> RpcResult {
        let height = app.consensus.current_height;
        let key = height.to_be_bytes();
        let bytes: Option<Vec<u8>> = app.storage.get(crate::storage::tables::BLOCKS, &key)
            .ok()
            .flatten();
        match bytes {
            Some(b) => {
                let block: crate::core::block::Block =
                    parity_scale_codec::Decode::decode(&mut &b[..]).map_err(|e| {
                        jsonrpsee::types::ErrorObject::owned(
                            -1,
                            format!("failed to decode block: {e}"),
                            None::<()>,
                        )
                    })?;
                Ok(serde_json::to_value(&block).unwrap_or_default())
            }
            None => Ok(serde_json::json!(null)),
        }
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonrpsee::core::client::ClientT;
    use std::sync::{Arc, RwLock};

    fn test_state() -> Arc<AppState> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rpc_test.db");
        let storage: Arc<dyn crate::storage::StorageEngine> = Arc::new(
            <crate::storage::redb::RedbEngine as crate::storage::StorageEngine>::open(&path)
                .unwrap(),
        );

        let state_machine = Arc::new(RwLock::new(crate::core::state::StateMachine::new(
            Vec::<(crate::core::account::Address, crate::core::account::Account)>::new(),
        )));
        let mempool = Arc::new(RwLock::new(crate::mempool::Mempool::new(
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

        Arc::new(AppState {
            storage,
            state_machine,
            mempool,
            p2p,
            consensus,
            chain_id: 0,
            genesis_hash: [0u8; 32],
        })
    }

    #[tokio::test]
    async fn test_chain_get_health_returns_ok() {
        let state = test_state();
        let server = ServerBuilder::default()
            .build("127.0.0.1:0")
            .await
            .unwrap();
        let addr = server.local_addr().unwrap();

        let mut module = RpcModule::new(state.clone());
        register_chain_methods(&mut module).unwrap();
        let _handle = server.start(module);

        let client = jsonrpsee::ws_client::WsClientBuilder::default()
            .build(&format!("ws://{addr}"))
            .await
            .unwrap();

        let response: serde_json::Value = client
            .request("chain_get_health", jsonrpsee::core::params::ArrayParams::new())
            .await
            .unwrap();

        assert_eq!(response["status"], "ok");
        assert_eq!(response["height"], 0);
        assert_eq!(response["chain_id"], 0);
    }
}
