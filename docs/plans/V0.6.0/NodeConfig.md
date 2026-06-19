# Node Configuration

## Config File

Node configuration is a single YAML or TOML file separate from the genesis JSON. Config covers operator-specific settings (keys, ports, paths, overrides). Genesis covers chain state.

### Format

Both YAML and TOML are supported. The loader detects format by file extension (`.yaml` / `.toml`).

### Search Order

| Priority | Source                                      |
| -------- | ------------------------------------------- |
| 1        | `--config /path/to/file.{yaml,toml}`        |
| 2        | `$MONONIUM_CONFIG` env var pointing to file |
| 3        | `~/.mononium/config.yaml`                   |
| 4        | `~/.mononium/config.toml`                   |
| 5        | No config → all defaults + required CLI     |

**YAML vs TOML tiebreak:** If both `config.yaml` and `config.toml` exist in the same directory (both at step 3 or both at step 4), **YAML wins**. The loader checks `.yaml` before `.toml`.

### Override Precedence

```
CLI flags  >  config file  >  built-in defaults
```

No per-field env var overrides.

---

## Schema

### YAML

```yaml
key: my-validator # XOR with key_file
key_file: /path/to/unencrypted-key # XOR with key
observer: false # true = no signing, sync-only

genesis: configs/genesis.devnet.json # required, no default

node:
  data_dir: ~/.mononium/data

network:
  p2p_port: 30333
  rpc_port: 9944
  rest_port: 9933
  bootnodes: [] # empty = mDNS only (localnet)

storage:
  mode: full # full (default) | compact
  compact_eras: 2 # eras to keep fully before compacting (compact only)
  full_node_rpc: # proxy for historical queries (compact only)
    - https://rpc.mononium.io

mempool:
  min_fee: 0.0667 # MONEX, local mempool filter only
  max_tx_per_account: 50 # per-block rate limit per account

log:
  level: info # trace | debug | info | warn | error
  json: true # JSON or human-readable
  file: ~/.mononium/node.log # optional, stdout if absent
```

### TOML

```toml
key = "my-validator"
key_file = "/path/to/unencrypted-key"
observer = false
genesis = "configs/genesis.devnet.json"

[node]
data_dir = "~/.mononium/data"

[network]
p2p_port = 30333
rpc_port = 9944
rest_port = 9933
bootnodes = []

[storage]
mode = "full"
compact_eras = 2
full_node_rpc = ["https://rpc.mononium.io"]

[mempool]
min_fee = 0.0667
max_tx_per_account = 50

[log]
level = "info"
json = true
file = "~/.mononium/node.log"
```

---

## Field Reference

### `key` / `key_file` / `observer`

Exactly one of:

- `key: "name"` → validator mode, loads `~/.mononium/keys/{name}.json`
- `key_file: "/path/k.json"` → validator mode, absolute path to key file
- `observer: true` → observer mode, no key, no signing, sync + RPC only

`key` and `key_file` are mutually exclusive (error if both provided).  
`observer: true` combined with `key` or `key_file` is an error.

### `genesis`

Path to genesis JSON file. Required — no convention-based lookup. If neither config file nor `--genesis` flag provides it, the node errors with a message pointing to `configs/` in the repo.

### `node.data_dir`

Default: `~/.mononium/data/`

Database and state files are stored under `{data_dir}/{chain_id}/`. The network subdirectory is derived from the genesis `chain_id`.

### `network.*`

| Field       | Default | Description                       |
| ----------- | ------- | --------------------------------- |
| `p2p_port`  | 30333   | libp2p listener                   |
| `rpc_port`  | 9944    | jsonrpsee WebSocket               |
| `rest_port` | 9933    | axum REST HTTP                    |
| `bootnodes` | `[]`    | Multiaddr list. Empty = mDNS only |

Empty `bootnodes` is valid. On localnet, mDNS handles discovery. On multi-node networks, the operator must supply bootnodes or use RPC `add_peer`.

### `storage.*`

| Field           | Default | Description                                                        |
| --------------- | ------- | ------------------------------------------------------------------ |
| `mode`          | `full`  | `full` = everything forever. `compact` = headers only after N eras |
| `compact_eras`  | `2`     | Eras to keep in full before compressing (compact mode only)        |
| `full_node_rpc` | `[]`    | Upstream RPC endpoints for historical tx lookups (compact only)    |

**Full mode:** All blocks, transactions, votes, and state retained since genesis. Default for all operators. Requires ~17 GB/day raw storage — plan disk accordingly.

**Compact mode:** Keeps the last `compact_eras` eras in full. Older blocks are compressed to headers only (height, state_root, parent_hash, timestamp — ~100 bytes/block). Historical transaction and state queries are proxied to the configured `full_node_rpc` endpoints. Intended for resource-constrained VPS operators who accept the trade-off of external dependency for historical data.

### `mempool.min_fee`

Default: `0.0667` MONEX (667_000_000_000_000_000_000_000_000_000_000 MOXX)

Local node policy only — not a consensus parameter. Transactions below this threshold are rejected from the local mempool but remain valid in blocks.

### `mempool.max_tx_per_account`

Default: `50`

Per-account rate limit per block at the mempool level. Local node policy. Combined with the 1 MONEX anti-spam deposit, prevents a single account from congesting a block.

### `log.*`

| Field   | Default | Description                                           |
| ------- | ------- | ----------------------------------------------------- |
| `level` | `info`  | trace / debug / info / warn / error                   |
| `json`  | `true`  | JSON output (machine-parseable). `false` = human text |
| `file`  | none    | Optional file path. Omit = stdout only                |

JSON log output can be converted to human-readable text via `mononium-cli logfmt < node.log`. Until the `logfmt` tool is built, pipe through `jq -r '.level + " " + .msg'` for quick inspection.

---

## CLI Flag Inventory

```
mononium-cli node \
  --config <path>          # override config file path (yaml/toml)
  --genesis <path>         # override genesis JSON path
  --key <name>             # validator key name (XOR key-file / --observer)
  --key-file <path>        # absolute path to key file
  --observer               # observer mode, no signing
  --p2p-port <port>        # libp2p port (default 30333)
  --rpc-port <port>        # JSON-RPC WebSocket (default 9944)
  --rest-port <port>       # REST HTTP (default 9933)
  --bootnodes <multiaddr>  # bootstrap peer(s), repeatable
  --data-dir <path>        # data directory (default ~/.mononium/data/)
  --storage-mode <full|compact>  # storage retention mode
  --compact-eras <n>             # eras to keep before compacting (compact mode)
  --full-node-rpc <url>          # upstream RPC for historical queries (repeatable)
  --log-level <level>            # log level override
  --log-format <text|json>       # log format override (interim: pipe through `jq`)

mononium-cli logfmt < node.log          # convert JSON logs to human-readable text
tail -f node.log | jq -r '.level + " " + .msg'   # quick live inspection
```

All CLI flags override the corresponding config file field.

---

## Validation Rules

1. Exactly one of `key`, `key_file`, or `observer: true` must be specified
2. `key` and `key_file` are mutually exclusive
3. `observer: true` with `key` or `key_file` → error
4. `genesis` must be provided (config file or `--genesis` flag)
5. Config file must have `.yaml` or `.toml` extension
6. If config file not found at specified path → hard error (not a fallback)
