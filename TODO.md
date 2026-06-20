# Mononium Phase 1 — Implementation Tracker

> **Goal:** `mononium-cli node` produces blocks locally. `mononium-cli wallet transfer` sends txs.
> **Approach:** TDD (Red → Green → Refactor per feature), dependency order, smaller sub-phases.

---

## Sub-phase 1.0 ✅ Foundation Types
- [x] Test: Account creation and field access
- [x] Test: Address formatting and checksum round-trip
- [x] Test: Address parse invalid/missing/too-short
- [x] Test: U256 constant correctness
- [x] Impl: `constants.rs` — chain-wide constants
- [x] Impl: `error.rs` — LibError enum
- [x] Impl: `core/constants.rs` — core constants (denomination, fees, supply)
- [x] Impl: `core/account.rs` — Account, Address types with checksum parsing
- [x] Rename: mononium-rust-lib → mononium-lib across workspace + docs
- [x] Fix deps: libp2p 0.56, primitive-types 0.12 features, workspace lints
- [x] 18 tests passing, clippy clean, committed

## Sub-phase 1.1 🔶 Cryptography (Falcon-512, BLAKE3, SignatureScheme)
- [ ] Test: SignatureScheme trait round-trip (sign → verify)
- [ ] Test: BLAKE3 wrapper produces correct 32-byte hash
- [ ] Test: Address derivation from public key
- [ ] Impl: `crypto/signature.rs` — SignatureScheme trait
- [ ] Impl: `crypto/falcon.rs` — Falcon512 impl
- [ ] Impl: `crypto/hash.rs` — BLAKE3 wrappers
- [ ] Impl: `crypto/address.rs` — Address derivation from pubkey
- [ ] Impl: `crypto/mod.rs` — re-exports
- [ ] Tests pass: `cargo nextest run -p mononium-rust-lib`

## Sub-phase 1.2 🔶 Sparse Merkle Tree
- [ ] Test: SMT insert → get returns same value
- [ ] Test: SMT root is deterministic for same inserts
- [ ] Test: SMT proof round-trip (prove → verify)
- [ ] Test: SMT empty root
- [ ] Impl: `crypto/trie.rs` — Trie trait + SMT impl
- [ ] Tests pass: `cargo nextest run -p mononium-rust-lib`

## Sub-phase 1.3 🔶 Transaction & Block Types + Fee
- [ ] Test: Transaction SCALE encode/decode symmetric
- [ ] Test: Transaction JSON serde round-trip
- [ ] Test: Block header hash depends on all fields
- [ ] Test: CommitVote SCALE encode/decode
- [ ] Test: Fee calculation is deterministic
- [ ] Test: Burn fee bypasses standard calculation
- [ ] Impl: `core/transaction.rs` — Transaction, TxBody enum
- [ ] Impl: `core/block.rs` — Block, BlockHeader, CommitVote
- [ ] Impl: `core/fee.rs` — FeePolicy + HybridFee
- [ ] Tests pass

## Sub-phase 1.4 🔶 State Machine
- [ ] Test: Block application order (validate → apply → distribute fees → commit)
- [ ] Test: Failed tx still pays fee
- [ ] Test: Fee distribution pro-rata by stake
- [ ] Test: Deterministic state root after block
- [ ] Impl: `core/state.rs` — StateMachine
- [ ] Tests pass

## Sub-phase 1.5 🔶 Storage (redb)
- [ ] Test: StorageEngine trait contract
- [ ] Test: redb put/get/delete round-trip
- [ ] Test: Genesis loading from JSON
- [ ] Test: Duplicate genesis detection
- [ ] Impl: `storage/redb.rs` — RedbEngine
- [ ] Impl: `storage/tables.rs` — table definitions
- [ ] Impl: `storage/genesis.rs` — genesis JSON loading
- [ ] Impl: `storage/mod.rs` — StorageEngine trait
- [ ] Tests pass

## Sub-phase 1.6 🔶 Mempool
- [ ] Test: Insert, remove, select by tip→time→nonce
- [ ] Test: TTL eviction
- [ ] Test: Nonce buffering
- [ ] Test: Per-sender cap
- [ ] Impl: `mempool/mod.rs`
- [ ] Impl: `mempool/ordering.rs`
- [ ] Tests pass

## Sub-phase 1.7 🔶 Consensus Basics
- [ ] Test: Top-N election sorts by stake desc, tie by registration
- [ ] Test: Round-robin proposer schedule
- [ ] Test: Era boundary transitions
- [ ] Test: Fixed supply = zero block reward
- [ ] Impl: `consensus/election.rs` — ValidatorElection trait + TopNElection
- [ ] Impl: `consensus/proposer.rs` — ProposerSelection + RoundRobin
- [ ] Impl: `consensus/era.rs` — Era calculation, ElectionMode
- [ ] Impl: `consensus/supply.rs` — SupplyPolicy + FixedSupply + CappedInflation
- [ ] Impl: `consensus/mod.rs` — ConsensusConfig
- [ ] Tests pass

## Sub-phase 1.8 🔶 Config + Genesis Files
- [ ] Test: Config file loading (YAML + TOML)
- [ ] Test: CLI flag override precedence
- [ ] Test: Genesis JSON parsing
- [ ] Impl: `config/mod.rs` — Config struct, load/merge
- [ ] Impl: `config/constants.rs` — default ports, paths
- [ ] Config files: `configs/genesis.localnet.json`
- [ ] Config files: `configs/genesis.devnet.json`
- [ ] Config files: `configs/genesis.testnet.json`
- [ ] Config files: `configs/node.devnet.yaml`
- [ ] Tests pass

## Sub-phase 1.9 🔶 CLI Node Daemon
- [ ] Test: CLI arg parsing (clap)
- [ ] Impl: `mononium-cli/src/main.rs` — CLI structure (node, wallet, query)
- [ ] Impl: `mononium-cli/src/node.rs` — startup lifecycle
- [ ] `mononium-cli node` starts and produces blocks locally
- [ ] REST API: `/health`, `/block/latest`, `/balance/{address}`
- [ ] Tests pass

## Sub-phase 1.10 🔶 CLI Wallet
- [ ] Test: Keygen produces valid Falcon-512 key
- [ ] Test: Key file JSON format
- [ ] Impl: Wallet keygen command
- [ ] Impl: Wallet transfer command
- [ ] Impl: Wallet balance command
- [ ] `mononium-cli wallet keygen` generates keys
- [ ] `mononium-cli wallet transfer` sends txs
- [ ] `mononium-cli wallet balance` queries state

---

## Phase 1 Exit Criteria
- [ ] `cargo build -p mononium-rust-lib` passes
- [ ] `cargo build -p mononium-cli` passes
- [ ] `cargo nextest run -p mononium-rust-lib` passes
- [ ] `cargo clippy -p mononium-rust-lib -- -D warnings` passes
- [ ] `mononium-cli node` starts and produces blocks on localnet
- [ ] `mononium-cli wallet keygen` generates Falcon-512 keys
- [ ] `mononium-cli wallet transfer` creates signed txs
- [ ] `mononium-cli wallet balance` queries account state via REST
