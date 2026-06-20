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

use anyhow::Context;
use clap::{Parser, Subcommand, Args};

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
}

// ===========================================================================

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Node(args) => node::run_node(args).await,
        Command::Wallet(wallet) => run_wallet(wallet),
        Command::Query(query) => run_query(query),
        Command::Logfmt => run_logfmt(),
    }
}

// ---------------------------------------------------------------------------
// Wallet subcommand dispatcher
// ---------------------------------------------------------------------------

fn run_wallet(args: WalletArgs) -> anyhow::Result<()> {
    match args.action {
        WalletAction::Keygen { name } => wallet::keygen(&name),
        WalletAction::Balance { address, node } => wallet::balance(&address, &node),
        WalletAction::Transfer { to, amount, key, node } => {
            wallet::transfer(&to, &amount, &key, &node)
        }
    }
}

// ---------------------------------------------------------------------------
// Query subcommand dispatcher
// ---------------------------------------------------------------------------

fn run_query(args: QueryArgs) -> anyhow::Result<()> {
    let base_url = match &args.action {
        QueryAction::Block { node, .. } | QueryAction::Latest { node, .. } => {
            node.trim_end_matches('/').to_string()
        }
    };

    let url = match args.action {
        QueryAction::Block { height, .. } => format!("{base_url}/block/{height}"),
        QueryAction::Latest { .. } => format!("{base_url}/block/latest"),
    };

    let resp = reqwest::blocking::get(&url)
        .with_context(|| format!("failed to query {url}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        anyhow::bail!("RPC error ({}): {}", status, text);
    }

    let json: serde_json::Value = resp.json()?;
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
