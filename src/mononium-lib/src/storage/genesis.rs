//! Genesis block configuration and loader.
//!
//! Reads a genesis JSON file and populates the storage engine with initial
//! accounts, validators, and chain metadata. Idempotent if already loaded
//! (detected via `META` table marker).

use std::path::Path;

use primitive_types::U256;
use serde::Deserialize;
use parity_scale_codec::Encode;

use crate::core::account::Account;
use crate::error::{LibError, Result};
use crate::storage::tables;
use crate::storage::StorageEngine;

/// Fallible result alias used internally by the genesis module.
type IoResult<T> = std::result::Result<T, LibError>;

/// Top-level genesis configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct GenesisConfig {
    /// Chain identifier (must match `block.header.chain_id`).
    pub chain_id: u64,
    /// Human-readable genesis time (informational only).
    pub genesis_time: String,
    /// Initial account balances: `{address_hex} → {balance_decimal_string}`.
    pub initial_accounts: std::collections::HashMap<String, String>,
    /// Initial validator set.
    #[serde(default)]
    pub initial_validators: Vec<GenesisValidator>,
}

/// A single validator entry in the genesis config.
#[derive(Debug, Clone, Deserialize)]
pub struct GenesisValidator {
    /// Hex-encoded 32-byte address.
    pub address: String,
    /// Initial stake as a decimal string.
    pub stake: String,
}

/// Parse a decimal string into `U256`.
fn parse_u256(s: &str) -> IoResult<U256> {
    U256::from_dec_str(s)
        .map_err(|e| LibError::Storage(format!("invalid decimal U256 '{s}': {e}")))
}

/// Parse a hex address string (with or without `0x` prefix) into raw 32 bytes.
fn parse_hex_addr(s: &str) -> IoResult<[u8; 32]> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s)
        .map_err(|e| LibError::Storage(format!("invalid hex address '{s}': {e}")))?;
    if bytes.len() != 32 {
        return Err(LibError::Storage(format!(
            "address must be 32 bytes, got {}",
            bytes.len()
        )));
    }
    let mut raw = [0u8; 32];
    raw.copy_from_slice(&bytes);
    Ok(raw)
}

/// Load genesis state from a JSON file.
///
/// # Errors
///
/// Returns `LibError::Storage` if:
/// - The file cannot be read or parsed.
/// - Genesis has already been loaded (duplicate detection).
/// - Account balances or validator stakes are invalid.
/// - Addresses are malformed.
pub fn load_genesis(engine: &impl StorageEngine, genesis_path: &Path) -> Result<()> {
    // ---------- duplicate detection ----------
    if engine
        .exists(tables::META, tables::GENESIS_LOADED_KEY)?
    {
        return Err(LibError::Storage(
            "genesis already loaded".to_string(),
        ));
    }

    // ---------- parse ----------
    let json_str = std::fs::read_to_string(genesis_path)
        .map_err(|e| LibError::Storage(format!("cannot read genesis file: {e}")))?;
    let config: GenesisConfig = serde_json::from_str(&json_str)
        .map_err(|e| LibError::Storage(format!("invalid genesis JSON: {e}")))?;

    // ---------- write chain_id ----------
    let chain_id_bytes = config.chain_id.to_le_bytes();
    engine.put(tables::META, tables::CHAIN_ID_KEY, &chain_id_bytes)?;

    // ---------- write initial accounts ----------
    for (addr_hex, balance_str) in &config.initial_accounts {
        let raw_addr = parse_hex_addr(addr_hex)?;
        let balance = parse_u256(balance_str)?;
        let account = Account::new(balance);
        let encoded = account.encode();
        engine.put(tables::ACCOUNTS, &raw_addr, &encoded)?;
    }

    // ---------- write initial validators ----------
    for v in &config.initial_validators {
        let raw_addr = parse_hex_addr(&v.address)?;
        let stake = parse_u256(&v.stake)?;
        // Store validator as SCALE-encoded (address_bytes ++ stake_bytes) for now;
        // a proper ValidatorEntry struct will replace this in sub-phase 1.7.
        let mut entry = raw_addr.to_vec();
        let mut stake_bytes = [0u8; 32];
        stake.to_little_endian(&mut stake_bytes);
        entry.extend_from_slice(&stake_bytes);
        engine.put(tables::VALIDATORS, &raw_addr, &entry)?;
    }

    // ---------- mark genesis loaded ----------
    engine.put(tables::META, tables::GENESIS_LOADED_KEY, b"1")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use parity_scale_codec::Decode;

    use super::*;
    use crate::storage::redb::RedbEngine;
    use tempfile::TempDir;

    fn setup_engine() -> (TempDir, RedbEngine) {
        let dir = TempDir::with_prefix("mononium-test-").unwrap();
        let db_path = dir.path().join("test.redb");
        let engine = RedbEngine::open(&db_path).unwrap();
        (dir, engine)
    }

    fn write_genesis_json(dir: &Path, json: &str) -> std::path::PathBuf {
        let path = dir.join("genesis.json");
        std::fs::write(&path, json).unwrap();
        path
    }

    /// Minimal valid genesis JSON.
    fn minimal_genesis_json() -> String {
        r#"{
            "chain_id": 0,
            "genesis_time": "2025-01-01T00:00:00Z",
            "initial_accounts": {
                "abababababababababababababababababababababababababababababababab": "1000000000000000000000000000000000"
            },
            "initial_validators": []
        }"#
        .to_string()
    }

    #[test]
    fn test_load_genesis_sets_chain_id() {
        let (dir, engine) = setup_engine();
        let path = write_genesis_json(dir.path(), &minimal_genesis_json());
        load_genesis(&engine, &path).unwrap();

        let raw = engine.get(tables::META, tables::CHAIN_ID_KEY).unwrap().unwrap();
        let chain_id = u64::from_le_bytes(raw.try_into().unwrap());
        assert_eq!(chain_id, 0);
    }

    #[test]
    fn test_load_genesis_creates_account() {
        let (dir, engine) = setup_engine();
        let path = write_genesis_json(dir.path(), &minimal_genesis_json());
        load_genesis(&engine, &path).unwrap();

        let addr_hex = "abababababababababababababababababababababababababababababababab";
        let raw_addr = parse_hex_addr(addr_hex).unwrap();
        let raw = engine.get(tables::ACCOUNTS, &raw_addr).unwrap().unwrap();
        let account = Account::decode(&mut &raw[..]).unwrap();
        assert_eq!(account.balance, U256::from_dec_str("1000000000000000000000000000000000").unwrap());
        assert_eq!(account.nonce, 0);
    }

    #[test]
    fn test_load_genesis_rejects_duplicate() {
        let (dir, engine) = setup_engine();
        let path = write_genesis_json(dir.path(), &minimal_genesis_json());
        load_genesis(&engine, &path).unwrap();

        let err = load_genesis(&engine, &path).unwrap_err();
        assert!(err.to_string().contains("genesis already loaded"), "got: {err}");
    }

    #[test]
    fn test_load_genesis_rejects_invalid_json() {
        let (dir, engine) = setup_engine();
        let path = write_genesis_json(dir.path(), "not json");
        let err = load_genesis(&engine, &path).unwrap_err();
        assert!(err.to_string().contains("invalid genesis JSON"), "got: {err}");
    }

    #[test]
    fn test_load_genesis_rejects_invalid_address() {
        let (dir, engine) = setup_engine();
        let json = r#"{
            "chain_id": 0,
            "genesis_time": "2025-01-01T00:00:00Z",
            "initial_accounts": {
                "zz": "1000"
            },
            "initial_validators": []
        }"#;
        let path = write_genesis_json(dir.path(), json);
        let err = load_genesis(&engine, &path).unwrap_err();
        assert!(err.to_string().contains("invalid hex"), "got: {err}");
    }

    #[test]
    fn test_load_genesis_rejects_invalid_balance() {
        let (dir, engine) = setup_engine();
        let json = r#"{
            "chain_id": 0,
            "genesis_time": "2025-01-01T00:00:00Z",
            "initial_accounts": {
                "abababababababababababababababababababababababababababababababab": "not-a-number"
            },
            "initial_validators": []
        }"#;
        let path = write_genesis_json(dir.path(), json);
        let err = load_genesis(&engine, &path).unwrap_err();
        assert!(err.to_string().contains("invalid decimal"), "got: {err}");
    }

    #[test]
    fn test_load_genesis_with_validator() {
        let (dir, engine) = setup_engine();
        let json = r#"{
            "chain_id": 7,
            "genesis_time": "2025-06-01T00:00:00Z",
            "initial_accounts": {},
            "initial_validators": [
                {
                    "address": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    "stake": "50000000000000000000000000000000000"
                }
            ]
        }"#;
        let path = write_genesis_json(dir.path(), json);
        load_genesis(&engine, &path).unwrap();

        let raw_addr = parse_hex_addr("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let raw = engine.get(tables::VALIDATORS, &raw_addr).unwrap().unwrap();
        // First 32 bytes = address, next 32 bytes = stake (U256 LE)
        assert_eq!(&raw[..32], &raw_addr);
        let stake_bytes: [u8; 32] = raw[32..64].try_into().unwrap();
        let stake = U256::from_little_endian(&stake_bytes);
        assert_eq!(stake, U256::from_dec_str("50000000000000000000000000000000000").unwrap());
    }

    #[test]
    fn test_fresh_db_no_genesis_marker() {
        let (_dir, engine) = setup_engine();
        assert!(!engine.exists(tables::META, tables::GENESIS_LOADED_KEY).unwrap());
    }

    #[test]
    fn test_genesis_marker_set_after_load() {
        let (dir, engine) = setup_engine();
        let path = write_genesis_json(dir.path(), &minimal_genesis_json());
        load_genesis(&engine, &path).unwrap();
        assert!(engine.exists(tables::META, tables::GENESIS_LOADED_KEY).unwrap());
    }
}
