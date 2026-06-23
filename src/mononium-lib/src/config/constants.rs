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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_data_dir_returns_path() {
        let path = default_data_dir();
        assert!(path.to_string_lossy().contains(".mononium"));
        assert!(path.to_string_lossy().contains("data"));
    }

    #[test]
    fn test_default_key_dir_returns_path() {
        let path = default_key_dir();
        assert!(path.to_string_lossy().contains(".mononium"));
        assert!(path.to_string_lossy().contains("keys"));
    }

    #[test]
    fn test_default_config_dir_returns_path() {
        let path = default_config_dir();
        assert!(path.to_string_lossy().contains(".mononium"));
        assert!(!path.to_string_lossy().contains("data"));
        assert!(!path.to_string_lossy().contains("keys"));
    }

    #[test]
    fn test_default_ports() {
        assert_eq!(DEFAULT_P2P_PORT, 30333);
        assert_eq!(DEFAULT_RPC_PORT, 9944);
        assert_eq!(DEFAULT_REST_PORT, 9933);
    }

    #[test]
    fn test_default_crypto_params() {
        assert_eq!(DEFAULT_ARGON2_MEMORY_MIB, 256);
        assert_eq!(DEFAULT_ARGON2_ITERATIONS, 16);
    }

    #[test]
    fn test_default_storage_params() {
        assert_eq!(DEFAULT_STORAGE_MODE, "full");
        assert_eq!(DEFAULT_COMPACT_ERAS, 2);
        assert_eq!(DEFAULT_MAX_TX_PER_ACCOUNT, 50);
        assert_eq!(DEFAULT_UNLOCK_TIMEOUT_SECS, 20);
    }

    #[test]
    fn test_default_min_fee() {
        assert!((DEFAULT_MIN_FEE_MONEX - 0.0667).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_dirs_contain_mononium() {
        let data = default_data_dir();
        let keys = default_key_dir();
        let cfg = default_config_dir();
        assert!(data.to_string_lossy().contains(".mononium/data"));
        assert!(keys.to_string_lossy().contains(".mononium/keys"));
        assert!(cfg.to_string_lossy().contains(".mononium"));
        assert!(!cfg.to_string_lossy().contains("data"));
    }

    #[test]
    fn test_default_min_fee_monex_positive() {
        assert!(DEFAULT_MIN_FEE_MONEX > 0.0);
    }
}
