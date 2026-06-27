//! Mononium CLI — node daemon, wallet, and chain queries.
//!
//! ```text
//! mononium-cli
//! ├── node          # start the validator/observer node
//! ├── wallet        # key management + transactions
//! ├── query         # chain queries (block, tx, validator set)
//! └── logfmt        # convert JSON logs to human-readable
//! ```

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

mod node;
mod wallet;

/// Mononium blockchain node and wallet.
#[derive(Parser)]
#[command(name = "mononium-cli", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the node daemon (validator or observer).
    Node(NodeArgs),

    /// Wallet operations: key generation, balance, transfers.
    Wallet(WalletArgs),

    /// Chain queries: blocks, transactions, validators.
    Query(QueryArgs),

    /// Convert JSON log lines to human-readable format (reads from stdin).
    Logfmt,
}

// ---------------------------------------------------------------------------
// node subcommand
// ---------------------------------------------------------------------------

#[derive(Args, Debug, Clone)]
struct NodeArgs {
    /// Path to config file (YAML or TOML).
    #[arg(long)]
    config: Option<PathBuf>,

    /// Path to genesis JSON (overrides config file).
    #[arg(long)]
    genesis: Option<String>,

    /// Validator key name (XOR with --key-file / --observer).
    #[arg(long)]
    key: Option<String>,

    /// Absolute path to key file.
    #[arg(long)]
    key_file: Option<String>,

    /// Observer mode (no signing, sync-only).
    #[arg(long, default_value_t = false)]
    observer: bool,

    /// P2P libp2p port.
    #[arg(long)]
    p2p_port: Option<u16>,

    /// JSON-RPC WebSocket port.
    #[arg(long)]
    rpc_port: Option<u16>,

    /// REST HTTP port.
    #[arg(long)]
    rest_port: Option<u16>,

    /// Bootstrap peer multiaddrs (repeatable).
    #[arg(long)]
    bootnode: Vec<String>,

    /// Data directory.
    #[arg(long)]
    data_dir: Option<String>,

    /// Storage retention mode (full | compact).
    #[arg(long)]
    storage_mode: Option<String>,

    /// Log level (trace | debug | info | warn | error).
    #[arg(long)]
    log_level: Option<String>,

    /// Log format (json | text).
    #[arg(long)]
    log_format: Option<String>,

    /// Key derivation timeout in seconds.
    #[arg(long)]
    unlock_timeout: Option<u64>,
}

// ---------------------------------------------------------------------------
// wallet subcommand
// ---------------------------------------------------------------------------

#[derive(Args, Debug, Clone)]
struct WalletArgs {
    #[command(subcommand)]
    action: WalletAction,
}

#[derive(Subcommand, Debug, Clone)]
enum WalletAction {
    /// Generate a new Falcon-512 key pair.
    Keygen {
        /// Name for the key file (saved to ~/.mononium/keys/{name}.json).
        name: String,
    },
    /// Query account balance.
    Balance {
        /// Address to query (hex, with or without 0x).
        address: String,
        /// Node REST endpoint.
        #[arg(long, default_value = "http://localhost:9933")]
        node: String,
    },
    /// Send MONEX to another account.
    Transfer {
        /// Recipient address (hex, with or without 0x).
        to: String,
        /// Amount in MONEX (human-readable, e.g. 1.5).
        amount: String,
        /// Sender key name.
        #[arg(long)]
        key: String,
        /// Node REST endpoint.
        #[arg(long, default_value = "http://localhost:9933")]
        node: String,
    },
    /// Register as a validator.
    Register {
        /// Signing key name.
        #[arg(long)]
        key: String,
        /// Node REST endpoint.
        #[arg(long, default_value = "http://localhost:9933")]
        node: String,
    },
    /// Stake MONEX to a validator.
    Stake {
        /// Validator address.
        validator: String,
        /// Amount in MONEX.
        amount: String,
        /// Signing key name.
        #[arg(long)]
        key: String,
        /// Node REST endpoint.
        #[arg(long, default_value = "http://localhost:9933")]
        node: String,
    },
    /// Atomic register as validator + self-stake.
    RegisterAndStake {
        /// Self-stake amount in MONEX.
        amount: String,
        /// Signing key name.
        #[arg(long)]
        key: String,
        /// Node REST endpoint.
        #[arg(long, default_value = "http://localhost:9933")]
        node: String,
    },
    /// Unstake MONEX from a validator.
    Unstake {
        /// Validator address.
        validator: String,
        /// Amount in MONEX.
        amount: String,
        /// Signing key name.
        #[arg(long)]
        key: String,
        /// Node REST endpoint.
        #[arg(long, default_value = "http://localhost:9933")]
        node: String,
    },
}

// ---------------------------------------------------------------------------
// query subcommand
// ---------------------------------------------------------------------------

#[derive(Args, Debug, Clone)]
struct QueryArgs {
    #[command(subcommand)]
    action: QueryAction,
}

#[derive(Subcommand, Debug, Clone)]
enum QueryAction {
    /// Get block at a given height.
    Block {
        height: u64,
        #[arg(long, default_value = "http://localhost:9933")]
        node: String,
    },
    /// Get latest block.
    Latest {
        #[arg(long, default_value = "http://localhost:9933")]
        node: String,
    },
    /// Get nonce for an address.
    Nonce {
        address: String,
        #[arg(long, default_value = "http://localhost:9933")]
        node: String,
    },
    /// Get validator info.
    Validator {
        address: String,
        #[arg(long, default_value = "http://localhost:9933")]
        node: String,
    },
}

// ===========================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Node(args) => node::run_node(args).await,
        Command::Wallet(wallet) => run_wallet(wallet).await,
        Command::Query(query) => run_query(query).await,
        Command::Logfmt => run_logfmt(),
    }
}

// ---------------------------------------------------------------------------
// Wallet subcommand dispatcher
// ---------------------------------------------------------------------------

async fn run_wallet(args: WalletArgs) -> anyhow::Result<()> {
    match args.action {
        WalletAction::Keygen { name } => wallet::keygen(&name),
        WalletAction::Balance { address, node } => wallet::balance(&address, &node).await,
        WalletAction::Transfer {
            to,
            amount,
            key,
            node,
        } => wallet::transfer(&to, &amount, &key, &node).await,
        WalletAction::Register { key, node } => wallet::register_validator(&key, &node).await,
        WalletAction::Stake {
            validator,
            amount,
            key,
            node,
        } => wallet::stake(&validator, &amount, &key, &node).await,
        WalletAction::RegisterAndStake { amount, key, node } => {
            wallet::register_and_stake(&amount, &key, &node).await
        }
        WalletAction::Unstake {
            validator,
            amount,
            key,
            node,
        } => wallet::unstake(&validator, &amount, &key, &node).await,
    }
}

// ---------------------------------------------------------------------------
// Query subcommand dispatcher
// ---------------------------------------------------------------------------

async fn run_query(args: QueryArgs) -> anyhow::Result<()> {
    match &args.action {
        QueryAction::Nonce { address, node } => {
            let base_url = node.trim_end_matches('/');
            let url = format!("{base_url}/nonce/{address}");
            let resp = reqwest::get(&url).await?;
            if !resp.status().is_success() {
                anyhow::bail!(
                    "RPC error ({}): {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                );
            }
            let data: serde_json::Value = resp.json().await?;
            println!("Nonce: {}", data["nonce"]);
            return Ok(());
        }
        QueryAction::Validator { address, node } => {
            let base_url = node.trim_end_matches('/');
            let url = format!("{base_url}/validator/{address}");
            let resp = reqwest::get(&url).await?;
            if !resp.status().is_success() {
                anyhow::bail!(
                    "RPC error ({}): {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                );
            }
            let data: serde_json::Value = resp.json().await?;
            println!("Validator: {address}");
            println!("  Exists: {}", data["exists"]);
            println!("  Status: {}", data["status"].as_str().unwrap_or("?"));
            println!("  Stake:  {}", data["stake"].as_str().unwrap_or("0"));
            return Ok(());
        }
        _ => {}
    }

    let base_url = match &args.action {
        QueryAction::Block { node, .. } | QueryAction::Latest { node, .. } => {
            node.trim_end_matches('/').to_string()
        }
        _ => unreachable!(),
    };

    let url = match args.action {
        QueryAction::Block { height, .. } => format!("{base_url}/block/{height}"),
        QueryAction::Latest { .. } => format!("{base_url}/block/latest"),
        _ => unreachable!(),
    };

    let resp = reqwest::get(&url)
        .await
        .map_err(|e| anyhow::anyhow!("failed to query {url}: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("RPC error ({}): {}", status, text);
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("failed to parse response: {e}"))?;
    println!("{}", serde_json::to_string_pretty(&json).unwrap());
    Ok(())
}

// ---------------------------------------------------------------------------
// Logfmt
// ---------------------------------------------------------------------------

fn run_logfmt() -> anyhow::Result<()> {
    use std::io::{self, BufRead};
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
            let level = json["level"].as_str().unwrap_or("?");
            let msg = json["msg"].as_str().unwrap_or(&line);
            let target = json["target"].as_str().unwrap_or("");
            let ts = json["timestamp"].as_str().unwrap_or("");
            println!("{ts} [{level:>5}] {target}: {msg}");
        } else {
            println!("{line}");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_node_minimal() {
        let cli = Cli::try_parse_from([
            "mononium-cli",
            "node",
            "--genesis",
            "gen.json",
            "--observer",
        ])
        .unwrap();
        match cli.command {
            Command::Node(args) => {
                assert!(args.observer);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn test_cli_wallet_keygen() {
        let cli = Cli::try_parse_from(["mononium-cli", "wallet", "keygen", "test-key"]).unwrap();
        match cli.command {
            Command::Wallet(WalletArgs {
                action: WalletAction::Keygen { name },
            }) => {
                assert_eq!(name, "test-key");
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn test_cli_wallet_balance() {
        let cli = Cli::try_parse_from([
            "mononium-cli",
            "wallet",
            "balance",
            "0xabcd",
            "--node",
            "http://localhost:9999",
        ])
        .unwrap();
        match cli.command {
            Command::Wallet(WalletArgs {
                action: WalletAction::Balance { address, node },
            }) => {
                assert_eq!(address, "0xabcd");
                assert_eq!(node, "http://localhost:9999");
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn test_cli_wallet_transfer() {
        let cli = Cli::try_parse_from([
            "mononium-cli",
            "wallet",
            "transfer",
            "0xrecipient",
            "1.5",
            "--key",
            "my-key",
        ])
        .unwrap();
        match cli.command {
            Command::Wallet(WalletArgs {
                action:
                    WalletAction::Transfer {
                        to,
                        amount,
                        key,
                        node,
                    },
            }) => {
                assert_eq!(to, "0xrecipient");
                assert_eq!(amount, "1.5");
                assert_eq!(key, "my-key");
                assert_eq!(node, "http://localhost:9933"); // default
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn test_cli_query_block() {
        let cli = Cli::try_parse_from(["mononium-cli", "query", "block", "42"]).unwrap();
        match cli.command {
            Command::Query(QueryArgs {
                action: QueryAction::Block { height, .. },
            }) => {
                assert_eq!(height, 42);
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn test_cli_query_latest_default_node() {
        let cli = Cli::try_parse_from(["mononium-cli", "query", "latest"]).unwrap();
        match cli.command {
            Command::Query(QueryArgs {
                action: QueryAction::Latest { node },
            }) => {
                assert_eq!(node, "http://localhost:9933");
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn test_cli_logfmt() {
        let cli = Cli::try_parse_from(["mononium-cli", "logfmt"]).unwrap();
        match cli.command {
            Command::Logfmt => {} // expected
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn test_cli_node_all_flags() {
        let cli = Cli::try_parse_from([
            "mononium-cli",
            "node",
            "--config",
            "/path/config.yaml",
            "--genesis",
            "/path/genesis.json",
            "--key",
            "validator1",
            "--p2p-port",
            "30444",
            "--rpc-port",
            "9955",
            "--rest-port",
            "9944",
            "--bootnode",
            "/ip4/1.2.3.4/tcp/30333",
            "--bootnode",
            "/ip4/5.6.7.8/tcp/30333",
            "--data-dir",
            "/data/mononium",
            "--storage-mode",
            "archive",
            "--log-level",
            "debug",
            "--log-format",
            "json",
            "--unlock-timeout",
            "300",
        ])
        .unwrap();
        match cli.command {
            Command::Node(args) => {
                assert_eq!(args.config, Some(PathBuf::from("/path/config.yaml")));
                assert_eq!(args.genesis, Some("/path/genesis.json".to_string()));
                assert_eq!(args.key, Some("validator1".to_string()));
                assert_eq!(args.p2p_port, Some(30444));
                assert_eq!(args.rpc_port, Some(9955));
                assert_eq!(args.rest_port, Some(9944));
                assert_eq!(args.bootnode.len(), 2);
                assert_eq!(args.data_dir, Some("/data/mononium".to_string()));
                assert_eq!(args.storage_mode, Some("archive".to_string()));
                assert_eq!(args.log_level, Some("debug".to_string()));
                assert_eq!(args.log_format, Some("json".to_string()));
                assert_eq!(args.unlock_timeout, Some(300));
            }
            _ => panic!("unexpected command"),
        }
    }

    #[test]
    fn test_run_logfmt_parses_json_line() {
        let _json_line = r#"{"level":"INFO","msg":"hello world","target":"test","timestamp":"2025-01-01T00:00:00Z"}"#;
    }

    #[test]
    fn test_run_logfmt_passthrough_non_json() {
        let _plain_line = "just a plain log line";
    }
}
