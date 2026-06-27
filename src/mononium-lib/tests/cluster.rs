//! Multi-validator cluster integration test harness.
//!
//! Uses real libp2p, real RedbEngine, real StateMachine, and the real
//! ConsensusEngine to run N validators that produce blocks in-process.
//!
//! # Design
//!
//! Each [`ClusterNode`] owns a temp data dir, a RedbEngine, a StateMachine,
//! a Mempool, a P2pHandle, and a ConsensusEngine.  The [`Cluster`] struct
//! manages N nodes, wires up networking, and provides `advance(height)` to
//! wait until all nodes have reached a target block height.
//!
//! Block time is set to 100 ms so tests complete in seconds, not minutes.

use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use mononium_lib::consensus::engine::{ConsensusEngine, LocalValidatorKey};
use mononium_lib::consensus::era;
use mononium_lib::consensus::proposer::ProposerSchedule;
use mononium_lib::consensus::ConsensusConfig;
use mononium_lib::core::account::{Account, Address};
use mononium_lib::core::block::{Block, BlockBody, BlockHeader};
use mononium_lib::core::state::StateMachine;
use mononium_lib::core::transaction::{Transaction, TxBody};
use mononium_lib::crypto::constants::FALCON_SIGNATURE_SIZE;
use mononium_lib::crypto::falcon::{Falcon512PublicKey, Falcon512Signature};
use mononium_lib::mempool::{Mempool, MempoolConfig};
use mononium_lib::network::{P2pConfig, P2pHandle, P2pService};
use mononium_lib::storage::genesis::load_genesis;
use mononium_lib::storage::redb::RedbEngine;
use mononium_lib::storage::tables;
use mononium_lib::storage::StorageEngine;

use parity_scale_codec::Decode;
use primitive_types::U256;
use tempfile::TempDir;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Number of validators in the default cluster.
const N_VALIDATORS: usize = 3;

/// Block time (100 ms) for fast test execution.
const BLOCK_TIME_MS: u64 = 100;

/// Wait timeout for cluster advancement.
const ADVANCE_TIMEOUT_SECS: u64 = 60;

/// Fee per RegisterValidator transaction.
const REGISTER_FEE: u128 = 100;

/// Balance per validator (enough for deposit + fee).
const VALIDATOR_BALANCE_STR: &str = "10000000000000000000000000000000000";

/// Number of blocks each node must produce before the basic progress
/// assertion passes.
const PROGRESS_TARGET_HEIGHT: u64 = 3;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find a currently unused TCP port by binding to port 0.
fn pick_unused_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// Deterministic 32-byte address for the n-th validator (0-indexed).
fn validator_addr(n: usize) -> Address {
    let mut bytes = [0u8; 32];
    bytes[0] = n as u8;
    Address::from(bytes)
}

/// Deterministic Falcon-512 public key for the n-th validator.
fn validator_pubkey(n: usize) -> Falcon512PublicKey {
    let mut data = [1u8; 897];
    data[0] = (n + 1) as u8;
    Falcon512PublicKey(data)
}

/// Deterministic fake signature used in test blocks.
fn fake_sig() -> Falcon512Signature {
    Falcon512Signature::from_bytes(&[0xCDu8; FALCON_SIGNATURE_SIZE]).expect("fake signature")
}

/// Default mempool config for integration tests.
fn test_mempool_config() -> MempoolConfig {
    MempoolConfig {
        max_size: 10_000,
        ttl: Duration::from_secs(600),
        min_fee: U256::zero(),
        per_sender_cap: 50,
    }
}

/// Read the highest block height from storage.
fn read_height(storage: &RedbEngine) -> u64 {
    let keys = storage.list_keys(tables::BLOCKS).unwrap_or_default();
    keys.iter()
        .filter_map(|k| {
            let arr: [u8; 8] = k.as_slice().try_into().ok()?;
            Some(u64::from_be_bytes(arr))
        })
        .max()
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// ClusterNode
// ---------------------------------------------------------------------------

/// A single validator node in the cluster.
struct ClusterNode {
    /// Shared storage engine — keeps the database alive and allows height
    /// queries while the consensus loop runs in the background.
    _storage: Arc<RedbEngine>,
    /// Handle to the running P2P service.
    _p2p: P2pHandle,
    /// Consensus loop task handle.
    _consensus_task: tokio::task::JoinHandle<()>,
}

impl ClusterNode {
    /// Create a new cluster node.
    #[allow(clippy::too_many_arguments)]
    fn new(
        data_dir: TempDir,
        chain_id: u64,
        genesis_json_path: &std::path::Path,
        validator_addrs: &[Address],
        validator_pubkeys: &[[u8; 897]],
        my_index: usize,
        p2p_port: u16,
        bootstrap_addr: Option<&str>,
    ) -> Self {
        // ── Storage ──────────────────────────────────────────────────
        let db_path = data_dir.path().join("node.redb");
        let engine = RedbEngine::open(&db_path).expect("open redb engine");
        let storage = Arc::new(engine);

        // Load genesis (populates ACCOUNTS, VALIDATORS, BLOCKS, META)
        load_genesis(&*storage, genesis_json_path).expect("load genesis");

        // ── State machine built from genesis accounts ────────────────
        let sm = Self::build_state_machine(&*storage, validator_addrs, validator_pubkeys);
        let state = Arc::new(RwLock::new(sm));

        // ── Mempool ──────────────────────────────────────────────────
        let mempool = Arc::new(RwLock::new(Mempool::new(test_mempool_config())));

        // ── P2P service ──────────────────────────────────────────────
        let p2p_config = P2pConfig {
            p2p_port,
            bootstrap_peers: bootstrap_addr
                .map(|a| a.parse().expect("valid multiaddr"))
                .into_iter()
                .collect(),
            enable_mdns: false,
            max_peers: N_VALIDATORS,
        };
        let p2p_service = P2pService::new(p2p_config, chain_id).expect("create P2pService");
        let p2p_service = p2p_service.with_storage(storage.clone(), [0u8; 32]);
        let p2p = p2p_service.start().expect("start P2pService");

        // ── Consensus engine ─────────────────────────────────────────
        let mut engine = ConsensusEngine::new(ConsensusConfig::new(
            Duration::from_millis(BLOCK_TIME_MS),
            era::ERA_LENGTH,
            N_VALIDATORS,
        ));
        engine.set_local_validator(LocalValidatorKey {
            address: validator_addrs[my_index],
        });

        let schedule = ProposerSchedule::new(
            validator_addrs.to_vec(),
            0, // era 0
            1, // start at height 1
        );
        engine.set_schedule(schedule);

        // ── Spawn consensus loop ─────────────────────────────────────
        let consensus_p2p = p2p.clone();
        let consensus_storage = storage.clone();

        let consensus_task = tokio::spawn(async move {
            engine
                .start_consensus_loop(
                    state,
                    mempool,
                    &consensus_p2p,
                    &*consensus_storage,
                    [0u8; 32],
                )
                .await;
        });

        Self {
            _storage: storage,
            _p2p: p2p,
            _consensus_task: consensus_task,
        }
    }

    /// Build a StateMachine from genesis storage and register all validators.
    fn build_state_machine(
        storage: &RedbEngine,
        validator_addrs: &[Address],
        validator_pubkeys: &[[u8; 897]],
    ) -> StateMachine {
        // Read all accounts from storage
        let account_keys = storage
            .list_keys(tables::ACCOUNTS)
            .expect("list account keys");
        let mut initial_accounts = Vec::with_capacity(account_keys.len());
        for key in &account_keys {
            let raw: [u8; 32] = {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(key);
                arr
            };
            let addr = Address::from(raw);
            if let Ok(Some(encoded)) = storage.get(tables::ACCOUNTS, key) {
                if let Ok(acct) = Account::decode(&mut &encoded[..]) {
                    initial_accounts.push((addr, acct));
                }
            }
        }

        let sm = StateMachine::new(initial_accounts);
        Self::register_validators(sm, validator_addrs, validator_pubkeys)
    }

    /// Register validators by applying RegisterValidator transactions.
    fn register_validators(
        mut sm: StateMachine,
        addrs: &[Address],
        pubkeys: &[[u8; 897]],
    ) -> StateMachine {
        for (i, addr) in addrs.iter().enumerate() {
            let acct = sm
                .get_account(addr)
                .unwrap_or_else(|| panic!("validator {i} account must exist from genesis"));
            let nonce = acct.nonce;

            let tx = Transaction {
                chain_id: 0,
                nonce,
                sender: *addr,
                fee: U256::from(REGISTER_FEE),
                body: TxBody::RegisterValidator {
                    public_key: pubkeys[i],
                },
                signature: fake_sig(),
            };

            let block = Block {
                header: BlockHeader {
                    height: (i + 1) as u64,
                    parent_hash: [0u8; 32],
                    global_state_root: [0u8; 32],
                    tx_root: [0u8; 32],
                    timestamp: 1_700_000_000 + i as u64,
                    proposer: *addr,
                    chain_id: 0,
                    proposer_signature: fake_sig(),
                },
                body: BlockBody {
                    transactions: vec![tx],
                },
            };

            sm.apply_block(&block)
                .unwrap_or_else(|e| panic!("apply register validator block for {i}: {e}"));

            assert!(
                sm.get_validator(addr).is_some(),
                "validator {i} should be registered after apply_block"
            );
        }

        sm.set_active_set(addrs.to_vec());
        sm
    }

    /// Read the current chain height from the BLOCKS table.
    fn height(&self) -> u64 {
        read_height(&*self._storage)
    }
}

// ---------------------------------------------------------------------------
// Cluster
// ---------------------------------------------------------------------------

/// Manages N validator nodes and provides advance/wait coordination.
struct Cluster {
    nodes: Vec<ClusterNode>,
    _genesis_dir: TempDir,
}

impl Cluster {
    /// Create a new cluster with `count` validator nodes.
    async fn new(count: usize) -> Self {
        let chain_id: u64 = 0;

        // Deterministic addresses and raw public key bytes
        let addrs: Vec<Address> = (0..count).map(validator_addr).collect();
        let pubkeys: Vec<[u8; 897]> = (0..count).map(|n| validator_pubkey(n).0).collect();

        // Write genesis JSON with all validator accounts
        let genesis_dir = TempDir::with_prefix("mononium-cluster-genesis-").unwrap();
        let genesis_path = genesis_dir.path().join("genesis.json");
        Self::write_genesis_json(&genesis_path, chain_id, &addrs);

        // Allocate P2P ports for all nodes
        let ports: Vec<u16> = (0..count).map(|_| pick_unused_port()).collect();
        let bootstrap_addr = format!("/ip4/127.0.0.1/tcp/{}", ports[0]);

        let mut nodes = Vec::with_capacity(count);
        for i in 0..count {
            let node_dir = TempDir::with_prefix(&format!("mononium-node{i}-")).unwrap();
            let bootstrap = if i == 0 {
                None
            } else {
                Some(bootstrap_addr.as_str())
            };

            let node = ClusterNode::new(
                node_dir,
                chain_id,
                &genesis_path,
                &addrs,
                &pubkeys,
                i,
                ports[i],
                bootstrap,
            );
            nodes.push(node);
        }

        Self {
            nodes,
            _genesis_dir: genesis_dir,
        }
    }

    /// Write a genesis JSON file with validator accounts and no validators.
    fn write_genesis_json(path: &std::path::Path, chain_id: u64, addrs: &[Address]) {
        let mut initial_accounts = serde_json::Map::new();
        for addr in addrs {
            initial_accounts.insert(
                hex::encode(addr.as_bytes()),
                serde_json::Value::String(VALIDATOR_BALANCE_STR.to_string()),
            );
        }

        let genesis = serde_json::json!({
            "chain_id": chain_id,
            "genesis_time": "2026-06-25T00:00:00Z",
            "initial_accounts": initial_accounts,
            "initial_validators": [],
        });

        std::fs::write(path, serde_json::to_string_pretty(&genesis).unwrap()).unwrap();
    }

    /// Wait until all nodes have reached at least `min_height`.
    async fn advance(&self, min_height: u64, timeout: Duration) {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if std::time::Instant::now() > deadline {
                let heights: Vec<u64> = self.nodes.iter().map(|n| n.height()).collect();
                panic!(
                    "timed out waiting for all nodes to reach height >= {min_height}, \
                     current heights: {heights:?}"
                );
            }

            if self.nodes.iter().all(|n| n.height() >= min_height) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Get the current height of each node.
    #[allow(dead_code)]
    fn heights(&self) -> Vec<u64> {
        self.nodes.iter().map(|n| n.height()).collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_3_validators_produce_blocks() {
    let cluster = Cluster::new(N_VALIDATORS).await;

    // Allow time for P2P discovery and initial block production.
    cluster
        .advance(
            PROGRESS_TARGET_HEIGHT,
            Duration::from_secs(ADVANCE_TIMEOUT_SECS),
        )
        .await;

    let heights: Vec<u64> = cluster.nodes.iter().map(|n| n.height()).collect();
    eprintln!("heights after advance({PROGRESS_TARGET_HEIGHT}): {heights:?}");

    for (i, h) in heights.iter().enumerate() {
        assert!(
            *h >= PROGRESS_TARGET_HEIGHT,
            "node {i} height {h} < {PROGRESS_TARGET_HEIGHT}"
        );
    }
}
