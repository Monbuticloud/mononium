# Mononium Phase 1 — Implementation Tracker ✅ COMPLETE

> **Goal:** `mononium-cli node` produces blocks locally. `mononium-cli wallet transfer` sends txs.
> **Approach:** TDD (Red → Green → Refactor per feature), dependency order, smaller sub-phases.
> **Commit cadence:** per test, per function, even if tests fail (RED/GREEN per commit).
> **Status:** All 12 sub-phases complete. `git tag` anchors every sub-phase.

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

## Phase 1 Exit Criteria ✅

- [x] `cargo build -p mononium-lib` passes
- [x] `cargo build -p mononium-cli` passes
- [x] `cargo nextest run -p mononium-lib` passes (327 tests)
- [x] `cargo clippy -p mononium-lib` passes (0 warnings; `-D warnings` has cosmetic lints only — intentionally deferred)
- [x] `mononium-cli node` starts and produces blocks on localnet (Rust e2e test)
- [x] `mononium-cli wallet keygen` generates Falcon-512 keys
- [x] `mononium-cli wallet transfer` creates signed txs (Rust e2e test)
- [x] `mononium-cli wallet balance` queries account state via REST (Rust e2e test)
- [x] **Mempool integrated into block production**: txs submitted via POST /tx → mempool → blocks
- [x] **Balance handler fixed**: reads from StateMachine (populated from storage at startup)
- [x] **Coverage**: 97.37% region / 97.15% lines (lib) + CLI unit tests added

---

## Coverage Improvement (Phase 1.11) ✅

**Result:** 97.37% region / 97.15% lines across `mononium-lib` (up from 86.58% / 85.62%)

| Module              | Before | After       | Status                                                         |
| ------------------- | ------ | ----------- | -------------------------------------------------------------- |
| storage/redb.rs     | 63.64% | **68.42%**  | ⚠️ I/O error paths (disk failures — 4 trivial wrappers remain) |
| config/mod.rs       | 88.26% | **99.70%**  | ✅                                                             |
| consensus/supply.rs | 90.41% | **98.62%**  | ✅                                                             |
| core/account.rs     | 93.39% | **95.45%**  | ✅                                                             |
| core/state.rs       | 96.03% | **99.71%**  | ✅                                                             |
| core/transaction.rs | 97.46% | **97.97%**  | ✅                                                             |
| core/fee.rs         | 97.44% | **98.19%**  | ✅                                                             |
| crypto/falcon.rs    | 97.81% | **98.38%**  | ✅                                                             |
| crypto/trie.rs      | 98.92% | **99.09%**  | ✅                                                             |
| storage/genesis.rs  | 96.83% | **100.00%** | ✅                                                             |
| mempool/mod.rs      | 99.13% | **99.18%**  | ✅                                                             |
| **CLI main.rs**     | 55.65% | **77.90%**  | ✅ (17 CLI parsing tests)                                      |
| **CLI wallet.rs**   | 85.11% | **86.30%**  | ✅ (KeyFile tests)                                             |
| **CLI node.rs**     | 0.00%  | **34.61%**  | ✅ (7 pure-function tests)                                     |

CLI crate remaining gaps: `run_node()`, REST handlers, block loop, `keygen()`, `balance()`, `transfer()` run in subprocess (not instrumented by llvm-cov). Full coverage requires refactoring to lib+bin split.

Total test count: **327** (lib) + 28 (CLI) + 1 (e2e)

### Coverage checklist

- [x] storage/redb.rs: test unknown table errors, empty table listing, large values, invalid path open (x2)
- [x] config/mod.rs: all accessor methods, full CLI merge, load error, validation pass for key_file
- [x] consensus/supply.rs: ceiling binds at equality, mid-range, just-below-cap, with_params
- [x] core/account.rs: extra chars in address, exact 82-char parse, scale helpers, known format, AsRef
- [x] core/state.rs: all 4 fee-only tx types (register, stake, register+stake, unstake), missing sender, cap-refill, failed-tx-still-pays-fee
- [x] core/transaction.rs: JSON roundtrips for all 6 TxBody variants
- [x] core/fee.rs: cap-refill burn, register-validator fee, burn variant equality
- [x] crypto/falcon.rs: from_private_key roundtrip, error on short private key, key size constants
- [x] crypto/trie.rs: sibling reordering, disjoint sibling root, caching after insert
- [x] storage/genesis.rs: 0x-prefix addresses, wrong-length/invalid hex, empty genesis, parse_u256 edge cases, directory-path error
- [x] mempool/mod.rs: select deducts from sender count, remove absent sender
- [x] Final coverage check: 97.37% region / 97.15% lines

---

## Phase 1 Wrap-up 🎉

All 12 sub-phases (1.0 through 1.11) are complete. Every sub-phase is tagged in git with an annotated tag.

| Tag           | Description                                                            |
| ------------- | ---------------------------------------------------------------------- |
| `phase1-1.0`  | Foundation types + workspace setup (18 tests)                          |
| `phase1-1.1`  | Cryptography: Falcon-512, BLAKE3, SignatureScheme trait (62 tests)     |
| `phase1-1.2`  | Sparse Merkle Tree implementation (79 tests)                           |
| `phase1-1.3`  | Transaction & block types + fee policy (105 tests)                     |
| `phase1-1.4`  | State machine: Transfer/Burn, nonce validation (117 tests)             |
| `phase1-1.5`  | Storage: StorageEngine trait, RedbEngine, genesis loader (134 tests)   |
| `phase1-1.6`  | Mempool: insert/remove/select/TTL/priority ordering (159 tests)        |
| `phase1-1.7`  | Consensus basics: Top-N election, round-robin, era, supply (192 tests) |
| `phase1-1.8`  | Config + genesis JSON files: NodeConfig, YAML/TOML (212 tests)         |
| `phase1-1.9`  | CLI node daemon: REST API, block production loop                       |
| `phase1-1.10` | CLI wallet: keygen, balance, transfer                                  |
| `phase1-1.11` | Coverage: 97.37% region / 97.15% lines (327 lib tests)                 |

**Phase 1 is now closed. Development continues with Phase 2 (Localnet).**

---

# Mononium Phase 2 — Localnet (Multi-Validator)

> **Goal:** 3+ validators produce blocks with consensus on a local machine. `mononium-cli wallet stake` commands work. P2P networking, staking txs, governance, slashing, and sync protocol operational.
> **Approach:** TDD per sub-phase. All state machine changes tested deterministically first. Networking tested via in-process harness before real libp2p.
> **Dependency order:** Staking txs → P2P core → block propagation → consensus engine → slashing → governance → RPC → multi-validator CLI → crash recovery → benchmarks.

---

## Sub-phase 2.0 — Staking Transaction Types

**From:** Protocol.md (TxBody variants), Validators.md (lifecycle), Architecture.md (consensus/ module)

Add the 4 staking TxBody variants and wire them through the state machine. Validator lifecycle states (Registered → Staked → Active → Unstaking → Inactive) + era boundary integration.

### Transaction types
- [ ] `TxBody::RegisterValidator { public_key: [u8; 897], proof_of_intent: [u8; 666] }` — one-time declaration tx with Falcon-512 proof of key ownership
- [ ] `TxBody::Stake { validator: [u8; 32], amount: U256 }` — lock MONEX, enters candidate pool
- [ ] `TxBody::RegisterAndStake { validator: [u8; 32], amount: U256 }` — convenience: atomically register + stake for new validators
- [ ] `TxBody::Unstake { validator: [u8; 32], amount: U256 }` — begin withdrawal from validator set
- [ ] All 4 variants: SCALE + JSON serialization, symmetric roundtrip tests
- [ ] All 4 variants: included in block sizes table (Protocol.md "Block Hard Cap": 500 KB / 500 txs)

### State machine
- [ ] `StateMachine::register_validator()` — create validator entry with status=Staked, store public key, nonce increment, fee deduction
- [ ] `StateMachine::stake()` — increase validator's stake, nonce increment, sender balance deduction
- [ ] `StateMachine::register_and_stake()` — atomic register + stake, single fee + nonce
- [ ] `StateMachine::unstake()` — set validator status to Unstaking, record cooldown start era
- [ ] Validator state enum: `{ Staked, Active, Unstaking { release_era: u64 }, Frozen { frozen_until: u64 }, Thawed }` stored in state tree namespace `0x01`
- [ ] `ValidatorEntry` struct: `{ address, public_key, stake: U256, status, registration_era, frozen_until: u64 }`
- [ ] Non-active validator tracking: candidate pool in SMT namespace `0x01`
- [ ] **Era 0 Open election**: any registered validator becomes Active (up to `max_validators`), no minimum stake
- [ ] **Era 1+ Top-N**: only validators with ≥ 1 MONEX stake enter candidate pool; Top-N by stake sorted into active set
- [ ] Unstaking cooldown: 168 eras from unstake tx → balance returned to sender's transferable balance
- [ ] Failed staking txs: insufficient balance, invalid validator address, double-register, unstaking nonexistent validator → fee-only deduction (same as Transfer/Burn)
- [ ] **Tests**: register, stake, register+stake, unstake happy paths; insufficient balance; double-register; unknown validator; unstaking from unregistered; era 0 vs era 1+ behavior; cooldown expiry
- [ ] Coverage: ≥ 95% region on `core/state.rs` staking paths, ≥ 100% on `core/transaction.rs` new variants

---

## Sub-phase 2.1 — P2P Networking (Core)

**From:** Network.md (P2P layer, topics, discovery), Architecture.md (`network/` module tree)

Build the `network/` module with libp2p gossipsub, kademlia, mDNS, and the 4 topics. Peer discovery via bootstrap + mDNS + kademlia.

### Module structure
- [ ] `network/mod.rs` — `P2pService` struct (start/stop), `P2pConfig` (ports, bootnodes, key path)
- [ ] `network/constants.rs` — topic name templates (`mononium/{type}/{chain_id}`), default port 30333, protocol version
- [ ] `network/topics.rs` — 4 gossipsub topics: `mononium/txs/{chain_id}`, `mononium/blocks/{chain_id}`, `mononium/votes/{chain_id}`, `mononium/evidence/{chain_id}`
- [ ] `network/discovery.rs` — bootstrap peer connection, kademlia DHT, mDNS local discovery, Identify protocol
- [ ] `network/messages.rs` — wire message enums per topic: `BlockMessage`, `TxMessage`, `VoteMessage`, `EvidenceMessage` (all SCALE-encoded)
- [ ] All message types: SCALE encode/decode symmetric roundtrip tests

### Gossipsub
- [ ] libp2p swarm construction with gossipsub, kademlia, identify, mDNS behaviours
- [ ] Topic subscription on startup (all 4 topics, scoped by chain_id from genesis config)
- [ ] Topic-level size limits (per Network.md table): txs=1MB, blocks=500KB, votes=1KB, evidence=5KB
- [ ] Topic-level rate limits per peer (per Network.md table): txs=20msg/s, blocks=1msg/s, votes=100msg/s, evidence=5msg/s
- [ ] Size/rate validation before deserialization — oversized/over-rate messages dropped, peer score decremented
- [ ] Message publishing: `P2pService::publish_tx()`, `publish_block()`, `publish_vote()`, `publish_evidence()`

### Discovery
- [ ] Bootstrap peer connection via multiaddrs from config (`network.bootnodes`)
- [ ] Kademlia: random walk for peer discovery, provider records for chain_id
- [ ] mDNS: local network peer discovery (localnet only, no subnet effect)
- [ ] Identify protocol: exchange agent version, protocol version, listen addrs
- [ ] Peer metadata: `shards: Vec<u16>` field for future shard-aware routing

### Peer scoring (per Network.md peer scoring table)
- [ ] Per-peer score: starts at 0, range -100 to 100
- [ ] Score adjustments: valid block/vote +1, valid sync batch +2, batch hash mismatch -10, invalid state -20, timeout -4, invalid gossip -10, flapping -10, duplicate gossip -2
- [ ] Score tiers: >0 Good (preferred for sync), -20–0 Neutral (connected, deprioritized), < -20 Banned (disconnected)
- [ ] Ban mechanics: 720-block ban duration, score decay via positive behavior
- [ ] Peer scoring tests: adjustments, ban threshold, expiry, deprioritization

### Transport
- [ ] TCP transport with Noise handshake (libp2p built-in) + yamux multiplexing
- [ ] Snappy compression at transport layer (per Network.md)
- [ ] `P2pConfig` from config file: `p2p_port`, `bootnodes`, optional `p2p_key_path`
- [ ] CLI flags: `--p2p-port`, `--bootnodes` (repeatable)
- [ ] Multiaddr parsing and validation in config

### Tests
- [ ] Unit: message encode/decode symmetry, topic validation, peer scoring math, ban duration calc
- [ ] Integration: two in-process libp2p hosts connect, subscribe to topics, publish/receive messages (loopback)
- [ ] Integration: mDNS discovers peer on local network (loopback interface)
- [ ] Integration: kademlia bootstrapping with known peer

---

## Sub-phase 2.2 — Block Propagation + Sync Protocol

**From:** Network.md (sync protocol, BlockSyncRequest/Response, SyncCursor), ADR-018

Blocks are gossiped on `mononium/blocks/{chain_id}`. Sync protocol via libp2p Request-Response for catch-up. SyncCursor persists position across restarts.

### Block gossip
- [ ] On block production: `P2pService::publish_block(block)` gossips to `mononium/blocks/{chain_id}`
- [ ] On block receive: validate block size ≤ 500KB, verify proposer signature (Falcon-512), verify parent_hash exists
- [ ] Block validation handler: validate → queue for state machine application → gossip to peers (if valid)
- [ ] Duplicate block rejection (by block_hash), peer score -2 for repeated duplicates (>3 identical)
- [ ] Integration test: proposer publishes block via gossipsub, other validators receive and validate

### Sync protocol messages
- [ ] `BlockSyncRequest { start_height, max_blocks: u16 (max 500), direction: SyncDirection, known_block_hash: Option<[u8;32]> }`
- [ ] `BlockSyncResponse { blocks: Vec<Block>, highest_height: u64, batch_hash: [u8;32] }` (per ADR-018: rolling BLAKE3 over batch)
- [ ] `BlockByHashRequest { block_hashes: Vec<[u8;32]> }` (max 100)
- [ ] `BlockByHashResponse { blocks: Vec<Block> }` (in request order, missing entries omitted)
- [ ] `SyncDirection::Forward` — normal catch-up, `SyncDirection::Backward` — recent blocks from tip
- [ ] All message types registered as libp2p Request-Response protocol

### SyncCursor
- [ ] `SyncCursor { last_verified_height, last_verified_hash, target_height, pending_range: Option<HeightRange> }`
- [ ] Persisted as `{data_dir}/{chain_id}/sync_cursor.json` (small JSON file, not redb)
- [ ] Created at genesis (height 0, genesis block hash), updated after each verified batch
- [ ] On restart: load cursor from disk, resume from `last_verified_height + 1`
- [ ] If cursor missing or corrupted: fall back to full replay from genesis
- [ ] Reset on checkpoint load (Phase 2.9): set to checkpoint height + verified state root hash

### Sync flow (per Network.md sync protocol)
- [ ] **Sync init**: connect to bootstrap peers → learn network tip (`Backward` request, max=1) → determine gap
- [ ] **Block catch-up loop**: while `last_verified_height < target_height`: select peer (round-robin), request batch (100 blocks, `Forward`), verify batch_hash, verify parent_hash chain, apply each block, advance cursor, persist
- [ ] `known_block_hash` anchor: requesting peer sends last verified hash, responding peer MUST serve blocks building on that hash or return empty (fork protection)
- [ ] Batch_hash verification per ADR-018: compute rolling BLAKE3 from genesis hash over batch, compare to response.batch_hash
- [ ] Per-block verification in batch: signature, timestamp ±2s, parent_hash link, re-execute txs → verify global_state_root
- [ ] Batch rejection: any block fails → entire batch discarded, try next peer
- [ ] Timeout: 5s/10s/15s per attempt, after 3 peers same height → 10s pause before retry
- [ ] After 10 consecutive batch failures across all peers → log critical, retry with exponential backoff (10s→30s→60s→120s→300s cap)
- [ ] **No-peer stall**: retry with backoff (5s→10s→30s→30s repeated), log warning, never exit or panic
- [ ] **Sync complete conditions**: `last_verified_height >= target_height` AND last block timestamp within `2 × block_time` of local clock AND ≥ 1 peer connected AND no pending verification

### Disconnection handling
- [ ] Mid-batch disconnect: discard incomplete batch (unverified), request same range from next peer with same `known_block_hash`
- [ ] Stateless recovery: no session state, no partial batch tracking
- [ ] Invariant: no block marked verified until entire batch passes parent_hash + per-block verification

### Fork detection
- [ ] Batch hash mismatch during sync → fork or corrupt response → disconnect peer, score -= 10, try next peer
- [ ] Peer disagreement resolution (per Network.md): if peer A and peer B return different batch_hashes → request from peer C (tiebreaker, majority wins)
- [ ] Minority peer scored down (score -= 5)

### Tests
- [ ] Unit: SyncCursor persistence roundtrip, batch_hash computation matches ADR-018
- [ ] Integration: two-node sync (one node ahead → lagging node catches up)
- [ ] Integration: disconnection mid-batch → resume from next peer without gaps
- [ ] Integration: peer serves wrong fork → detection + ban
- [ ] Integration: empty response handling, timeout handling

---

## Sub-phase 2.3 — PoS Consensus Engine

**From:** Consensus.md (BFT commit, slot model, fork choice, missed slots), Architecture.md (finality.rs, consensus engine)

Wire the proposer schedule into real block production with multi-validator rounds. BFT commit votes, verification window, fork-choice rule, missed slot penalties.

### BFT commit tracking
- [ ] `consensus/finality.rs` — `CommitTracker`: collects `CommitVote`s per height, tracks cumulative stake weight of committing validators
- [ ] Active validator stake weights loaded from state at era boundaries
- [ ] Finality threshold: `total_commit_weight > (2/3 × total_active_stake)`, strict > (not ≥)
- [ ] `CommitVote` gossiped on `mononium/votes/{chain_id}` immediately after verification
- [ ] Vote collection: next proposer collects gossiped votes during verification window
- [ ] Block header includes `collected_commits: Vec<CommitVote>` (up to active set size)
- [ ] Block is final when > 2/3 commits for it appear in any later block's header
- [ ] Verification window: ~20s (4 blocks) for validators to verify + publish votes

### Proposer schedule integration
- [ ] `ConsensusEngine` struct: owns election, proposer schedule, commit tracker, slot timer
- [ ] Proposer schedule computed at era boundaries from active validator set
- [ ] `ConsensusEngine::current_proposer()` — returns the proposer for the current slot (round-robin over active set)
- [ ] `ConsensusEngine::am_i_proposer()` — checks if local validator is the scheduled proposer
- [ ] On proposer slot: collect txs from mempool (up to 500 txs / 500 KB), build block, sign (Falcon-512), gossip on `mononium/blocks/{chain_id}`
- [ ] On non-proposer slot: wait for block from proposer (5s timeout) → if received: verify + vote → if timeout: slot goes empty
- [ ] Slot timer: 5s intervals, aligned to wall clock

### Block verification (non-proposer validators)
- [ ] Receive block via gossipsub → verify Falcon-512 proposer signature
- [ ] Verify `block.header.proposer == ConsensusEngine::current_proposer()`
- [ ] Verify `block.header.timestamp` ± 2s of local clock
- [ ] Verify `block.header.parent_hash` matches local canonical tip
- [ ] Verify block size ≤ 500 KB (SCALE bytes)
- [ ] Re-execute all txs → compute SMT root → assert matches `global_state_root`
- [ ] If valid: sign `CommitVote { height, block_hash, validator, signature }`, gossip on `mononium/votes/{chain_id}`
- [ ] If invalid: reject block, do NOT vote, log warning

### Fork-choice rule (per Consensus.md)
- [ ] Canonical chain = heaviest chain by total stake backing
- [ ] `total_stake_weight = sum(stake of all validators who committed to each block in chain)`
- [ ] Validators compute fork choice independently from their view of committed votes
- [ ] No separate finality gadget — fork choice is always available as fallback
- [ ] After equivocation fork: honest chain becomes heavier immediately (equivocators lose 90% stake)
- [ ] Temporary partition: scheduled proposer per slot breaks symmetry — one side gets more scheduled proposers

### Missed slot handling
- [ ] If no block received from scheduled proposer within 5s → slot goes empty, no height change
- [ ] Height increments only when a block is actually proposed (Option B per Consensus.md)
- [ ] Missed slot penalty: 0.08 MONEX deducted from proposer's stake at era boundary
- [ ] Penalty sent to Cap-Refill address (`0x00..01`) — expands effective max supply
- [ ] Penalty applies as average across era: validator who missed all 720 slots loses full stake
- [ ] No mid-era slashing for missed slots — reconciled at era boundary

### Tests
- [ ] Unit: CommitTracker adds votes, computes stake weight, reaches/exceeds 2/3 threshold
- [ ] Unit: `current_proposer()` returns correct validator per slot, cycles through active set
- [ ] Unit: verification window — votes arriving after 4 blocks are not included
- [ ] Unit: missed slot — no block → no height increment, penalty computed correctly
- [ ] Integration: single proposer produces blocks, other validators receive + verify + vote
- [ ] Integration: 3-validator cluster produces 10 blocks with BFT commits, all validators agree on canonical chain at each height
- [ ] Integration: proposer offline for 1 slot → slot goes empty, next proposer builds on last canonical block, no stall
- [ ] Integration: clock drift rejection (block with timestamp > 2s from local clock is rejected)

---

## Sub-phase 2.4 — Slashing

**From:** Slashing.md (evidence format, 90% slash, freeze, thaw), Consensus.md (equivocation fork resolution)

Equivocation detection, evidence verification, slashing execution, freeze management.

### Evidence type
- [ ] `EquivocationEvidence { header_a, signature_a, header_b, signature_b, proposer }` (per Slashing.md)
- [ ] Evidence verification: same height, same parent_hash, distinct blocks, both Falcon-512 signatures verify against proposer's public key
- [ ] Evidence gossiped on `mononium/evidence/{chain_id}` topic
- [ ] Any account can submit evidence as a transaction to the state machine

### State machine: slash execution
- [ ] `StateMachine::apply_slash(evidence)` — verify evidence, deduct 90% of validator's stake
- [ ] 90% slashed → 90% of slashed amount to Burn address (`0x00..00`), 10% to reporter's locked balance
- [ ] Remaining 10% of original stake stays staked (validator retains it, not burned)
- [ ] Validator status → Frozen, `frozen_until = current_era + 72`
- [ ] Reporter bounty: 10% of slashed amount credited as locked balance (not transferable)
- [ ] Reporter types: active validator (bounty → existing stake), inactive staker (bounty → existing stake), non-validator (bounty → locked MONEX balance)

### Freeze management
- [ ] `FreezeRecord { validator_id, frozen_at_era, remaining_eras: u16 }` (starts at 72)
- [ ] At era boundary: decrement `remaining_eras` for all frozen validators
- [ ] When `remaining_eras == 0`: validator status → Thawed, re-enters candidate pool (if still staked)
- [ ] Frozen validator: excluded from proposer schedule, cannot vote, excluded from fee distribution
- [ ] Mid-era freeze: excluded from remaining blocks in era, already-computed proposer slots go empty (no reassignment), no additional missed-slot penalty
- [ ] Thawed validator with remaining stake → candidate pool (re-electable at next era boundary)
- [ ] Thawed but fully unstaked validator → Inactive (must re-register and stake)
- [ ] Double-slashing: can be slashed again after thawed for a new equivocation (fresh 72-era freeze)
- [ ] Already-frozen validator: secondary evidence against them is ignored

### Tests
- [ ] Unit: evidence verification — valid pair accepted, mismatched height rejected, same blocks rejected, bad signature rejected
- [ ] Unit: 90% slash + 10% bounty math with various stake sizes
- [ ] Unit: freeze countdown — 72 eras decremented, thaw at 0, frozen validator excluded from election
- [ ] Unit: mid-era freeze — excluded from fee distribution for remaining blocks, proposer slots go empty
- [ ] Integration: equivocation evidence submitted on-chain → validator frozen → missed slots → era boundary → thawed
- [ ] Integration: reporter bounty locked (not immediately withdrawable)

---

## Sub-phase 2.5 — Governance Module

**From:** Governance.md (full lifecycle, proposal/vote types, era-boundary hook), Architecture.md (`governance/` module tree)

On-chain stake-weighted governance with proposal submission, 7-era voting window, era-boundary tally, and automatic execution.

### Module structure
- [ ] `governance/mod.rs` — `GovernanceEngine`: propose, vote, cancel, tally, execute
- [ ] `governance/types.rs` — `Proposal`, `Vote`, `GovernanceAction`, `GovernanceParam` (all SCALE + JSON)
- [ ] `governance/constants.rs` — deposit=100 MONEX, voting window=7 eras, max_active_per_proposer=5, max_proposals_per_era=50, quorum=2/3, threshold=simple majority
- [ ] Quorum: `≥ 2/3 of total active stake` (not participating stake), strict ≥
- [ ] Threshold: `> 50% of participating stake approves`

### Proposal lifecycle
- [ ] **Submit**: `TxBody::Propose { proposal_id, title, description, actions, deposit }` — proposer must have ≥ 100 MONEX staked, deposit deducted, rate limits enforced
- [ ] `GovernanceAction::UpdateParam { param: GovernanceParam, new_value: U256 }` — 10 mutable params with bounds
- [ ] `GovernanceAction::IncreaseShards { new_count: u16, effective_era: u64 }` — increase only, grace period ≥ 24 eras
- [ ] `GovernanceParam` enum: MaxValidators, EraLength, BlockSizeCapBytes, BlockTxCap, FlatFee, PerByteRate, AntiSpamDeposit, MissedSlotPenalty, SupplyCeilingRate, SupplyHeadroomRate
- [ ] Parameter bounds enforced at validation (per Governance.md bounds table): e.g., `max_validators ∈ [1, 1000]`, `era_length ∈ [100, 10000]`, `flat_fee ∈ [0, 100 MONEX]`
- [ ] **Vote**: `TxBody::Vote { proposal_id, approve: bool }` — weight snapshotted at vote block, one per voter, second vote overwrites first
- [ ] Voter must have > 0 staked MONEX, proposal must be in voting window (`submission_era ≤ current_era < submission_era + 7`)
- [ ] **Cancel**: `TxBody::CancelProposal { proposal_id }` — only proposer, only before any votes cast, deposit returned
- [ ] Parameter lock: active proposal for param P prevents new proposals targeting P (releases on resolution)
- [ ] Param lock is per-parameter — two proposals for different params can coexist

### Era-boundary hook
- [ ] At era boundary (after validator set recalculation, before proposer schedule reset):
- [ ] For each open proposal where `submission_era + 7 == current_era`: tally votes
- [ ] Quorum met + majority approve → mark Approved, enqueue execution at next era boundary
- [ ] Quorum met + majority reject → mark Rejected, deposit returned
- [ ] Quorum not met → mark Expired, deposit forfeited to Cap-Refill (`0x00..01`)
- [ ] Execution: sort approved proposals by `proposal_id` hash (lexicographic ascending), apply each action in order
- [ ] `UpdateParam`: write new value to governance param state (last-write-wins for duplicates)
- [ ] `IncreaseShards`: trigger shard migration (deferred to Phase 3+)
- [ ] Approved proposals execute at next era boundary after tallying (never mid-era)

### State storage
- [ ] Governance state in SMT namespace `0x03` (governance):
  - [ ] `prop_{proposal_id}` → `Proposal` record
  - [ ] `vote_{proposal_id}_{voter}` → `Vote` record
  - [ ] `gov_param_{param_name}` → `U256` current value
  - [ ] `gov_active_count` → `u64` rate limit counter

### Tests
- [ ] Unit: propose validation — deposit, rate limits, param bounds, duplicate proposal_id, title/desc size
- [ ] Unit: vote validation — voter stake, voting window, overwrite, proposal not found
- [ ] Unit: cancel — only proposer, only before first vote
- [ ] Unit: tally — quorum met/passed, quorum met/rejected, quorum not met/expired
- [ ] Unit: execution — param update, sorted proposal execution, last-write-wins
- [ ] Unit: param bounds — proposal with out-of-bounds param rejected at validation
- [ ] Integration: full governance flow: propose → vote → era boundary tally → execute param change
- [ ] Integration: proposal expires without quorum → deposit forfeited to Cap-Refill
- [ ] Integration: parameter lock — second proposal for same param blocked until first resolves

---

## Sub-phase 2.6 — RPC (jsonrpsee + REST expansion)

**From:** Architecture.md (RPC Interface section, jsonrpsee methods, subscriptions)

Add jsonrpsee WebSocket server alongside existing REST. 16 JSON-RPC methods + 3 subscriptions. Expand REST with missing endpoints.

### jsonrpsee server
- [ ] `rpc/jsonrpc.rs` — jsonrpsee WebSocket server on `--rpc-port` (default 9944)
- [ ] Server co-exists with axum REST on different ports (both started in node lifecycle step 15)
- [ ] Config: `network.rpc_port` / `--rpc-port` flag

### JSON-RPC methods (per Architecture.md table)
- [ ] `tx_submit(Transaction)` → `TxHash` — submit signed transaction (SCALE-hex encoded)
- [ ] `tx_status(TxHash)` → `{ status, height?, index? }` — status: Pending / Finalized / Failed
- [ ] `block_get(BlockId)` → `Block` — full block with body (BlockId = height or hash)
- [ ] `block_header(BlockId)` → `BlockHeader` — header only
- [ ] `block_latest()` → `BlockHeader` — latest header
- [ ] `state_get_balance(Address)` → `U256` — balance in MOXX
- [ ] `state_get_nonce(Address)` → `u64` — next valid nonce
- [ ] `validator_set()` → `Vec<ValidatorInfo>` — active + candidate set
- [ ] `validator_stake(Address)` → `U256` — stake of specific validator
- [ ] `era_current()` → `u64` — current era index
- [ ] `chain_get_height()` → `u64` — current block height
- [ ] `chain_get_genesis()` → `Hash` — genesis block hash

### Subscriptions
- [ ] `subscribe_blocks` → `Event<BlockHeader>` — new block header on each slot
- [ ] `subscribe_finality` → `Event<FinalityEvent>` — finality notifications (>2/3 commits)
- [ ] `subscribe_votes` → `Event<CommitVote>` — vote notifications

### Error codes (per Architecture.md error code table)
- [ ] 0=Success, -1=Internal, -2=Invalid params, -3=Tx validation, -4=Block not found, -5=Tx not found, -6=Address not found, -7=Rate limited

### REST expansion
- [ ] GET `/nonce/{address}` — returns sender's current nonce (Phase 1 missing from REST)
- [ ] GET `/validators` — returns `Vec<ValidatorInfo>`
- [ ] GET `/validator/{address}` — returns `ValidatorInfo`
- [ ] GET `/genesis` — returns genesis block hash
- [ ] GET `/block/{hash}` — block lookup by hash (currently height-only)

### Integration
- [ ] jsonrpsee + axum run on separate ports, both behind graceful shutdown (SIGINT/SIGTERM)
- [ ] Both servers use same `Arc<AppState>` for state machine + storage access
- [ ] Error responses: both JSON-RPC error object and REST `{ error: { code, message } }` consistent

### Tests
- [ ] Unit: JSON-RPC method parameter parsing, response serialization
- [ ] Integration: submit tx via jsonrpsee → query status → verify in block
- [ ] Integration: REST + JSON-RPC both serving simultaneously
- [ ] Integration: subscription — connect via WebSocket, receive block events

---

## Sub-phase 2.7 — Multi-Validator Node Mode

**From:** Architecture.md (node startup lifecycle steps 1-16), Network.md (config bootnodes), Consensus.md (bootstrap phase, era 0 election)

Wire the full node startup lifecycle with P2P networking, consensus participation, and bootstrap phase. Observer mode for non-validators. Docker compose for local multi-validator.

### Node startup lifecycle (full, per Architecture.md)
- [ ] Step 1-6: CLI args → config load → merge → key load → passphrase prompt → key derivation (Phase 1)
- [ ] Step 7: **[y/N] startup preview** (per Architecture.md preview block: network, validator address, P2P port, boot peers, data dir)
- [ ] Step 8-10: Open/init redb → load genesis → load state from DB (Phase 1)
- [ ] Step 11: **Start libp2p host** — bind P2P port, subscribe to 4 gossipsub topics (Phase 2.1)
- [ ] Step 12: **Connect to bootstrap peers** — kademlia discovery on same chain_id (Phase 2.1)
- [ ] Step 13: **Initialize consensus engine** — load proposer schedule for current era (Phase 2.3)
- [ ] Step 14: **Start slot timer** — begin listening for block production/voting slots (Phase 2.3)
- [ ] Step 15: **Start RPC servers** — axum REST + jsonrpsee WebSocket (Phase 2.6)
- [ ] Step 16: **Signal handlers** — SIGINT/SIGTERM → graceful shutdown (flush DB, disconnect peers)

### Bootstrap phase (per Consensus.md)
- [ ] Genesis config: `bootstrap { public_keys: [...], blocks: N }` field read from genesis JSON
- [ ] Bootstrap proposer selection: round-robin over bootstrap key list for blocks 1..N
- [ ] Non-bootstrap validators cannot propose during bootstrap phase (blocks rejected)
- [ ] Bootstrap proposers include RegisterValidator/Stake txs from other validators
- [ ] At block N+1 (bootstrap phase ends): run election over all registered validators → commit as era 0 active set (up to `max_validators`)
- [ ] Bootstrap key has no special status after phase ends — regular validator if registered
- [ ] Era 0 Open election: all registered validators automatically active, no minimum stake
- [ ] Era 1+: standard Top-N by stake with minimum 1 MONEX

### Observer mode
- [ ] `observer: true` config flag → node syncs state but does NOT participate in consensus
- [ ] No key file loaded, no signing, no block proposals, no votes
- [ ] Sync mode only — follows canonical chain, serves RPC queries
- [ ] Observer node validates blocks like a full validator but without submitting votes

### Config expansion
- [ ] `bootnodes` field populated in node configs (Phase 1 configs have empty `bootnodes` for single-node)
- [ ] `configs/node.localnet.yaml` — localnet with mDNS, no bootnodes
- [ ] `configs/node.devnet.yaml` — devnet with bootnodes pointing to bootstrap peers
- [ ] CLI flag `--observer` — override to observer mode
- [ ] Validator key prompt: step 5 passphrase with timeout (configurable via `unlock_timeout`)

### Docker compose
- [ ] `docker/docker-compose.yml` — multi-service: bootstrap node + N validators + RPC node
- [ ] Each container: own data dir, own key, unique P2P/REST/RPC ports
- [ ] `Dockerfile` — multi-stage cargo build, minimal runtime image
- [ ] Docker compose scales: `--scale validator=5` for N-validator testing
- [ ] Network: shared docker network, mDNS disabled (bootnodes configured explicitly)
- [ ] Docs: `user_docs/Docker.md` or inline in compose file

### Tests
- [ ] Integration: 3-validator in-process cluster (real libp2p loopback) produces 20 blocks with consensus
- [ ] Integration: bootstrap phase — only bootstrap keys can propose blocks 1..N
- [ ] Integration: era 0 → era 1+ transition at era boundary
- [ ] Integration: observer node syncs from genesis without signing
- [ ] Integration: validator startup with passphrase prompt (simulated stdin)

---

## Sub-phase 2.8 — CLI Stake/Unstake Commands + Validator Queries

**From:** Architecture.md (CLI tree: `wallet stake`), Validators.md (staking flow), Protocol.md (stake tx types)

Add CLI commands for staking workflow and validator status queries.

### CLI commands
- [ ] `mononium-cli wallet register <public_key>` — creates + signs + submits RegisterValidator tx
- [ ] `mononium-cli wallet stake <validator_address> <amount> --key <name>` — creates + signs + submits Stake tx
- [ ] `mononium-cli wallet register-and-stake <validator_address> <amount> --key <name>` — convenience: atomic register + stake
- [ ] `mononium-cli wallet unstake <validator_address> <amount> --key <name>` — creates + signs + submits Unstake tx
- [ ] `mononium-cli query validator <address>` — queries validator info (stake, status, era registered)
- [ ] `mononium-cli query validators` — lists all validators (active + candidate)
- [ ] `mononium-cli query nonce <address>` — queries current nonce

### REST integration
- [ ] All staking txs submitted via POST `/tx` (existing endpoint)
- [ ] GET `/validator/{address}` — returns `ValidatorInfo { address, public_key, stake, status, registration_era }`
- [ ] GET `/validators` — returns `Vec<ValidatorInfo>` for all registered validators

### Output format
- [ ] Tx submission: prints `TxHash:` and human-readable confirmation
- [ ] Validator query: table format with address, stake (MONEX), status, era registered
- [ ] Balance query: updated to show both transferable balance + locked/staked amounts (if applicable)

### Tests
- [ ] CLI unit: parse staking command args, build correct tx bodies, error on invalid args
- [ ] CLI e2e: register → stake → query validator status (via existing e2e test infra)
- [ ] CLI e2e: unstake → verify cooldown state

---

## Sub-phase 2.9 — Crash Recovery + State Checkpoints

**From:** Architecture.md (Crash Recovery), Storage.md (Checkpoints section)

Automatic crash recovery on restart. State checkpoints at era boundaries for fast sync.

### Crash recovery
- [ ] On restart: open redb, read current height from META table
- [ ] Load latest canonical block from blocks table
- [ ] Recompute SMT root from stored state: insert all accounts from ACCOUNTS table into fresh SMT → compare root with block header's `global_state_root`
- [ ] If SMT root matches → resume from `current_height + 1`
- [ ] If SMT root mismatches → panic (redb write txn ACID guarantee — should never happen)
- [ ] After recovery: discover missed blocks via sync protocol (Phase 2.2)

### State checkpoints (per Storage.md)
- [ ] At era boundary: spawn background task to write checkpoint
- [ ] `checkpoint_meta { era, height, global_state_root, timestamp, num_shards }` → stored in `checkpoint_meta` table (keyed by era)
- [ ] `checkpoint_data { (era, shard_id) → SMT key-value pairs }` → stored in `checkpoint_data` table
- [ ] Checkpoint production runs as background task — does NOT block next block production
- [ ] If previous checkpoint write still in progress at next era boundary → cancel previous, start new (new subsumes old)
- [ ] Generation counter in checkpoint_meta for atomic reads (readers see last completed checkpoint)

### Retention policy (per Storage.md)
- [ ] **Full mode** (default): keep latest 2 checkpoints — `checkpoint_era_N` overwrites `checkpoint_era_N-2`
- [ ] **Compact mode**: skip checkpoint production entirely (save write IO + disk)
- [ ] **Archive mode**: retain all checkpoints (opt-in, for archive nodes)
- [ ] Config: `storage.mode` = full | compact | archive, `storage.compact_eras` (default 2)
- [ ] Config already exists from Phase 1.8 — wire into checkpoint behavior

### Checkpoint serving (for sync, Phase 2.2 integration)
- [ ] `CheckpointRequest { target_height }` / `CheckpointResponse { height, smt_nodes, validator_set, validator_set_hash, checkpoint_block_header, checkpoint_hash }`
- [ ] All fields SCALE-encoded per Network.md spec
- [ ] Serves checkpoint at nearest era boundary ≤ target_height
- [ ] Validator set included in response (syncing node doesn't have historical state)
- [ ] Checkpoint trust model: verify BFT commit votes (>2/3) against included validator set, rebuild SMT, compare global_state_root
- [ ] Checkpoint served via libp2p Request-Response protocol

### Tests
- [ ] Unit: checkpoint write/read roundtrip, retention policy (full keeps 2, compact skips, archive keeps all)
- [ ] Integration: crash recovery — simulate crash mid-block apply, restart, verify state consistent
- [ ] Integration: checkpoint produced at era boundary, readable via sync protocol

---

## Sub-phase 2.10 — Benchmarks + In-Process Test Harness

**From:** Testing.md (test tiers 2-3, benchmark suite, target metrics), Roadmap.md (100 tx/s target)

In-process multi-validator test harness. criterion benchmarks for crypto, state, and e2e throughput. 100 tx/s target measurement.

### In-process test harness
- [ ] `tests/harness.rs` — `ClusterBuilder` + `Cluster` for in-process multi-validator tests
- [ ] `ClusterBuilder::with_validator(name, key)` — add a validator
- [ ] `ClusterBuilder::with_genesis(path)` — set genesis config
- [ ] `Cluster::start()` — spawn all validators with in-memory redb, loopback libp2p
- [ ] `Cluster::run_until(blocks: u64)` — let cluster produce N blocks
- [ ] `Cluster::stop()` — graceful shutdown
- [ ] `Cluster::state(name)` — access a validator's state for assertions
- [ ] Validators communicate via real libp2p gossipsub over loopback TCP (not mocked)

### Integration tests (per Testing.md integration test list)
- [ ] `tests/integration/basic_transfer.rs` — full flow: keygen → tx → block → state
- [ ] `tests/integration/multi_validator.rs` — 3 validators produce 20 blocks, all agree on canonical chain
- [ ] `tests/integration/era_transition.rs` — era boundary: validator set change, proposer schedule reset
- [ ] `tests/integration/slashing_scenarios.rs` — equivocation → evidence → slash → freeze → era boundary → thaw
- [ ] `tests/integration/governance_flow.rs` — propose → vote → tally at era boundary → execute param change

### Benchmarks (per Testing.md benchmark suite + target metrics)
- [ ] `benches/crypto.rs`:
  - [ ] Falcon sign: target < 10ms
  - [ ] Falcon verify: target < 5ms
  - [ ] Falcon batch verify (10): target < 20ms
  - [ ] SMT insert 1000 accounts: target < 50ms
  - [ ] SMT root after 1000 inserts: target < 10ms
- [ ] `benches/state.rs`:
  - [ ] Block apply 100 txs (all Falcon verify): target < 200ms
  - [ ] Block apply 500 txs: target < 1s
  - [ ] Mempool insert 10000: target < 50ms
- [ ] `benches/e2e.rs`:
  - [ ] E2E 3-validator cluster, 100 blocks: target > 50 tx/s
  - [ ] E2E 3-validator cluster, 500 blocks: target > 100 tx/s (Phase 2 stretch goal)

### Benchmark infrastructure
- [ ] `cargo bench -p mononium-lib` runs all benchmark suites
- [ ] Baseline comparison: `--baseline main` for regression detection
- [ ] Benchmark results tracked in repo (baseline files committed after each sub-phase)
- [ ] CI: benchmark regressions in critical paths (Falcon verify, block apply) are CI-failures after Phase 2

### Tests
- [ ] Harness unit: single validator starts, produces block, height advances
- [ ] Harness unit: 3 validators all reach same height after N rounds
- [ ] All integration tests pass with real libp2p loopback (not mocked I/O)

---

## Sub-phase 2.11 — User Docs + Devnet Deployment

**From:** Roadmap.md (Phase 2 goal), UserDocs.md, Architecture.md (Docker, multi-validator simulation)

Create operator-facing documentation. Docker compose for devnet deployment. Monitoring setup.

### User docs
- [ ] `user_docs/README.md` — index + quick start: clone → configure → run validator
- [ ] `user_docs/Devnet.md` — local devnet deployment guide (per UserDocs.md requirements):
  - [ ] Hardware requirements section (CPU, RAM, disk, bandwidth per tier)
  - [ ] Bootstrap key generation: `mononium-cli wallet keygen`
  - [ ] Genesis configuration: template, MONEX allocation, bootstrap pubkeys, era 0 length, max_validators
  - [ ] Docker compose: multi-service bootstrap + validators + RPC node
  - [ ] Monitoring: `--metrics-addr` (if added), Prometheus scrape, Grafana dashboard

### Docker deployment
- [ ] `Dockerfile` — multi-stage build: cargo build → minimal distroless runtime
- [ ] `docker/docker-compose.yml` — bootstrap node + 3 validators + RPC observer
- [ ] `docker/grafana/dashboard.json` — validator monitoring dashboard
- [ ] `docker/prometheus/prometheus.yml` — scrape config

### Phase 2 exit criteria
- [ ] `cargo build -p mononium-lib` passes
- [ ] `cargo build -p mononium-cli` passes
- [ ] `cargo nextest run -p mononium-lib` passes (≥ 400 tests)
- [ ] `cargo bench -p mononium-lib` runs all suites
- [ ] 3-validator in-process cluster produces 20 blocks with BFT finality
- [ ] `mononium-cli wallet register/stake/unstake` creates signed txs, submits via POST /tx, validators process them
- [ ] P2P sync: two nodes on same machine, one ahead → lagging node catches up
- [ ] Slashing: equivocation evidence submitted → validator frozen → fork resolved by heaviest chain
- [ ] Governance: propose → vote → tally at era boundary → param change executes
- [ ] Docker compose: `docker compose up -d --scale validator=3` produces blocks with consensus
- [ ] Crash recovery: kill validator → restart → resume from last verified height without state loss
- [ ] Coverage: ≥ 90% region on all Phase 2 new modules (network, governance, consensus/finality, consensus/slashing)
