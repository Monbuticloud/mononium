//! Wallet operations: key generation, balance queries, transaction creation.

use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

use mononium_lib::config::constants as cfg_constants;
use mononium_lib::core::account::{format_address, Address};
use mononium_lib::core::transaction::{Transaction, TxBody};
use mononium_lib::crypto::address::derive_address;
use mononium_lib::crypto::constants::FALCON_SEED_SIZE;
use mononium_lib::crypto::falcon::{Falcon512, Falcon512PublicKey, Falcon512Signature};
use mononium_lib::crypto::signature::SignatureScheme;

// ---------------------------------------------------------------------------
// Key file format
// ---------------------------------------------------------------------------

/// JSON structure saved to `~/.mononium/keys/{name}.json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct KeyFile {
    /// Hex-encoded 897-byte Falcon-512 public key.
    pub public_key: String,
    /// Hex-encoded 1281-byte Falcon-512 private key.
    pub private_key: String,
    /// Hex-encoded 48-byte seed entropy.
    pub seed: String,
    /// Formatted address (0x + 64 hex + 16 checksum).
    pub address: String,
}

// ---------------------------------------------------------------------------
// Key generation
// ---------------------------------------------------------------------------

/// Generate a new Falcon-512 key pair and save it to disk.
///
/// Saves to `~/.mononium/keys/{name}.json` and prints the address.
pub fn keygen(name: &str) -> Result<()> {
    // Generate random 48-byte seed
    use rand::RngCore;
    let mut seed = [0u8; FALCON_SEED_SIZE];
    rand::thread_rng().fill_bytes(&mut seed);

    // Generate key pair
    let kp = Falcon512::generate(&seed)
        .context("failed to generate Falcon-512 key pair")?;

    let pk_bytes = kp.public_key_bytes();
    let _pk = Falcon512PublicKey(pk_bytes);
    let sk_bytes = kp.private_key_bytes();

    // Derive address
    let addr = derive_address(&pk_bytes);
    let addr_formatted = format_address(&addr);

    // Build key file
    let key_file = KeyFile {
        public_key: format!("0x{}", hex::encode(pk_bytes)),
        private_key: format!("0x{}", hex::encode(sk_bytes)),
        seed: format!("0x{}", hex::encode(seed)),
        address: addr_formatted.clone(),
    };

    // Ensure keys directory exists
    let keys_dir = cfg_constants::default_key_dir();
    std::fs::create_dir_all(&keys_dir)
        .context(format!("failed to create keys directory at {:?}", keys_dir))?;

    let key_path: PathBuf = keys_dir.join(format!("{name}.json"));
    let json = serde_json::to_string_pretty(&key_file)
        .context("failed to serialize key file")?;

    let mut file = std::fs::File::create(&key_path)
        .context(format!("failed to create key file at {:?}", key_path))?;
    file.write_all(json.as_bytes())
        .context("failed to write key file")?;

    println!("Generated key pair: {name}");
    println!("  Address: {addr_formatted}");
    println!("  Saved to: {path}", path = key_path.display());

    Ok(())
}

// ---------------------------------------------------------------------------
// Load a key file
// ---------------------------------------------------------------------------

/// Load a key file by name from the default keys directory.
pub fn load_key(name: &str) -> Result<KeyFile> {
    let keys_dir = cfg_constants::default_key_dir();
    let key_path: PathBuf = keys_dir.join(format!("{name}.json"));
    let json = std::fs::read_to_string(&key_path)
        .map_err(|e| anyhow::anyhow!("failed to read key file at {:?}: {e}", key_path))?;
    let key_file: KeyFile = serde_json::from_str(&json)
        .context("failed to parse key file")?;
    Ok(key_file)
}

// ---------------------------------------------------------------------------
// Balance query
// ---------------------------------------------------------------------------

/// Query an account balance from a node's REST API.
pub async fn balance(address: &str, node_url: &str) -> Result<()> {
    let url = format!("{}/balance/{}", node_url.trim_end_matches('/'), address);
    let resp = reqwest::get(&url)
        .await
        .map_err(|e| anyhow::anyhow!("failed to query {url}: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("RPC error ({}): {}", status, text);
    }

    let balance_data: serde_json::Value = resp.json().await
        .map_err(|e| anyhow::anyhow!("failed to parse response: {e}"))?;
    let bal = balance_data["balance"].as_str().unwrap_or("0");
    let nonce = balance_data["nonce"].as_u64().unwrap_or(0);

    println!("Address: {address}");
    println!("Balance: {bal} MOXX");
    println!("Nonce:   {nonce}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Transfer
// ---------------------------------------------------------------------------

/// Create, sign, and submit a transfer transaction.
pub async fn transfer(to: &str, amount_monex: &str, key_name: &str, node_url: &str) -> Result<()> {
    // Load key
    let key_file = load_key(key_name)?;

    // Decode private key
    let sk_hex = key_file
        .private_key
        .strip_prefix("0x")
        .unwrap_or(&key_file.private_key);
    let sk_bytes = hex::decode(sk_hex)
        .map_err(|e| anyhow::anyhow!("invalid private key hex: {e}"))?;

    // Reconstruct key pair from private key
    let kp = Falcon512::from_private_key(&sk_bytes)
        .map_err(|e| anyhow::anyhow!("failed to restore key pair: {e}"))?;

    // Parse recipient address (raw hex without checksum)
    let to_hex = to.strip_prefix("0x").unwrap_or(to);
    let to_bytes = hex::decode(to_hex)
        .map_err(|e| anyhow::anyhow!("invalid recipient address hex: {e}"))?;
    if to_bytes.len() != 32 {
        anyhow::bail!("recipient address must be 32 bytes, got {}", to_bytes.len());
    }
    let mut to_addr_bytes = [0u8; 32];
    to_addr_bytes.copy_from_slice(&to_bytes);
    let recipient = Address::from(to_addr_bytes);

    // Parse amount (supports decimal MONEX → MOXX conversion)
    let amount_moxx = parse_monex_amount(amount_monex)?;

    // Get current nonce and chain_id from node
    let base_url = node_url.trim_end_matches('/');
    let addr_hex = hex::encode(to_addr_bytes);
    let balance_url = format!("{base_url}/balance/0x{addr_hex}");
    let bal_resp = reqwest::get(&balance_url)
        .await
        .map_err(|e| anyhow::anyhow!("failed to query nonce: {e}"))?;
    let bal_data: serde_json::Value = bal_resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("failed to parse balance response: {e}"))?;
    let nonce = bal_data["nonce"].as_u64().unwrap_or(0);

    // Build transaction
    let tx = Transaction {
        chain_id: 0,
        nonce,
        sender: derive_address(&kp.public_key_bytes()),
        fee: mononium_lib::core::constants::DEFAULT_FLAT_FEE,
        body: TxBody::Transfer {
            recipient,
            amount: amount_moxx,
        },
        signature: Falcon512Signature::from_bytes(&[0u8; 809]).unwrap(), // placeholder
    };

    // Serialize and sign
    let tx_encoded = parity_scale_codec::Encode::encode(&tx);
    let sig = Falcon512::sign(&kp, &tx_encoded)
        .map_err(|e| anyhow::anyhow!("failed to sign transaction: {e}"))?;

    // Build final signed tx
    let signed_tx = Transaction {
        signature: sig,
        ..tx
    };

    // Submit to node
    let submit_url = format!("{base_url}/tx");
    let tx_json = serde_json::to_value(&signed_tx)
        .map_err(|e| anyhow::anyhow!("failed to serialize transaction: {e}"))?;

    let client = reqwest::Client::new();
    let resp = client
        .post(&submit_url)
        .json(&tx_json)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("failed to submit tx: {e}"))?;

    if resp.status().is_success() {
        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("failed to parse response: {e}"))?;
        println!("Transaction submitted!");
        println!("  Tx hash: {}", result["tx_hash"].as_str().unwrap_or("unknown"));
    } else {
        let status = resp.status();
        let text_fut = resp.text();
        let text = text_fut.await.unwrap_or_default();
        anyhow::bail!("submit error ({}): {}", status, text);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Amount parsing
// ---------------------------------------------------------------------------

/// Parse a MONEX amount string (e.g. "1.5") into MOXX (U256 in LE bytes).
///
/// Handles decimal strings with up to 32 decimal places.
fn parse_monex_amount(s: &str) -> Result<primitive_types::U256> {
    use primitive_types::U256;

    // Split on decimal point
    let parts: Vec<&str> = s.split('.').collect();
    let (integer_str, fractional_str) = match parts.len() {
        1 => (parts[0], ""),
        2 => (parts[0], parts[1]),
        _ => anyhow::bail!("invalid amount format: {s}"),
    };

    // Pad/truncate fractional part to 32 digits
    let mut fractional = fractional_str.to_string();
    if fractional.len() > 32 {
        anyhow::bail!("too many decimal places (max 32): {s}");
    }
    while fractional.len() < 32 {
        fractional.push('0');
    }

    // Combine: integer.fractional → one big integer
    let combined = format!("{integer_str}{fractional}");
    U256::from_dec_str(&combined)
        .with_context(|| format!("invalid amount: {s}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_monex_amount_whole() {
        let val = parse_monex_amount("100").unwrap();
        // 100 MONEX = 100 * 10^32 MOXX = 10^34 MOXX
        assert_eq!(val, primitive_types::U256::from(10u128).pow(34.into()));
    }

    #[test]
    fn test_parse_monex_amount_decimal() {
        let val = parse_monex_amount("1.5").unwrap();
        assert_eq!(val, primitive_types::U256::from(15u128) * primitive_types::U256::from(10u128).pow(31.into()));
    }

    #[test]
    fn test_parse_monex_amount_zero() {
        let val = parse_monex_amount("0").unwrap();
        assert_eq!(val, primitive_types::U256::zero());
    }

    #[test]
    fn test_parse_monex_amount_too_many_decimals() {
        let err = parse_monex_amount("1.1234567890123456789012345678901234567890");
        assert!(err.is_err());
    }

    #[test]
    fn test_parse_monex_amount_small() {
        let val = parse_monex_amount("0.0000000000000000001").unwrap();
        assert_eq!(val, primitive_types::U256::from_dec_str("10000000000000").unwrap());
    }

    #[test]
    fn test_parse_monex_amount_invalid() {
        assert!(parse_monex_amount("abc").is_err());
        assert!(parse_monex_amount("1.2.3").is_err());
    }

    #[test]
    fn test_keyfile_json_roundtrip() {
        let kf = KeyFile {
            public_key: "0xab".to_string(),
            private_key: "0xcd".to_string(),
            seed: "0xef".to_string(),
            address: "0x1234".to_string(),
        };
        let json = serde_json::to_string(&kf).unwrap();
        let decoded: KeyFile = serde_json::from_str(&json).unwrap();
        assert_eq!(kf.public_key, decoded.public_key);
        assert_eq!(kf.private_key, decoded.private_key);
        assert_eq!(kf.seed, decoded.seed);
        assert_eq!(kf.address, decoded.address);
    }

    #[test]
    fn test_keyfile_missing_field_fails() {
        let err = serde_json::from_str::<KeyFile>("{}");
        assert!(err.is_err());
    }
}
