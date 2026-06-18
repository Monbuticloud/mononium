# Mononium

**Post-quantum PoS L1 for low-end VPS and containers.** Falcon-512 signatures, BFT finality, built in Rust.

## Overview

Mononium is a Layer 1 blockchain designed to run on low-end VPS hardware, not data centers. Every design decision targets low CPU, low RAM, and low bandwidth. Post-quantum signatures (Falcon-512) are the primary signing scheme from day one, eliminating the need for a future migration.

Development and contribution are designed around containers — spin up a localnet in seconds without installing toolchains or managing dependencies.

### Key Decisions

| Area               | Choice                                                               | Why                                                |
| ------------------ | -------------------------------------------------------------------- | -------------------------------------------------- |
| Validator hardware | **1-2 vCPU, ~100 MB RAM**                                            | Low-cost VPS accessible, not data-center exclusive |
| Signatures         | **Falcon-512**                                                       | Post-quantum secure from day one, NIST Level I     |
| Consensus          | PoS, BFT commit per block                                            | Finality in ~20s (4 blocks)                        |
| Database           | redb (embedded)                                                      | Pure Rust, ACID, memory-mapped, MIT license        |
| State model        | Account-based (SMT, BLAKE3)                                          | Simpler than UTXO for V1                           |
| Networking         | libp2p (gossipsub + kademlia)                                        | Established P2P stack                              |
| Serialization      | SCALE (wire) + JSON (RPC)                                            | Compact wire format, human-readable API            |
| Token              | MONEX (U256, 18 decimals)                                            | Standard account model                             |
| Supply             | Fixed (dev tiers) / Capped inflation (mainnet, 3.5% annual, 10B cap) | Fair launch, no pre-mine                           |
| Fees               | Flat + per-byte + tip, distributed pro-rata by stake                 | Rewards all active validators proportionally       |
| CLI                | clap derive                                                          | Standard Rust CLI framework                        |
| GUI                | Iced (deferred to Phase 3)                                           | Native desktop application                         |

### Architecture

```
mononium/
├── Cargo.toml                  # workspace root
├── mononium-rust-lib/          # core library (all blockchain logic)
├── mononium-cli/               # CLI binary (node daemon + wallet)
└── mononium-gui/               # GUI binary (desktop app)
```

- `mononium-rust-lib`, types, state machine, consensus engine, crypto, storage, P2P, RPC
- `mononium-cli`, node daemon, wallet keygen, transfer, staking commands
- `mononium-gui`, connects to a running node via RPC (does not run a node itself)

### Consensus Flow

```
Bootstrap key (genesis-designated, N blocks)
  → Era 0: Open registration, no stake minimum
    → Era 1+: Top-N by stake, minimum 1 MONEX
```

- **Block time:** 5 seconds (fixed)
- **Finality:** ~20 seconds (BFT commit, 2/3+ validator signatures)
- **Eras:** 720 blocks (~1 hour), validator set recalculation at each boundary
- **Proposer election:** Round-robin (V1), VRF planned (V2+)
- **Slashing:** Equivocation only (V1), 90% burned, 10% reporter bounty (staked)
- **Missed slot penalty:** 0.08 MONEX flat, applied at era boundary

### Network Tiers

| Tier     | Chain ID | Supply                       | Purpose                 |
| -------- | -------- | ---------------------------- | ----------------------- |
| Localnet | 0        | 10 MONEX                     | Single-node development |
| Devnet   | 1        | 100 MONEX per key (3-5 keys) | Multi-validator testing |
| Testnet  | 2        | 100 MONEX                    | Public test network     |
| Mainnet  | 3        | 0 MONEX (inflation)          | Production              |

## Key Security

Private keys are protected with memory-hard password derivation (Argon2id, 512 MiB memory, 4 iterations, 4 parallel). The 48-byte Falcon-512 seed is encrypted at rest using NaCl secretbox (XSalsa20-Poly1305) and stored at `~/.mononium/keys/{name}.json`. The public key (897 bytes) is stored in plaintext — it is public by definition.

Initial key unlock on node startup requires ~2.5-5s due to Argon2id memory cost. This is a one-time operation at node launch. After unlock, the validator runs at the standard low-memory profile.

## Resource Profile

- **Idle validator:** ~100 MB RAM target
- **Startup/unlock:** up to ~512 MB RAM (Argon2 key derivation)
- **Runtime:** optimized for low-memory VPS environments

## Project Status

**Planning phase (V0.x.x).** No code exists yet. Architecture and protocol decisions are being finalized through structured review sessions. See `docs/plans/GRILL.md` for open questions.

| Phase   | What ships                                         | Target date      |
| ------- | -------------------------------------------------- | ---------------- |
| Phase 1 | `mononium-rust-lib` + `mononium-cli` (single-node) | 2026-06-17 start |
| Phase 2 | Multi-validator localnet                           | After Phase 1    |
| Phase 3 | Devnet + GUI begins                                | After Phase 2    |
| Phase 4 | Public testnet                                     | After Phase 3    |
| Phase 5 | Mainnet + GUI v1.0                                 | After Phase 4    |

## Quick Start

### From source

```bash
git clone https://github.com/mononium/mononium
cd mononium
cargo build --release

# Generate a validator key
./target/release/mononium-cli wallet keygen --name my-validator

# Start a single-node localnet
./target/release/mononium-cli node --genesis configs/genesis.localnet.json --key my-validator
```

### With Docker (no toolchain needed)

```bash
docker run --rm -v $PWD/configs:/configs ghcr.io/mononium/cli node \
  --genesis /configs/genesis.localnet.json --key my-validator
```

## RPC Interface

jsonrpsee + REST hybrid on port 9944. Namespaces: `chain` (blocks, headers), `state` (balance, nonce), `tx` (submit, status), `validator` (set, stake). See `docs/rpc/` for full spec.

## Testing

Five-tier pyramid: unit → integration → property-based (proptest) → fuzz (cargo-fuzz) → invariant benchmarks (criterion). Every invariant in the spec must have a corresponding test. See `docs/plans/V0.4.0/Testing.md`.

## Contributing

PRs welcome. All dependencies must be MIT, Apache-2.0, BSD, or Zlib licensed — `cargo-deny` enforces this on every PR. For architecture discussions, open an issue or start an ADR under `docs/architecture/`.

## Development

```bash
# Build all crates
cargo build

# Run tests
cargo nextest run -p mononium-rust-lib

# Run benchmarks
cargo bench -p mononium-rust-lib

# Lint
cargo clippy -p mononium-rust-lib -- -D warnings
```

## Documentation

- `docs/plans/V0.4.0/` — Current planning docs (Philosophy, Architecture, Consensus, Protocol, Cryptography, Network, Storage, Validators, Testing, Roadmap, NodeConfig)
- `docs/architecture/` — ADRs (Architectural Decision Records)
- `docs/plans/GRILL.md` — Open questions and deferred decisions

## License

AGPL-3.0-only. See [LICENSE](LICENSE).
