//! Default configuration constants for node operation.

use std::path::PathBuf;

/// Default P2P libp2p port.
pub const DEFAULT_P2P_PORT: u16 = 30333;

/// Default JSON-RPC WebSocket port.
pub const DEFAULT_RPC_PORT: u16 = 9944;

/// Default REST HTTP port.
pub const DEFAULT_REST_PORT: u16 = 9933;

/// Default data directory (under home).
pub fn default_data_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".mononium").join("data")
}

/// Default key directory (under home).
pub fn default_key_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".mononium").join("keys")
}

/// Default config directory (under home).
pub fn default_config_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".mononium")
}

/// Default memory in MiB for Argon2id key derivation.
pub const DEFAULT_ARGON2_MEMORY_MIB: u32 = 256;

/// Default iterations for Argon2id key derivation.
pub const DEFAULT_ARGON2_ITERATIONS: u32 = 16;

/// Default unlock timeout in seconds.
pub const DEFAULT_UNLOCK_TIMEOUT_SECS: u64 = 20;

/// Default mempool min fee (0.0667 MONEX in MOXX).
/// This is 667_000_000_000_000_000_000_000_000_000_000 as parsed from the plan.
/// Actually stored as U256 in core::constants::DEFAULT_MIN_MEMPOOL_FEE.
/// Here we store the MONEX f64 value for config display purposes.
pub const DEFAULT_MIN_FEE_MONEX: f64 = 0.0667;

/// Default max txs per account per block.
pub const DEFAULT_MAX_TX_PER_ACCOUNT: usize = 50;

/// Default storage mode.
pub const DEFAULT_STORAGE_MODE: &str = "full";

/// Default compact eras retention.
pub const DEFAULT_COMPACT_ERAS: u32 = 2;
