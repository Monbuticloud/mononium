//! End-to-end integration tests for mononium-cli.
//!
//! Start a real node, query balances, submit transactions, verify state changes.

use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

/// Path to the compiled CLI binary.
fn cli_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_mononium-cli"))
}

/// A running node instance that cleans up on drop.
struct NodeProcess {
    child: Child,
    port: u16,
    data_dir: PathBuf,
}

impl NodeProcess {
    fn start(genesis_path: &PathBuf, data_dir: &PathBuf, port: u16) -> Self {
        let child = Command::new(cli_binary())
            .arg("node")
            .arg("--genesis")
            .arg(genesis_path)
            .arg("--data-dir")
            .arg(data_dir)
            .arg("--observer")
            .arg("--rest-port")
            .arg(port.to_string())
            .arg("--log-level")
            .arg("error")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("failed to start node");

        let mut node = Self {
            child,
            port,
            data_dir: data_dir.clone(),
        };

        // Wait for node to be ready
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if Instant::now() > deadline {
                panic!("node failed to start within 15s");
            }
            if let Ok(resp) = reqwest::blocking::get(&format!("http://localhost:{port}/health")) {
                if resp.status().is_success() {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        node
    }

    fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    fn wait_for_blocks(&self, min_height: u64, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        loop {
            if Instant::now() > deadline {
                panic!("timed out waiting for height >= {min_height}");
            }
            let url = format!("{}/height", self.url());
            if let Ok(resp) = reqwest::blocking::get(&url) {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    if let Some(h) = json["height"].as_u64() {
                        if h >= min_height {
                            return;
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }
}

impl Drop for NodeProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // Clean up data dir
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn e2e_wallet_balance_and_transfer() {
    // First generate a key — this creates it in ~/.mononium/keys/
    let key_name = format!("e2e_test_alice_{}", std::process::id());
    let output = Command::new(cli_binary())
        .arg("wallet")
        .arg("keygen")
        .arg(&key_name)
        .output()
        .expect("failed to run keygen");
    assert!(
        output.status.success(),
        "keygen failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Extract address from stdout, strip checksum for raw 32-byte hex
    let stdout = String::from_utf8_lossy(&output.stdout);
    let addr_line = stdout.lines().find(|l| l.contains("Address:")).unwrap();
    let address_with_checksum = addr_line.split("Address: ").nth(1).unwrap().trim();
    assert!(
        address_with_checksum.starts_with("0x"),
        "address should start with 0x"
    );
    assert_eq!(
        address_with_checksum.len(),
        82,
        "address should be 82 chars (0x + 64 + 16)"
    );

    // Raw hex address (without checksum) for genesis and balance queries
    let raw_addr_hex = &address_with_checksum[2..66]; // 64 hex chars
    assert_eq!(raw_addr_hex.len(), 64);

    // Full formatted address (with checksum) for key file lookup
    let full_address = address_with_checksum;

    // Create temp genesis JSON with this address
    let dir = std::env::temp_dir().join(format!("mononium_e2e_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let genesis_path = dir.join("genesis.json");
    let genesis_json = serde_json::json!({
        "chain_id": 0,
        "genesis_time": "2026-06-20T00:00:00Z",
        "initial_accounts": {
            raw_addr_hex: "100000000000000000000000000000000000" // 100 MONEX
        },
        "initial_validators": []
    });
    std::fs::write(
        &genesis_path,
        serde_json::to_string_pretty(&genesis_json).unwrap(),
    )
    .unwrap();

    // Start node
    let data_dir = dir.join("data");
    let port = 19944;
    let node = NodeProcess::start(&genesis_path, &data_dir, port);

    // Wait for a few blocks
    node.wait_for_blocks(3, Duration::from_secs(30));

    // -----------------------------------------------------------------------
    // Test: balance query shows genesis balance
    // -----------------------------------------------------------------------
    let balance_output = Command::new(cli_binary())
        .arg("wallet")
        .arg("balance")
        .arg(raw_addr_hex)
        .arg("--node")
        .arg(node.url())
        .output()
        .expect("failed to query balance");
    assert!(
        balance_output.status.success(),
        "balance query failed: {}",
        String::from_utf8_lossy(&balance_output.stderr)
    );
    let balance_stdout = String::from_utf8_lossy(&balance_output.stdout);
    assert!(
        balance_stdout.contains("100000000000000000000000000000000000"),
        "expected genesis balance in output, got: {balance_stdout}"
    );

    // -----------------------------------------------------------------------
    // Test: transfer sends MONEX
    // -----------------------------------------------------------------------
    // Bob's address (random)
    let bob_raw = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let transfer_output = Command::new(cli_binary())
        .arg("wallet")
        .arg("transfer")
        .arg(bob_raw) // recipient address (no 0x)
        .arg("1.0") // amount in MONEX
        .arg("--key")
        .arg(&key_name)
        .arg("--node")
        .arg(node.url())
        .output()
        .expect("failed to run transfer");
    assert!(
        transfer_output.status.success(),
        "transfer failed: {}",
        String::from_utf8_lossy(&transfer_output.stderr)
    );
    let transfer_stdout = String::from_utf8_lossy(&transfer_output.stdout);
    assert!(
        transfer_stdout.contains("Transaction submitted"),
        "expected submission confirmation, got: {transfer_stdout}"
    );

    // Wait for next block to include the tx
    node.wait_for_blocks(4, Duration::from_secs(15));

    // -----------------------------------------------------------------------
    // Test: bob's balance reflects transfer
    // -----------------------------------------------------------------------
    let bob_balance_output = Command::new(cli_binary())
        .arg("wallet")
        .arg("balance")
        .arg(bob_raw)
        .arg("--node")
        .arg(node.url())
        .output()
        .expect("failed to query bob balance");
    assert!(
        bob_balance_output.status.success(),
        "bob balance query failed: {}",
        String::from_utf8_lossy(&bob_balance_output.stderr)
    );
    let bob_stdout = String::from_utf8_lossy(&bob_balance_output.stdout);
    // Bob should have 1.0 MONEX = 10^32 MOXX
    assert!(
        bob_stdout.contains("100000000000000000000000000000000"),
        "expected bob to have ~1 MONEX, got: {bob_stdout}"
    );

    // -----------------------------------------------------------------------
    // Test: query block/latest and block/:height
    // -----------------------------------------------------------------------
    let latest_output = Command::new(cli_binary())
        .arg("query")
        .arg("latest")
        .arg("--node")
        .arg(node.url())
        .output()
        .expect("failed to query latest block");
    assert!(
        latest_output.status.success(),
        "latest block query failed: {}",
        String::from_utf8_lossy(&latest_output.stderr)
    );

    let block_output = Command::new(cli_binary())
        .arg("query")
        .arg("block")
        .arg("1")
        .arg("--node")
        .arg(node.url())
        .output()
        .expect("failed to query block 1");
    assert!(
        block_output.status.success(),
        "block 1 query failed: {}",
        String::from_utf8_lossy(&block_output.stderr)
    );

    // Node is killed on drop (end of scope)
}
// Test of doom and despair (CI runners won't like this one)
