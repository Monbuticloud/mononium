# Mononium User Guide

Mononium is a Layer 1 blockchain built in Rust. This guide covers running a node, staking, and network operations.

## Quick Start

```bash
# Build
cargo build --release -p mononium-cli

# Run a localnet validator (single node)
cargo run --release -p mononium-cli -- node --genesis configs/genesis.localnet.json

# Run an observer (sync-only, no signing)
cargo run --release -p mononium-cli -- node --genesis configs/genesis.localnet.json --observer
```

## Node Configuration

### Config File (recommended)

```bash
mononium-cli node --config configs/node.localnet.yaml
```

Pre-built configs:

| File | Use Case |
|------|----------|
| `configs/node.localnet.yaml` | Single validator, mDNS enabled, no bootnodes |
| `configs/node.devnet.yaml` | Multi-validator devnet, mDNS disabled, explicit bootnodes |
| `configs/node.observer.yaml` | Observer node (sync-only, no signing) |

### CLI Flags

All config values can be overridden via CLI flags:

```
mononium-cli node [FLAGS]

Flags:
      --config <PATH>          Config file path (YAML or TOML)
      --genesis <PATH>         Genesis JSON (overrides config)
      --key <NAME>             Validator key name
      --key-file <PATH>        Absolute path to key file
      --observer               Observer mode (no signing, sync-only)
      --p2p-port <PORT>        P2P libp2p port (default: 30333)
      --rpc-port <PORT>        JSON-RPC port (default: 9944, 0=disable)
      --rest-port <PORT>       REST HTTP port (default: 9933)
      --bootnode <MULTIADDR>   Bootstrap peer (repeatable)
      --data-dir <PATH>        Data directory
      --log-level <LEVEL>      Log level (trace|debug|info|warn|error)
```

## Wallet

```bash
# Generate a key pair
mononium-cli wallet keygen my-validator

# Check balance
mononium-cli wallet balance <address> --node http://localhost:9933

# Send MONEX
mononium-cli wallet transfer <recipient> 10.5 --key my-validator --node http://localhost:9933
```

### Staking

```bash
# Register as validator
mononium-cli wallet register --key my-validator --node http://localhost:9933

# Stake to a validator
mononium-cli wallet stake <validator-addr> 1000 --key my-validator --node http://localhost:9933

# Atomic register + self-stake
mononium-cli wallet register-and-stake 1000 --key my-validator --node http://localhost:9933

# Unstake (168-era cooldown)
mononium-cli wallet unstake <validator-addr> 500 --key my-validator --node http://localhost:9933
```

## REST API

| Endpoint | Description |
|----------|-------------|
| `GET /health` | Node health + current height |
| `GET /height` | Current block height |
| `GET /era` | Current era |
| `GET /block/latest` | Latest block |
| `GET /block/{height}` | Block by height |
| `GET /block/hash/{hash}` | Block by 32-byte hex hash |
| `GET /balance/{address}` | Account balance + nonce |
| `GET /nonce/{address}` | Account nonce |
| `GET /validator/{address}` | Validator info |
| `GET /validators` | All validators |
| `GET /genesis` | Genesis block hash |
| `POST /tx` | Submit transaction (SCALE-hex) |

## JSON-RPC (WebSocket)

Connect to `ws://localhost:9944` for JSON-RPC with 12 methods + 3 subscriptions.

### Methods

| Method | Params | Returns |
|--------|--------|---------|
| `chain_get_health` | — | `{ status, height, peers, finalized_height }` |
| `chain_get_height` | — | `u64` |
| `chain_get_genesis` | — | `Hash` |
| `era_current` | — | `u64` |
| `state_get_balance` | `Address` | `U256` |
| `state_get_nonce` | `Address` | `u64` |
| `validator_stake` | `Address` | `U256` |
| `validator_set` | — | `Vec<ValidatorInfo>` |
| `block_latest` | — | `BlockHeader` |
| `block_header` | `BlockId` | `BlockHeader` |
| `block_get` | `BlockId` | `Block` |
| `tx_submit` | `Transaction` | `TxHash` |

### Subscriptions

| Method | Event |
|--------|-------|
| `subscribe_blocks` | `BlockHeader` |
| `subscribe_finality` | `FinalityEvent` |
| `subscribe_votes` | `CommitVote` |

### BlockId format

- Number: `{ "number": 42 }`
- `"latest"`: most recent block
- Hex hash: `"0xabcd..."` (32-byte block hash)
