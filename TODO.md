# Mononium Phase 1 — Implementation Tracker

> **Goal:** `mononium-cli node` produces blocks locally. `mononium-cli wallet transfer` sends txs.
> **Approach:** TDD (Red → Green → Refactor per feature), dependency order, smaller sub-phases.
> **Commit cadence:** per test, per function, even if tests fail (RED/GREEN per commit).

---

## Sub-phase 1.0 ✅ Foundation Types (commit `c8762e4`)

- [x] `constants.rs` — chain-wide constants
- [x] `error.rs` — LibError enum
- [x] `core/constants.rs` — core constants (denomination, fees, supply)
- [x] `core/account.rs` — Account, Address types with checksum parsing
- [x] Rename: mononium-rust-lib → mononium-lib across workspace + docs
- [x] Fix deps: libp2p 0.56, primitive-types 0.12 features, workspace lints
- [x] 18 tests passing, clippy clean

## Sub-phase 1.1 ✅ Cryptography (commit `e4f62a1`)

- [x] `crypto/constants.rs` — key/signature size constants (48/1281/897/809)
- [x] `crypto/signature.rs` — SignatureScheme trait
- [x] `crypto/falcon.rs` — Falcon512 impl (generate, sign, verify, from_private_key)
- [x] `crypto/hash.rs` — BLAKE3 utilities (hash, hash_pair, keyed_hash, derive_key, batch_hash)
- [x] `crypto/address.rs` — Address derivation from pubkey
- [x] `crypto/mod.rs` — module re-exports
- [x] 62 total tests passing, clippy clean

## Sub-phase 1.2 ✅ Sparse Merkle Tree (commits `352c962` → `f048af6`)

- [x] Test: empty SMT root equals precomputed 256-level default hash (RED)
- [x] Impl: `root()` with lazy computation + 256-level default (GREEN)
- [x] Test: insert → get, unknown key, overwrite (GREEN)
- [x] Test: multiple keys, deterministic root, caching (GREEN)
- [x] `Trie` trait (get, insert, root, prove as todo!)
- [x] Namespace helpers (NS_ACCOUNTS `0x00`, NS_VALIDATORS `0x01`, NS_META `0x02`)
- [x] 79 total tests passing, clippy clean

## Sub-phase 1.3 ✅ Transaction & Block Types + Fee (commits `9923206` → `fe4b6a4`)

- [x] `core/transaction.rs` — Transaction, TxBody enum, BurnTarget, Falcon512Signature SCALE/JSON
- [x] `core/block.rs` — BlockHeader, Block, BlockBody, CommitVote SCALE/JSON
- [x] `core/fee.rs` — FeePolicy trait, HybridFee impl, burn bypass (flat 10 MOXX)
- [x] 105 total tests passing, clippy clean

## Sub-phase 1.4 ✅ State Machine (commits starting from `7d192d9`)

- [x] `core/state.rs` — StateMachine with SMT-backed accounts
- [x] Account CRUD (get/insert with namespace prefix `0x00`)
- [x] `apply_block()` — validates chain_id, executes Transfer/Burn, tracks fees
- [x] Failed tx: deducts fee only (wrong nonce, insufficient balance)
- [x] Burn: sends to permanent burn address with flat fee
- [x] Multiple txs in a block with sequential nonces
- [x] 117 total tests passing, clippy clean

## Sub-phase 1.5 ✅ Storage (redb)

- [x] `storage/mod.rs` — StorageEngine trait (open, put, get, delete, exists, list_keys)
- [x] `storage/tables.rs` — table name constants (ACCOUNTS, BLOCKS, TXS, VOTES, VALIDATORS, META)
- [x] `storage/redb.rs` — RedbEngine wrapping redb::Database
- [x] `storage/genesis.rs` — GenesisConfig + load_genesis from JSON
- [x] Duplicate genesis detection (META table marker)
- [x] 5 test groups: StorageEngine contract (put/get/delete/list_keys/isolation), genesis chain_id, accounts, validators, duplicate rejection, error handling
- [x] 134 total tests passing, clippy clean

## Sub-phase 1.6 ✅ Mempool

- [x] `mempool/ordering.rs` — PoolTx wrapper, cmp_priority (fee desc → time asc → nonce asc)
- [x] `mempool/mod.rs` — Mempool struct with config, insert/remove/select/evict/contains/len
- [x] Constraints: max_size, min_fee, per_sender_cap (insert), per_sender_cap (select), TTL expiry
- [x] Duplicate (sender + nonce) rejection, empty-pool select
- [x] 24 mempool tests: priority ordering (fee/time/nonce), insert, remove, select order, per-sender cap, TTL eviction, mixed eviction, sender query
- [x] 159 total tests passing

## Sub-phase 1.7 ✅ Consensus Basics

- [x] `consensus/election.rs` — ValidatorElection trait, TopNElection, ValidatorCandidate, ElectionMode
- [x] `consensus/proposer.rs` — ProposerSelection trait, RoundRobin
- [x] `consensus/era.rs` — Era calculation, is_era_boundary, election_mode_for_era
- [x] `consensus/supply.rs` — SupplyPolicy trait, FixedSupply (zero reward), CappedInflation (55.5/block flat → decaying)
- [x] `consensus/mod.rs` — ConsensusConfig
- [x] 33 new tests: Top-N sort/max/ties/empty/zero, round-robin cycles/single/large/panic, era calc/boundary/modes/starts, fixed supply, capped inflation math at various supply levels
- [x] 192 total tests passing, clippy clean

## Sub-phase 1.8 ✅ Config + Genesis Files

- [x] `config/mod.rs` — NodeConfig with nested sections, YAML/TOML serde, CliOverrides merging, validation
- [x] `config/constants.rs` — default ports (30333/9944/9933), paths, Argon2 params (256 MiB / 16 iters), storage
- [x] `configs/genesis.localnet.json` — single validator bootstrap
- [x] `configs/genesis.devnet.json` — 3-validator bootstrap with stakes
- [x] `configs/genesis.testnet.json` — single validator bootstrap
- [x] `configs/node.devnet.yaml` — complete node config example
- [x] 20 new tests: defaults, YAML parse, TOML parse, CLI merge rules, validation rules, file loading
- [x] 212 total tests passing, clippy clean

## Sub-phase 1.9 ✅ CLI Node Daemon

- [x] `mononium-cli/src/main.rs` — clap CLI tree (node, wallet, query, logfmt) with all flags
- [x] `mononium-cli/src/node.rs` — startup lifecycle: config→DB→genesis→state→REST→block loop
- [x] REST API: `/health`, `/block/latest`, `/block/{height}`, `/balance/{address}`, `/height`, `/era`
- [x] Block production: empty blocks every 5s, stored in redb
- [x] `mononium-cli node --help` shows all 14 flags
- [x] `mononium-cli logfmt` converts JSON logs to text
- [x] CLI binary builds: `cargo build -p mononium-cli`

## Sub-phase 1.10 ✅ CLI Wallet

- [x] `mononium-cli/src/wallet.rs` — Keygen (Falcon-512), balance (HTTP), transfer (sign + submit)
- [x] `mononium-cli wallet keygen <name>` — generates Falcon-512 keys, saves to `~/.mononium/keys/{name}.json`
- [x] `mononium-cli wallet balance <address>` — queries account via REST API
- [x] `mononium-cli wallet transfer <to> <amount> --key <name>` — signs + submits tx via POST /tx
- [x] `mononium-cli query block <height>` / `latest` — queries blocks via REST API
- [x] `mononium-cli logfmt` — JSON log → human-readable
- [x] POST `/tx` endpoint on node REST API
- [x] Key file format: JSON with public_key, private_key, seed, address (hex)
- [x] 218 total tests passing (212 lib + 6 CLI wallet tests)

---

## Phase 1 Exit Criteria (on hold)

- [x] `cargo build -p mononium-lib` passes
- [x] `cargo build -p mononium-cli` passes
- [x] `cargo nextest run -p mononium-lib` passes (218 tests)
- [ ] `cargo clippy -p mononium-lib -- -D warnings` passes (~50 doc/pedantic warnings)
- [x] `mononium-cli node` starts and produces blocks on localnet (Rust e2e test)
- [x] `mononium-cli wallet keygen` generates Falcon-512 keys
- [x] `mononium-cli wallet transfer` creates signed txs (Rust e2e test)
- [x] `mononium-cli wallet balance` queries account state via REST (Rust e2e test)
- [x] **Mempool integrated into block production**: txs submitted via POST /tx → mempool → blocks
- [x] **Balance handler fixed**: reads from StateMachine (populated from storage at startup)
- [x] **Coverage**: 86.58% region / 85.62% line (lib + CLI)

---

## Coverage Improvement (Phase 1.11) ✅

**Result:** 97.37% region / 97.15% lines across `mononium-lib` (up from 86.58% / 85.62%)

| Module              | Before | After      | Status                                                        |
| ------------------- | ------ | ---------- | ------------------------------------------------------------- |
| storage/redb.rs     | 63.64% | 66.36%     | ⚠️ I/O error paths (disk failures) — impractical to unit test |
| config/mod.rs       | 88.26% | **99.70%** | ✅                                                            |
| consensus/supply.rs | 90.41% | **98.62%** | ✅                                                            |
| core/account.rs     | 93.39% | **95.45%** | ✅                                                            |
| core/state.rs       | 96.03% | **99.71%** | ✅                                                            |
| core/transaction.rs | 97.46% | **97.97%** | ✅                                                            |
| core/fee.rs         | 97.44% | **98.19%** | ✅                                                            |
| crypto/falcon.rs    | 97.81% | **98.38%** | ✅                                                            |
| crypto/trie.rs      | 98.92% | **99.09%** | ✅                                                            |
| storage/genesis.rs  | 96.83% | **99.00%** | ✅                                                            |
| mempool/mod.rs      | 99.13% | **99.18%** | ✅                                                            |

- [x] storage/redb.rs: test unknown table errors, empty table listing, large values
- [x] config/mod.rs: all accessor methods, full CLI merge, load error, validation pass for key_file
- [x] consensus/supply.rs: ceiling binds at equality, mid-range, just-below-cap, with_params
- [x] core/account.rs: extra chars in address, exact 82-char parse, scale helpers, known format, AsRef
- [x] core/state.rs: all 4 fee-only tx types (register, stake, register+stake, unstake), missing sender, cap-refill, failed-tx-still-pays-fee
- [x] core/transaction.rs: JSON roundtrips for all 6 TxBody variants
- [x] core/fee.rs: cap-refill burn, register-validator fee, burn variant equality
- [x] crypto/falcon.rs: from_private_key roundtrip, error on short private key, key size constants
- [x] crypto/trie.rs: sibling reordering, disjoint sibling root, caching after insert
- [x] storage/genesis.rs: 0x-prefix addresses, wrong-length/invalid hex, empty genesis, parse_u256 edge cases
- [x] mempool/mod.rs: select deducts from sender count, remove absent sender
- [x] Final coverage check: 97.37% region / 97.15% lines
