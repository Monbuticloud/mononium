//! Node configuration: YAML/TOML loading, CLI override merging, defaults.

pub mod constants;

use std::path::Path;

use serde::Deserialize;

use crate::error::{LibError, Result};

// ---------------------------------------------------------------------------
// CliOverrides — partial config struct fed by CLI flags
// ---------------------------------------------------------------------------

/// CLI flag values that override config file fields.
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    pub genesis: Option<String>,
    pub key: Option<String>,
    pub key_file: Option<String>,
    pub observer: Option<bool>,
    pub p2p_port: Option<u16>,
    pub rpc_port: Option<u16>,
    pub rest_port: Option<u16>,
    pub bootnodes: Option<Vec<String>>,
    pub data_dir: Option<String>,
    pub storage_mode: Option<String>,
    pub compact_eras: Option<u32>,
    pub full_node_rpc: Option<Vec<String>>,
    pub log_level: Option<String>,
    pub log_json: Option<bool>,
    pub unlock_timeout: Option<u64>,
}

// ---------------------------------------------------------------------------
// Nested config sections (match YAML/TOML schema)
// ---------------------------------------------------------------------------

/// `[node]` section.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NodeSection {
    pub data_dir: Option<String>,
    pub unlock_timeout: Option<u64>,
}

impl Default for NodeSection {
    fn default() -> Self {
        Self {
            data_dir: None,
            unlock_timeout: None,
        }
    }
}

/// `[crypto]` section (config-file only).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CryptoSec {
    pub argon2_memory_mib: Option<u32>,
    pub argon2_iterations: Option<u32>,
}

impl Default for CryptoSec {
    fn default() -> Self {
        Self {
            argon2_memory_mib: None,
            argon2_iterations: None,
        }
    }
}

/// `[network]` section.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NetworkSection {
    pub p2p_port: Option<u16>,
    pub rpc_port: Option<u16>,
    pub rest_port: Option<u16>,
    pub bootnodes: Option<Vec<String>>,
}

impl Default for NetworkSection {
    fn default() -> Self {
        Self {
            p2p_port: None,
            rpc_port: None,
            rest_port: None,
            bootnodes: None,
        }
    }
}

/// `[storage]` section.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StorageSection {
    pub mode: Option<String>,
    pub compact_eras: Option<u32>,
    pub full_node_rpc: Option<Vec<String>>,
}

impl Default for StorageSection {
    fn default() -> Self {
        Self {
            mode: None,
            compact_eras: None,
            full_node_rpc: None,
        }
    }
}

/// `[mempool]` section.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MempoolSection {
    pub max_tx_per_account: Option<usize>,
}

impl Default for MempoolSection {
    fn default() -> Self {
        Self {
            max_tx_per_account: None,
        }
    }
}

/// `[log]` section.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LogSection {
    pub level: Option<String>,
    pub json: Option<bool>,
    pub file: Option<String>,
}

impl Default for LogSection {
    fn default() -> Self {
        Self {
            level: None,
            json: None,
            file: None,
        }
    }
}

// ---------------------------------------------------------------------------
// NodeConfig
// ---------------------------------------------------------------------------

/// Full node configuration, typically loaded from a YAML or TOML file then
/// overlaid with CLI flags.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct NodeConfig {
    // --- Identity (top-level) ---
    pub key: Option<String>,
    pub key_file: Option<String>,
    pub observer: bool,

    // --- Genesis (top-level) ---
    pub genesis: Option<String>,

    // --- Nested sections ---
    pub node: NodeSection,
    pub crypto: CryptoSec,
    pub network: NetworkSection,
    pub storage: StorageSection,
    pub mempool: MempoolSection,
    pub log: LogSection,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            key: None,
            key_file: None,
            observer: false,
            genesis: None,
            node: NodeSection::default(),
            crypto: CryptoSec::default(),
            network: NetworkSection::default(),
            storage: StorageSection::default(),
            mempool: MempoolSection::default(),
            log: LogSection::default(),
        }
    }
}

impl NodeConfig {
    /// Load configuration from a YAML or TOML file, detected by extension.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| LibError::Storage(format!("cannot read config: {e}")))?;

        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match ext {
            "yaml" | "yml" => serde_yaml::from_str(&content)
                .map_err(|e| LibError::Storage(format!("invalid YAML config: {e}"))),
            "toml" => toml::from_str(&content)
                .map_err(|e| LibError::Storage(format!("invalid TOML config: {e}"))),
            other => Err(LibError::Storage(format!(
                "unsupported config format: '.{other}' (expected .yaml, .yml, or .toml)"
            ))),
        }
    }

    /// Merge CLI flag overrides on top of the current config.
    pub fn merge_cli(&mut self, cli: CliOverrides) {
        if let Some(v) = cli.genesis { self.genesis = Some(v); }
        if let Some(v) = cli.key { self.key = Some(v); }
        if let Some(v) = cli.key_file { self.key_file = Some(v); }
        if let Some(v) = cli.observer { self.observer = v; }
        if let Some(v) = cli.p2p_port { self.network.p2p_port = Some(v); }
        if let Some(v) = cli.rpc_port { self.network.rpc_port = Some(v); }
        if let Some(v) = cli.rest_port { self.network.rest_port = Some(v); }
        if let Some(v) = cli.bootnodes { self.network.bootnodes = Some(v); }
        if let Some(v) = cli.data_dir { self.node.data_dir = Some(v); }
        if let Some(v) = cli.storage_mode { self.storage.mode = Some(v); }
        if let Some(v) = cli.compact_eras { self.storage.compact_eras = Some(v); }
        if let Some(v) = cli.full_node_rpc { self.storage.full_node_rpc = Some(v); }
        if let Some(v) = cli.log_level { self.log.level = Some(v); }
        if let Some(v) = cli.log_json { self.log.json = Some(v); }
        if let Some(v) = cli.unlock_timeout { self.node.unlock_timeout = Some(v); }
    }

    /// Resolve the genesis path.
    #[must_use]
    pub fn genesis_path(&self) -> Option<&str> {
        self.genesis.as_deref()
    }

    // -----------------------------------------------------------------------
    // Flattened accessors (config → fallback to defaults)
    // -----------------------------------------------------------------------

    pub fn data_dir(&self) -> String {
        self.node
            .data_dir
            .clone()
            .unwrap_or_else(|| constants::default_data_dir().to_string_lossy().to_string())
    }

    pub fn unlock_timeout(&self) -> u64 {
        self.node.unlock_timeout.unwrap_or(constants::DEFAULT_UNLOCK_TIMEOUT_SECS)
    }

    pub fn p2p_port(&self) -> u16 {
        self.network.p2p_port.unwrap_or(constants::DEFAULT_P2P_PORT)
    }

    pub fn rpc_port(&self) -> u16 {
        self.network.rpc_port.unwrap_or(constants::DEFAULT_RPC_PORT)
    }

    pub fn rest_port(&self) -> u16 {
        self.network.rest_port.unwrap_or(constants::DEFAULT_REST_PORT)
    }

    pub fn bootnodes(&self) -> &[String] {
        self.network.bootnodes.as_deref().unwrap_or(&[])
    }

    pub fn storage_mode(&self) -> &str {
        self.storage.mode.as_deref().unwrap_or(constants::DEFAULT_STORAGE_MODE)
    }

    pub fn compact_eras(&self) -> u32 {
        self.storage.compact_eras.unwrap_or(constants::DEFAULT_COMPACT_ERAS)
    }

    pub fn full_node_rpc(&self) -> &[String] {
        self.storage.full_node_rpc.as_deref().unwrap_or(&[])
    }

    pub fn max_tx_per_account(&self) -> usize {
        self.mempool.max_tx_per_account.unwrap_or(constants::DEFAULT_MAX_TX_PER_ACCOUNT)
    }

    pub fn log_level(&self) -> &str {
        self.log.level.as_deref().unwrap_or("info")
    }

    pub fn log_json(&self) -> bool {
        self.log.json.unwrap_or(true)
    }

    pub fn log_file(&self) -> Option<&str> {
        self.log.file.as_deref()
    }

    // -----------------------------------------------------------------------
    // Validation
    // -----------------------------------------------------------------------

    /// Validate required fields.
    ///
    /// # Errors
    ///
    /// Returns `LibError::Storage` if identity or genesis rules are violated.
    pub fn validate(&self) -> Result<()> {
        match (self.key.as_ref(), self.key_file.as_ref(), self.observer) {
            (None, None, false) => {
                return Err(LibError::Storage(
                    "one of key, key_file, or observer must be set".to_string(),
                ));
            }
            (Some(_), Some(_), _) => {
                return Err(LibError::Storage(
                    "key and key_file are mutually exclusive".to_string(),
                ));
            }
            (_, _, true) if self.key.is_some() || self.key_file.is_some() => {
                return Err(LibError::Storage(
                    "observer cannot be combined with key or key_file".to_string(),
                ));
            }
            _ => {}
        }

        if self.genesis.is_none() {
            return Err(LibError::Storage(
                "genesis path must be configured (use --genesis or config file)"
                    .to_string(),
            ));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> NodeConfig {
        NodeConfig::default()
    }

    // -----------------------------------------------------------------------
    // Defaults
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_p2p_port() {
        assert_eq!(defaults().p2p_port(), 30333);
    }

    #[test]
    fn test_default_rpc_port() {
        assert_eq!(defaults().rpc_port(), 9944);
    }

    #[test]
    fn test_default_rest_port() {
        assert_eq!(defaults().rest_port(), 9933);
    }

    #[test]
    fn test_default_log_level() {
        assert_eq!(defaults().log_level(), "info");
    }

    #[test]
    fn test_default_log_json() {
        assert!(defaults().log_json());
    }

    #[test]
    fn test_default_unlock_timeout() {
        assert_eq!(defaults().unlock_timeout(), 20);
    }

    // -----------------------------------------------------------------------
    // YAML parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_yaml_full_config() {
        let yaml = r#"
key: my-validator
genesis: configs/genesis.devnet.json
node:
  data_dir: /tmp/mononium-data
  unlock_timeout: 30
network:
  p2p_port: 30444
  rpc_port: 9955
log:
  level: debug
  json: false
storage:
  mode: compact
"#;
        let cfg: NodeConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.key, Some("my-validator".to_string()));
        assert_eq!(cfg.genesis, Some("configs/genesis.devnet.json".to_string()));
        assert_eq!(cfg.p2p_port(), 30444);
        assert_eq!(cfg.rpc_port(), 9955);
        assert_eq!(cfg.log_level(), "debug");
        assert!(!cfg.log_json());
        assert_eq!(cfg.storage_mode(), "compact");
        assert_eq!(cfg.unlock_timeout(), 30);
    }

    #[test]
    fn test_yaml_with_bootnodes() {
        let yaml = r#"
observer: true
genesis: genesis.json
network:
  bootnodes:
    - /ip4/1.2.3.4/tcp/30333/p2p/QmX
    - /ip4/5.6.7.8/tcp/30333/p2p/QmY
"#;
        let cfg: NodeConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(cfg.observer);
        assert_eq!(cfg.bootnodes().len(), 2);
    }

    // -----------------------------------------------------------------------
    // TOML parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_toml_full_config() {
        let toml = r#"
key = "my-validator"
genesis = "configs/genesis.devnet.json"

[node]
data_dir = "/tmp/mononium-data"
unlock_timeout = 30

[network]
p2p_port = 30444
rpc_port = 9955

[log]
level = "debug"
json = false

[storage]
mode = "compact"
"#;
        let cfg: NodeConfig = toml::from_str(toml).unwrap();
        assert_eq!(cfg.key, Some("my-validator".to_string()));
        assert_eq!(cfg.p2p_port(), 30444);
        assert_eq!(cfg.rpc_port(), 9955);
        assert_eq!(cfg.log_level(), "debug");
        assert!(!cfg.log_json());
        assert_eq!(cfg.storage_mode(), "compact");
        assert_eq!(cfg.unlock_timeout(), 30);
    }

    // -----------------------------------------------------------------------
    // Merge CLI overrides
    // -----------------------------------------------------------------------

    #[test]
    fn test_merge_cli_overrides() {
        let mut cfg = defaults();
        let cli = CliOverrides {
            p2p_port: Some(30666),
            log_level: Some("trace".to_string()),
            ..Default::default()
        };
        cfg.merge_cli(cli);
        assert_eq!(cfg.p2p_port(), 30666);
        assert_eq!(cfg.log_level(), "trace");
        // Unchanged defaults
        assert_eq!(cfg.rpc_port(), 9944);
    }

    #[test]
    fn test_merge_cli_does_not_erase_existing() {
        let mut cfg = defaults();
        cfg.network.p2p_port = Some(30888);
        let cli = CliOverrides {
            observer: Some(true),
            ..Default::default()
        };
        cfg.merge_cli(cli);
        assert!(cfg.observer);
        assert_eq!(cfg.p2p_port(), 30888);
    }

    // -----------------------------------------------------------------------
    // Validation
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_no_identity_errors() {
        let mut cfg = defaults();
        cfg.genesis = Some("genesis.json".to_string());
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("one of key"), "got: {err}");
    }

    #[test]
    fn test_validate_mutually_exclusive() {
        let mut cfg = defaults();
        cfg.key = Some("alice".to_string());
        cfg.key_file = Some("/path/key.json".to_string());
        cfg.genesis = Some("genesis.json".to_string());
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("mutually exclusive"), "got: {err}");
    }

    #[test]
    fn test_validate_observer_with_key_errors() {
        let mut cfg = defaults();
        cfg.key = Some("alice".to_string());
        cfg.observer = true;
        cfg.genesis = Some("genesis.json".to_string());
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("observer"), "got: {err}");
    }

    #[test]
    fn test_validate_no_genesis_errors() {
        let mut cfg = defaults();
        cfg.key = Some("alice".to_string());
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("genesis"), "got: {err}");
    }

    #[test]
    fn test_validate_observer_passes() {
        let mut cfg = defaults();
        cfg.observer = true;
        cfg.genesis = Some("genesis.json".to_string());
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_validator_passes() {
        let mut cfg = defaults();
        cfg.key = Some("alice".to_string());
        cfg.genesis = Some("genesis.json".to_string());
        assert!(cfg.validate().is_ok());
    }

    // -----------------------------------------------------------------------
    // File loading
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_yaml_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_mononium_cfg.yaml");
        std::fs::write(&path, "key: test\nobserver: false\ngenesis: test.json\n").unwrap();
        let cfg = NodeConfig::load(&path).unwrap();
        assert_eq!(cfg.key, Some("test".to_string()));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_toml_file() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_mononium_cfg.toml");
        std::fs::write(&path, "key = \"test\"\nobserver = false\ngenesis = \"test.json\"\n").unwrap();
        let cfg = NodeConfig::load(&path).unwrap();
        assert_eq!(cfg.key, Some("test".to_string()));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_unsupported_extension() {
        let dir = std::env::temp_dir();
        let path = dir.join("config.json");
        std::fs::write(&path, "{}").unwrap();
        let err = NodeConfig::load(&path).unwrap_err();
        assert!(err.to_string().contains("unsupported"), "got: {err}");
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_load_nonexistent_path() {
        let err = NodeConfig::load(Path::new("/nonexistent/mononium/config.yaml")).unwrap_err();
        assert!(err.to_string().contains("cannot read config"), "got: {err}");
    }

    // -----------------------------------------------------------------------
    // Accessor methods
    // -----------------------------------------------------------------------

    #[test]
    fn test_data_dir_default() {
        assert!(!defaults().data_dir().is_empty());
    }

    #[test]
    fn test_data_dir_from_config() {
        let mut cfg = defaults();
        cfg.node.data_dir = Some("/custom/path".to_string());
        assert_eq!(cfg.data_dir(), "/custom/path");
    }

    #[test]
    fn test_storage_mode_default() {
        assert_eq!(defaults().storage_mode(), "full");
    }

    #[test]
    fn test_storage_mode_from_config() {
        let mut cfg = defaults();
        cfg.storage.mode = Some("archive".to_string());
        assert_eq!(cfg.storage_mode(), "archive");
    }

    #[test]
    fn test_compact_eras_default() {
        assert_eq!(defaults().compact_eras(), 2);
    }

    #[test]
    fn test_compact_eras_from_config() {
        let mut cfg = defaults();
        cfg.storage.compact_eras = Some(3);
        assert_eq!(cfg.compact_eras(), 3);
    }

    #[test]
    fn test_full_node_rpc_default() {
        assert!(defaults().full_node_rpc().is_empty());
    }

    #[test]
    fn test_full_node_rpc_from_config() {
        let mut cfg = defaults();
        cfg.storage.full_node_rpc = Some(vec!["alice".to_string()]);
        assert_eq!(cfg.full_node_rpc(), &["alice"]);
    }

    #[test]
    fn test_max_tx_per_account_default() {
        assert_eq!(defaults().max_tx_per_account(), 50);
    }

    #[test]
    fn test_max_tx_per_account_from_config() {
        let mut cfg = defaults();
        cfg.mempool.max_tx_per_account = Some(500);
        assert_eq!(cfg.max_tx_per_account(), 500);
    }

    #[test]
    fn test_log_file_default() {
        assert!(defaults().log_file().is_none());
    }

    #[test]
    fn test_log_file_from_config() {
        let mut cfg = defaults();
        cfg.log.file = Some("/var/log/mononium.log".to_string());
        assert_eq!(cfg.log_file(), Some("/var/log/mononium.log"));
    }

    #[test]
    fn test_bootnodes_default() {
        assert!(defaults().bootnodes().is_empty());
    }

    #[test]
    fn test_genesis_path() {
        let mut cfg = defaults();
        assert!(cfg.genesis_path().is_none());
        cfg.genesis = Some("custom.json".to_string());
        assert_eq!(cfg.genesis_path(), Some("custom.json"));
    }

    // -----------------------------------------------------------------------
    // CLI overrides: all fields
    // -----------------------------------------------------------------------

    #[test]
    fn test_merge_cli_all_fields() {
        let mut cfg = defaults();
        let cli = CliOverrides {
            genesis: Some("cli-genesis.json".to_string()),
            key: Some("cli-key".to_string()),
            key_file: Some("cli-key-file".to_string()),
            observer: Some(true),
            p2p_port: Some(30666),
            rpc_port: Some(9955),
            rest_port: Some(9944),
            bootnodes: Some(vec!["/ip4/1.2.3.4".to_string()]),
            data_dir: Some("/cli/data".to_string()),
            storage_mode: Some("archive".to_string()),
            compact_eras: Some(5),
            full_node_rpc: Some(vec!["bob".to_string()]),
            log_level: Some("trace".to_string()),
            log_json: Some(false),
            unlock_timeout: Some(120),
        };
        cfg.merge_cli(cli);
        assert_eq!(cfg.genesis, Some("cli-genesis.json".to_string()));
        assert_eq!(cfg.key, Some("cli-key".to_string()));
        assert_eq!(cfg.key_file, Some("cli-key-file".to_string()));
        assert!(cfg.observer);
        assert_eq!(cfg.p2p_port(), 30666);
        assert_eq!(cfg.rpc_port(), 9955);
        assert_eq!(cfg.rest_port(), 9944);
        assert_eq!(cfg.bootnodes(), &["/ip4/1.2.3.4"]);
        assert_eq!(cfg.data_dir(), "/cli/data");
        assert_eq!(cfg.storage_mode(), "archive");
        assert_eq!(cfg.compact_eras(), 5);
        assert_eq!(cfg.full_node_rpc(), &["bob"]);
        assert_eq!(cfg.log_level(), "trace");
        assert!(!cfg.log_json());
        assert_eq!(cfg.unlock_timeout(), 120);
    }

    #[test]
    fn test_merge_cli_observer_false() {
        let mut cfg = defaults();
        cfg.observer = true;
        cfg.merge_cli(CliOverrides { observer: Some(false), ..Default::default() });
        assert!(!cfg.observer);
    }

    // -----------------------------------------------------------------------
    // Validation: observer only (passes)
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_key_file_passes() {
        let mut cfg = defaults();
        cfg.key_file = Some("/path/key.json".to_string());
        cfg.genesis = Some("genesis.json".to_string());
        assert!(cfg.validate().is_ok());
    }
}
