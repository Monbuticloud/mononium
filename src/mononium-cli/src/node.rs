//! Node daemon — startup lifecycle, REST API, block production loop.
//!
//! Startup order:
//! 1. Load config file (if provided), merge CLI overrides
//! 2. Setup tracing/logging
//! 3. Open redb database
//! 4. Load genesis (if fresh database)
//! 5. Initialize state machine from stored state
//! 6. Start REST API server
//! 7. Start block production loop

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;

use mononium_lib::config::NodeConfig;
use mononium_lib::config::CliOverrides;
use mononium_lib::consensus::era;
use mononium_lib::core::account::Address;
use mononium_lib::core::block::{Block, BlockBody, BlockHeader};
use mononium_lib::core::state::StateMachine;
use mononium_lib::core::transaction::Transaction;
use mononium_lib::error::LibError;
use mononium_lib::mempool::{Mempool, MempoolConfig};
use mononium_lib::rpc;
use mononium_lib::rpc::server::start_rpc_server;
use mononium_lib::rpc::state::AppState as RpcAppState;
use mononium_lib::storage::genesis::load_genesis;
use parity_scale_codec::Decode;
use mononium_lib::storage::redb::RedbEngine;
use mononium_lib::storage::tables;
use mononium_lib::storage::StorageEngine;

use crate::NodeArgs;

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

struct AppState {
    engine: Arc<dyn StorageEngine>,
    state: StateMachine,
    mempool: Mempool,
    current_height: u64,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the node daemon with the given CLI arguments.
pub async fn run_node(args: NodeArgs) -> Result<()> {
    // ---------- 1. Load & merge config ----------
    let mut config = if let Some(ref config_path) = args.config {
        NodeConfig::load(config_path)
            .with_context(|| format!("failed to load config from {}", config_path.display()))?
    } else {
        NodeConfig::default()
    };

    config.merge_cli(build_cli_overrides(&args));

    // ---------- 2. Setup logging ----------
    let log_level = args.log_level.as_deref().unwrap_or("info");
    let filter = EnvFilter::try_new(format!("mononium={log_level}"))
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();

    // ---------- 3. Validate ----------
    config.validate().context("config validation failed")?;

    // ---------- 4. Log role ----------
    if config.observer {
        tracing::info!("role: observer (no signing, sync-only)");
    } else {
        tracing::info!("role: validator");
    }

    // ---------- 6. Open database ----------
    let data_dir = config.data_dir();
    let db_dir = Path::new(&data_dir);
    tokio::fs::create_dir_all(db_dir).await?;
    let db_path = db_dir.join("chain.redb");
    tracing::info!(path = %db_path.display(), "opening database");
    let engine = RedbEngine::open(&db_path)
        .context("failed to open database")?;

    // ---------- 7. Load genesis ----------
    {
        let genesis_loaded = engine.exists(tables::META, tables::GENESIS_LOADED_KEY)?;
        if !genesis_loaded {
            let genesis_path = config.genesis_path()
                .context("genesis path not configured")?;
            tracing::info!(path = genesis_path, "loading genesis");
            load_genesis(&engine, Path::new(genesis_path))
                .context("failed to load genesis")?;
        } else {
            tracing::info!("genesis already loaded, skipping");
        }
    }

    // ---------- 8. Determine current height ----------
    let current_height = load_latest_height(&engine)?;
    tracing::info!(height = current_height, "node starting");

    // ---------- 9. Wrap engine in Arc for sharing ----------
    let engine: Arc<dyn StorageEngine> = Arc::new(engine);

    // ---------- 10. Load genesis hash + chain_id from storage ----------
    let genesis_hash_raw = engine
        .get(tables::META, tables::GENESIS_HASH_KEY)?
        .unwrap_or_else(|| vec![0u8; 32]);
    let mut genesis_hash = [0u8; 32];
    genesis_hash.copy_from_slice(&genesis_hash_raw);
    let chain_id = engine
        .get(tables::META, tables::CHAIN_ID_KEY)
        .ok()
        .flatten()
        .and_then(|v| {
            let arr: [u8; 8] = v.try_into().ok()?;
            Some(u64::from_le_bytes(arr))
        })
        .unwrap_or(0);

    // ---------- 11. Initialize state machine from storage ----------
    let mut state = load_state_from_storage(&*engine)?;
    tracing::info!(height = current_height, "state machine initialized");

    // ---------- 11b. Crash recovery: verify state consistency ----------
    if let Err(e) = verify_state_consistency(&mut state, &*engine, current_height) {
        tracing::error!("crash recovery failed: {e}");
        // Mismatch is fatal — redb ACID guarantee should prevent this
        anyhow::bail!("state root mismatch — database may be corrupted: {e}");
    }

    // ---------- 12. Create mempool ----------
    let mempool_config = MempoolConfig {
        max_size: 10_000,
        ttl: Duration::from_secs(600),
        min_fee: primitive_types::U256::zero(),
        per_sender_cap: 50,
    };
    let mempool = Mempool::new(mempool_config.clone());
    tracing::info!("mempool ready");

    // ---------- 13. Start P2P networking (if enabled) ----------
    let p2p_port = config.p2p_port();
    let p2p_handle: Arc<mononium_lib::network::P2pHandle> = if p2p_port > 0 {
        let p2p_config = mononium_lib::network::P2pConfig {
            p2p_port,
            bootstrap_peers: config.bootnodes().iter()
                .filter_map(|s| s.parse::<libp2p::Multiaddr>().ok())
                .collect(),
            enable_mdns: config.enable_mdns(),
            max_peers: 50,
        };
        match mononium_lib::network::P2pService::new(p2p_config, chain_id) {
            Ok(service) => {
                match service.start() {
                    Ok(handle) => {
                        tracing::info!("P2P networking started on port {p2p_port}");
                        Arc::new(handle)
                    }
                    Err(e) => {
                        tracing::error!("P2P start failed: {e}");
                        Arc::new(mononium_lib::network::dummy_p2p_handle())
                    }
                }
            }
            Err(e) => {
                tracing::error!("P2P service creation failed: {e}");
                Arc::new(mononium_lib::network::dummy_p2p_handle())
            }
        }
    } else {
        Arc::new(mononium_lib::network::dummy_p2p_handle())
    };

    // ---------- 14. Spawn sync loop (background task) ----------
    if p2p_port > 0 {
        let sync_p2p = p2p_handle.clone();
        let sync_engine = engine.clone();
        let cursor_dir = Path::new(&data_dir).join(chain_id.to_string());
        let cursor_path = cursor_dir.to_string_lossy().to_string();
        let era_len = mononium_lib::consensus::era::ERA_LENGTH;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await; // wait for P2P to stabilize
            loop {
                let should_wait = {
                    match mononium_lib::network::sync::run_sync_loop(
                        &*sync_p2p,
                        &*sync_engine,
                        genesis_hash,
                        std::path::Path::new(&cursor_path),
                        era_len,
                    )
                    .await
                    {
                        Ok(()) => {
                            tracing::info!("sync loop completed: node is synced to tip");
                            false
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            if msg.contains("no connected peers") {
                                tracing::debug!("sync: waiting for peers...");
                            } else {
                                tracing::warn!("sync loop exited: {msg}");
                            }
                            true
                        }
                    }
                };
                if should_wait {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        });
    }

    // ---------- 15. Start JSON-RPC WebSocket server ----------
    let rpc_port = config.rpc_port();
    if rpc_port > 0 {
        let rpc_consensus = Arc::new({
            let mut c = mononium_lib::consensus::engine::ConsensusEngine::new(
                mononium_lib::consensus::ConsensusConfig::default(),
            );
            c.set_current_height(current_height);
            c
        });

        let rpc_state = Arc::new(RpcAppState::new(
            engine.clone(),
            Arc::new(std::sync::RwLock::new(
                mononium_lib::core::state::StateMachine::new(Vec::<(Address, _)>::new()),
            )),
            Arc::new(std::sync::RwLock::new(Mempool::new(mempool_config))),
            p2p_handle.clone(),
            rpc_consensus,
            chain_id,
            genesis_hash,
        ));

        let rpc_addr = format!("0.0.0.0:{rpc_port}");
        tracing::info!("RPC WebSocket server listening on {rpc_addr}");

        tokio::spawn(async move {
            if let Err(e) = start_rpc_server(&rpc_addr, rpc_state).await {
                tracing::error!("RPC server failed: {e}");
            }
        });
    }

    // ---------- 16. Start REST API ----------
    let rest_port = config.rest_port();
    let shared = Arc::new(Mutex::new(AppState {
        engine,
        state,
        mempool,
        current_height,
    }));

    let app = build_router(shared.clone());
    let addr = format!("0.0.0.0:{rest_port}");
    tracing::info!("REST API listening on {addr}");

    tokio::spawn(async move {
        if let Err(e) = serve_rest(app, &addr).await {
            tracing::error!("REST server failed: {e}");
        }
    });

    // ---------- 17. Block production loop ----------
    tracing::info!("starting block production loop (5s blocks)");
    block_production_loop(shared).await;

    Ok(())
}

async fn serve_rest(app: Router, addr: &str) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Build CliOverrides from CLI args
// ---------------------------------------------------------------------------

fn build_cli_overrides(args: &NodeArgs) -> CliOverrides {
    let bootnodes = if args.bootnode.is_empty() {
        None
    } else {
        Some(args.bootnode.clone())
    };

    CliOverrides {
        genesis: args.genesis.clone(),
        key: args.key.clone(),
        key_file: args.key_file.clone(),
        observer: if args.observer { Some(true) } else { None },
        p2p_port: args.p2p_port,
        rpc_port: args.rpc_port,
        rest_port: args.rest_port,
        bootnodes,
        data_dir: args.data_dir.clone(),
        storage_mode: args.storage_mode.clone(),
        compact_eras: None,
        full_node_rpc: None,
        log_level: args.log_level.clone(),
        log_json: args.log_format.as_ref().map(|f| f == "json"),
        unlock_timeout: args.unlock_timeout,
    }
}

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

/// Read the latest block height from the stored blocks table.
/// Keys are big-endian u64 so the max byte sequence is the highest height.
fn load_latest_height(engine: &dyn StorageEngine) -> Result<u64> {
    let keys = engine.list_keys(tables::BLOCKS)?;
    if keys.is_empty() {
        return Ok(0);
    }
    // Find the highest key (lexicographic on BE bytes)
    let max_key = keys.iter().max().unwrap();
    let mut buf = [0u8; 8];
    buf.copy_from_slice(max_key);
    Ok(u64::from_be_bytes(buf))
}

/// Verify that the rebuilt SMT state root matches the latest block's global_state_root.
/// This is the core crash recovery check: if they match, the node can safely resume.
/// If they don't, the database is corrupt and the node must panic.
fn verify_state_consistency(
    state: &mut StateMachine,
    engine: &dyn StorageEngine,
    height: u64,
) -> Result<()> {
    if height == 0 {
        return Ok(()); // genesis — no previous block to verify against
    }
    let key = height.to_be_bytes();
    let raw = engine
        .get(tables::BLOCKS, &key)?
        .ok_or_else(|| anyhow::anyhow!("block {height} not found during consistency check"))?;
    let block = Block::decode(&mut &raw[..])
        .map_err(|e| anyhow::anyhow!("failed to decode block {height}: {e}"))?;
    let expected_root = block.header.global_state_root;
    let actual_root = state.state_root();
    if expected_root != actual_root {
        anyhow::bail!(
            "state root mismatch at height {height}: expected {} got {}",
            hex::encode(expected_root),
            hex::encode(actual_root),
        );
    }
    tracing::info!(
        height,
        state_root = %hex::encode(expected_root),
        "state consistency verified"
    );
    Ok(())
}

/// Load all accounts from storage into the in-memory state machine.
fn load_state_from_storage(engine: &dyn StorageEngine) -> Result<StateMachine> {
    let account_keys = engine.list_keys(tables::ACCOUNTS)?;
    let mut accounts: Vec<(Address, mononium_lib::core::account::Account)> = Vec::new();
    for key in &account_keys {
        if key.len() != 32 {
            continue; // skip non-address keys
        }
        if let Ok(Some(raw)) = engine.get(tables::ACCOUNTS, key) {
            if let Ok(acct) = parity_scale_codec::Decode::decode(&mut &raw[..]) {
                let addr = Address::from({
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(key);
                    arr
                });
                accounts.push((addr, acct));
            }
        }
    }
    Ok(StateMachine::new(accounts))
}

// ---------------------------------------------------------------------------
// REST API router
// ---------------------------------------------------------------------------

type SharedState = Arc<Mutex<AppState>>;

fn build_router(state: SharedState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/block/latest", get(block_latest_handler))
        .route("/block/{height}", get(block_by_height_handler))
        .route("/block/hash/{hash}", get(block_by_hash_handler))
        .route("/balance/{address}", get(balance_handler))
        .route("/height", get(height_handler))
        .route("/era", get(era_handler))
        .route("/genesis", get(genesis_handler))
        .route("/nonce/{address}", get(nonce_handler))
        .route("/validators", get(validators_handler))
        .route("/validator/{address}", get(validator_handler))
        .route("/tx", axum::routing::post(tx_submit_handler))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct HealthResponse {
    status: String,
    height: u64,
}

#[derive(serde::Serialize)]
struct HeightResponse {
    height: u64,
}

#[derive(serde::Serialize)]
struct EraResponse {
    era: u64,
}

#[derive(serde::Serialize)]
struct BalanceResponse {
    address: String,
    balance: String,
    nonce: u64,
}

#[derive(serde::Serialize)]
struct NonceResponse {
    nonce: u64,
}

#[derive(serde::Serialize)]
struct ValidatorResponse {
    address: String,
    exists: bool,
    status: String,
    stake: String,
}

#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
}

fn err_response(code: u16, msg: String) -> axum::response::Response {
    let body = serde_json::to_string(&ErrorResponse { error: msg }).unwrap();
    axum::response::Response::builder()
        .status(code)
        .body(axum::body::Body::from(body))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn health_handler(State(state): State<SharedState>) -> Json<HealthResponse> {
    let guard = state.lock().await;
    Json(HealthResponse {
        status: "ok".to_string(),
        height: guard.current_height,
    })
}

async fn height_handler(State(state): State<SharedState>) -> Json<HeightResponse> {
    let guard = state.lock().await;
    Json(HeightResponse {
        height: guard.current_height,
    })
}

async fn era_handler(State(state): State<SharedState>) -> Json<EraResponse> {
    let guard = state.lock().await;
    Json(EraResponse {
        era: era::era_at_height(guard.current_height),
    })
}

#[derive(serde::Serialize)]
struct GenesisResponse {
    genesis_hash: String,
}

async fn genesis_handler(State(state): State<SharedState>) -> Json<GenesisResponse> {
    let guard = state.lock().await;
    let key = 0u64.to_be_bytes();
    let hash = if let Ok(Some(raw)) = guard.engine.get(tables::BLOCKS, &key) {
        if let Ok(block) = Block::decode(&mut &raw[..]) {
            let h = blake3::hash(&parity_scale_codec::Encode::encode(&block.header));
            hex::encode(h.as_bytes())
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    Json(GenesisResponse { genesis_hash: hash })
}

#[derive(serde::Serialize)]
struct ValidatorsListResponse {
    validators: Vec<ValidatorInfo>,
}

#[derive(serde::Serialize)]
struct ValidatorInfo {
    address: String,
    stake: String,
}

async fn validators_handler(State(state): State<SharedState>) -> Json<ValidatorsListResponse> {
    let guard = state.lock().await;
    let keys = guard.engine.list_keys(tables::VALIDATORS).unwrap_or_default();
    let mut validators = Vec::new();
    for key in &keys {
        if key.len() != 32 {
            continue;
        }
        if let Ok(Some(raw)) = guard.engine.get(tables::VALIDATORS, key) {
            if let Ok(entry) = mononium_lib::core::validator::ValidatorEntry::decode(&mut &raw[..]) {
                validators.push(ValidatorInfo {
                    address: hex::encode(&key[..]),
                    stake: format!("{:#x}", entry.stake),
                });
            }
        }
    }
    Json(ValidatorsListResponse { validators })
}

async fn block_latest_handler(
    State(state): State<SharedState>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let guard = state.lock().await;
    if guard.current_height == 0 {
        return Err(err_response(404, "no blocks yet".to_string()));
    }
    let block = load_block_json(&*guard.engine, guard.current_height)
        .map_err(|e| err_response(500, format!("{e}")))?;
    Ok(Json(block))
}

async fn block_by_height_handler(
    State(state): State<SharedState>,
    axum::extract::Path(height): axum::extract::Path<u64>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let guard = state.lock().await;
    let block = load_block_json(&*guard.engine, height)
        .map_err(|_| err_response(404, format!("block {height} not found")))?;
    Ok(Json(block))
}

async fn block_by_hash_handler(
    State(state): State<SharedState>,
    axum::extract::Path(hash_hex): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, axum::response::Response> {
    let s = hash_hex.trim_start_matches("0x");
    let hash = hex::decode(s).map_err(|_| err_response(400, "invalid hex hash".to_string()))?;
    if hash.len() != 32 {
        return Err(err_response(400, "hash must be 32 bytes".to_string()));
    }
    let guard = state.lock().await;
    let keys = guard.engine.list_keys(tables::BLOCKS).map_err(|_| err_response(500, "storage error".to_string()))?;
    for key in &keys {
        if key.len() != 8 {
            continue;
        }
        if let Ok(Some(raw)) = guard.engine.get(tables::BLOCKS, key) {
            if let Ok(block) = Block::decode(&mut &raw[..]) {
                let block_hash = blake3::hash(&parity_scale_codec::Encode::encode(&block.header));
                if block_hash.as_bytes() == &hash[..] {
                    let json = serde_json::to_value(&block).unwrap();
                    return Ok(Json(json));
                }
            }
        }
    }
    Err(err_response(404, "block not found by hash".to_string()))
}

async fn balance_handler(
    State(state): State<SharedState>,
    axum::extract::Path(address_str): axum::extract::Path<String>,
) -> Result<Json<BalanceResponse>, axum::response::Response> {
    let guard = state.lock().await;

    let raw = parse_raw_address(&address_str)
        .map_err(|_| err_response(400, format!("invalid address: {address_str}")))?;
    let addr = Address::from(raw);

    match guard.state.get_account(&addr) {
        Some(acct) => Ok(Json(BalanceResponse {
            address: address_str,
            balance: acct.balance.to_string(),
            nonce: acct.nonce,
        })),
        None => Ok(Json(BalanceResponse {
            address: address_str,
            balance: "0".to_string(),
            nonce: 0,
        })),
    }
}

async fn nonce_handler(
    State(state): State<SharedState>,
    axum::extract::Path(address_str): axum::extract::Path<String>,
) -> Result<Json<NonceResponse>, axum::response::Response> {
    let guard = state.lock().await;
    let raw = parse_raw_address(&address_str)
        .map_err(|_| err_response(400, format!("invalid address: {address_str}")))?;
    let addr = Address::from(raw);
    let nonce = guard.state.get_account(&addr).map(|a| a.nonce).unwrap_or(0);
    Ok(Json(NonceResponse { nonce }))
}

async fn validator_handler(
    State(state): State<SharedState>,
    axum::extract::Path(address_str): axum::extract::Path<String>,
) -> Result<Json<ValidatorResponse>, axum::response::Response> {
    let guard = state.lock().await;
    let raw = parse_raw_address(&address_str)
        .map_err(|_| err_response(400, format!("invalid address: {address_str}")))?;
    let addr = Address::from(raw);
    match guard.state.get_validator(&addr) {
        Some(v) => Ok(Json(ValidatorResponse {
            address: address_str,
            exists: true,
            status: format!("{:?}", v.status),
            stake: v.stake.to_string(),
        })),
        None => Ok(Json(ValidatorResponse {
            address: address_str,
            exists: false,
            status: "unknown".to_string(),
            stake: "0".to_string(),
        })),
    }
}

// ---------------------------------------------------------------------------
// Tx submit handler
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct TxSubmitResponse {
    tx_hash: String,
    status: String,
}

async fn tx_submit_handler(
    State(state): State<SharedState>,
    axum::extract::Json(tx): axum::extract::Json<Transaction>,
) -> Result<Json<TxSubmitResponse>, axum::response::Response> {
    let encoded = parity_scale_codec::Encode::encode(&tx);
    let hash = mononium_lib::crypto::hash::blake3_hash(&encoded);
    let tx_hash = hex::encode(&hash[..8]);

    let mut guard = state.lock().await;
    match guard.mempool.insert(tx) {
        Ok(()) => {
            tracing::info!(tx_hash = %tx_hash, "tx added to mempool");
            Ok(Json(TxSubmitResponse {
                tx_hash,
                status: "in_mempool".to_string(),
            }))
        }
        Err(e) => {
            tracing::warn!(tx_hash = %tx_hash, error = %e, "tx rejected from mempool");
            Err(err_response(400, format!("tx rejected: {e}")))
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Parse a hex address string (with or without `0x`) into 32 raw bytes.
fn parse_raw_address(s: &str) -> std::result::Result<[u8; 32], LibError> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).map_err(|_| LibError::InvalidAddress(s.to_string()))?;
    if bytes.len() != 32 {
        return Err(LibError::InvalidAddress(s.to_string()));
    }
    let mut raw = [0u8; 32];
    raw.copy_from_slice(&bytes);
    Ok(raw)
}

/// Load a block from storage, decode from SCALE, convert to JSON.
fn load_block_json(engine: &dyn StorageEngine, height: u64) -> std::result::Result<serde_json::Value, LibError> {
    let key = height.to_be_bytes();
    let raw = engine
        .get(tables::BLOCKS, &key)?
        .ok_or(LibError::BlockNotFound(height))?;
    let block: Block = parity_scale_codec::Decode::decode(&mut &raw[..])
        .map_err(|e| LibError::Codec(format!("block decode: {e}")))?;
    serde_json::to_value(&block).map_err(|e| LibError::Codec(format!("block JSON: {e}")))
}

// ---------------------------------------------------------------------------
// Block production loop
// ---------------------------------------------------------------------------

async fn block_production_loop(state: SharedState) {
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
    // Ticks are immediate by default; consume the first one so we don't
    // produce block 1 at t=0.
    interval.tick().await;

    loop {
        interval.tick().await;
        let mut guard = state.lock().await;
        let height = guard.current_height + 1;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Select transactions from mempool
        let txs = guard.mempool.select(500);
        if !txs.is_empty() {
            tracing::info!(count = txs.len(), "including txs from mempool");
        }

        let block = Block {
            header: BlockHeader {
                height,
                parent_hash: [0u8; 32],
                global_state_root: [0u8; 32],
                tx_root: [0u8; 32],
                timestamp: now,
                proposer: Address::from([0u8; 32]),
                chain_id: 0,
                proposer_signature: mononium_lib::crypto::falcon::Falcon512Signature::from_bytes(
                    &[0xCD; mononium_lib::crypto::constants::FALCON_SIGNATURE_SIZE],
                ).unwrap(),
            },
            body: BlockBody {
                transactions: txs,
            },
        };

        // Apply to state machine (updates state root, validates txs)
        let _receipt = guard.state.apply_block(&block);

        // Store in DB
        let key = height.to_be_bytes();
        let encoded = parity_scale_codec::Encode::encode(&block);
        if let Err(e) = guard.engine.put(tables::BLOCKS, &key, &encoded) {
            tracing::error!(%height, error = %e, "failed to store block");
            continue;
        }

        guard.current_height = height;
        tracing::info!(height, timestamp = now, "block produced");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use mononium_lib::storage::StorageEngine;
    use mononium_lib::storage::redb::RedbEngine;

    #[test]
    fn test_parse_raw_address_valid() {
        let addr = [0xABu8; 32];
        let hex = hex::encode(addr);
        let result = parse_raw_address(&hex).unwrap();
        assert_eq!(result, addr);
    }

    #[test]
    fn test_parse_raw_address_with_0x() {
        let addr = [0x42u8; 32];
        let hex = format!("0x{}", hex::encode(addr));
        let result = parse_raw_address(&hex).unwrap();
        assert_eq!(result, addr);
    }

    #[test]
    fn test_parse_raw_address_wrong_length() {
        let err = parse_raw_address("aabb").unwrap_err();
        assert!(err.to_string().contains("invalid address"), "got: {err}");
    }

    #[test]
    fn test_parse_raw_address_invalid_hex() {
        let err = parse_raw_address(&"z".repeat(64)).unwrap_err();
        assert!(err.to_string().contains("invalid address"), "got: {err}");
    }

    /// Helper: create a temporary RedbEngine for storage tests.
    fn setup_engine() -> RedbEngine {
        let dir = tempfile::TempDir::with_prefix("mononium-node-test-").unwrap();
        RedbEngine::open(&dir.path().join("test.redb")).unwrap()
    }

    #[test]
    fn test_load_latest_height_empty_db() {
        let engine = setup_engine();
        let height = load_latest_height(&engine).unwrap();
        assert_eq!(height, 0);
    }

    #[test]
    fn test_load_latest_height_with_block() {
        let engine = setup_engine();
        // Store a block at height 5 (keys are big-endian u64)
        let key = 5u64.to_be_bytes();
        let block = Block {
            header: BlockHeader {
                height: 5, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: Address::from([0xAAu8; 32]), chain_id: 0,
                proposer_signature: mononium_lib::crypto::falcon::Falcon512Signature::from_bytes(
                    &[0xCD; mononium_lib::crypto::constants::FALCON_SIGNATURE_SIZE],
                ).unwrap(),
            },
            body: BlockBody { transactions: vec![] },
        };
        engine.put(tables::BLOCKS, &key, &parity_scale_codec::Encode::encode(&block)).unwrap();
        let height = load_latest_height(&engine).unwrap();
        assert_eq!(height, 5);
    }

    #[test]
    fn test_load_block_json_not_found() {
        let engine = setup_engine();
        let err = load_block_json(&engine, 99).unwrap_err();
        assert!(err.to_string().contains("block not found") || err.to_string().contains("BlockNotFound"), "got: {err}");
    }

    #[test]
    fn test_load_block_json_roundtrip() {
        let engine = setup_engine();
        let block = Block {
            header: BlockHeader {
                height: 1,
                parent_hash: [0u8; 32],
                global_state_root: [0u8; 32],
                tx_root: [0u8; 32],
                timestamp: 1_700_000_000,
                proposer: Address::from([0xAAu8; 32]),
                chain_id: 0,
                proposer_signature: mononium_lib::crypto::falcon::Falcon512Signature::from_bytes(
                    &[0xCD; mononium_lib::crypto::constants::FALCON_SIGNATURE_SIZE],
                ).unwrap(),
            },
            body: BlockBody { transactions: vec![] },
        };
        let key = 1u64.to_be_bytes();
        let encoded = parity_scale_codec::Encode::encode(&block);
        engine.put(tables::BLOCKS, &key, &encoded).unwrap();
        let json = load_block_json(&engine, 1).unwrap();
        assert_eq!(json["header"]["height"], 1);
    }

    #[test]
    fn test_load_state_from_storage_empty() {
        let engine = setup_engine();
        let mut sm = load_state_from_storage(&engine).unwrap();
        // Empty SMT root is the precomputed default (not all zeros)
        let root = sm.state_root();
        assert_ne!(root, [0u8; 32]);
        // No accounts
        assert!(sm.get_account(&Address::from([0x01u8; 32])).is_none());
    }

    // -----------------------------------------------------------------------
    // Crash recovery
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_state_consistency_genesis_height_skips() {
        let engine = setup_engine();
        let mut sm = load_state_from_storage(&engine).unwrap();
        // Height 0 should always pass (no previous block to verify)
        verify_state_consistency(&mut sm, &engine, 0).unwrap();
    }

    #[test]
    fn test_verify_state_consistency_matching_root() {
        let engine = setup_engine();
        let mut sm = load_state_from_storage(&engine).unwrap();
        let state_root = sm.state_root();

        // Insert a block with the correct state root
        let block = Block {
            header: BlockHeader {
                height: 5,
                parent_hash: [0u8; 32],
                global_state_root: state_root,
                tx_root: [0u8; 32],
                timestamp: 1_700_000_000,
                proposer: Address::from([0xAAu8; 32]),
                chain_id: 0,
                proposer_signature: mononium_lib::crypto::falcon::Falcon512Signature::from_bytes(
                    &[0xCD; mononium_lib::crypto::constants::FALCON_SIGNATURE_SIZE],
                ).unwrap(),
            },
            body: BlockBody { transactions: vec![] },
        };
        let key = 5u64.to_be_bytes();
        engine.put(tables::BLOCKS, &key, &parity_scale_codec::Encode::encode(&block)).unwrap();

        verify_state_consistency(&mut sm, &engine, 5).unwrap();
    }

    #[test]
    fn test_verify_state_consistency_mismatch_fails() {
        let engine = setup_engine();
        let mut sm = load_state_from_storage(&engine).unwrap();

        // Insert a block with a WRONG state root
        let block = Block {
            header: BlockHeader {
                height: 3,
                parent_hash: [0u8; 32],
                global_state_root: [0xFFu8; 32], // wrong!
                tx_root: [0u8; 32],
                timestamp: 1_700_000_000,
                proposer: Address::from([0xAAu8; 32]),
                chain_id: 0,
                proposer_signature: mononium_lib::crypto::falcon::Falcon512Signature::from_bytes(
                    &[0xCD; mononium_lib::crypto::constants::FALCON_SIGNATURE_SIZE],
                ).unwrap(),
            },
            body: BlockBody { transactions: vec![] },
        };
        let key = 3u64.to_be_bytes();
        engine.put(tables::BLOCKS, &key, &parity_scale_codec::Encode::encode(&block)).unwrap();

        let err = verify_state_consistency(&mut sm, &engine, 3).unwrap_err();
        assert!(err.to_string().contains("state root mismatch"), "got: {err}");
    }

    #[test]
    fn test_verify_state_consistency_missing_block_fails() {
        let engine = setup_engine();
        let mut sm = load_state_from_storage(&engine).unwrap();
        let err = verify_state_consistency(&mut sm, &engine, 42).unwrap_err();
        assert!(err.to_string().contains("not found"), "got: {err}");
    }
}
