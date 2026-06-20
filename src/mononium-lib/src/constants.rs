//! Chain-wide protocol constants.
//!
//! These are shared across all modules (core, consensus, storage, etc.).
//! Module-specific constants live in their respective `constants.rs` files.

/// Maximum block size in bytes (hard cap enforced at gossip and consensus).
pub const MAX_BLOCK_SIZE_BYTES: u32 = 500_000; // 500 KB

/// Maximum number of transactions per block.
pub const MAX_TX_PER_BLOCK: u32 = 500;

/// Default P2P port (libp2p).
pub const DEFAULT_P2P_PORT: u16 = 30333;

/// Default REST API port (axum HTTP).
pub const DEFAULT_REST_PORT: u16 = 9933;

/// Default JSON-RPC WebSocket port (jsonrpsee).
pub const DEFAULT_RPC_PORT: u16 = 9944;

/// Key file directory name under `~/.mononium/`.
pub const KEYS_DIR: &str = "keys";

/// Data directory name under `~/.mononium/`.
pub const DATA_DIR: &str = "data";

/// Default config file name (YAML).
pub const CONFIG_FILE_YAML: &str = "config.yaml";

/// Default config file name (TOML).
pub const CONFIG_FILE_TOML: &str = "config.toml";

/// Default genesis file name localnet.
pub const GENESIS_LOCALNET: &str = "configs/genesis.localnet.json";

/// Default genesis file name devnet.
pub const GENESIS_DEVNET: &str = "configs/genesis.devnet.json";

/// Chain ID for localnet (single-node development).
pub const CHAIN_ID_LOCALNET: u64 = 0;

/// Chain ID for devnet (multi-validator testing).
pub const CHAIN_ID_DEVNET: u64 = 1;

/// Chain ID for testnet (public test network).
pub const CHAIN_ID_TESTNET: u64 = 2;

/// Chain ID for mainnet (production).
pub const CHAIN_ID_MAINNET: u64 = 3;

/// Default block time in seconds.
pub const DEFAULT_BLOCK_TIME_SEC: u64 = 5;

/// Genesis block height.
pub const GENESIS_HEIGHT: u64 = 0;
