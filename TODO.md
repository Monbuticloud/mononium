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
> **Dependency order:** Staking txs (2.0) → P2P core (2.1) → block propagation (2.2) → consensus engine (2.3) → slashing (2.4) → governance (2.5) → RPC (2.6) → multi-validator CLI (2.7) → stake CLI (2.8) → crash recovery (2.9) → benchmarks (2.10) → docs (2.11).
> **Source docs:** All items below are extracted from `docs/plans/V1.0.0/` — Architecture.md, Consensus.md, Network.md, Protocol.md, Validators.md, Genesis.md, Fees.md, Slashing.md, Governance.md, Storage.md, Testing.md, UserDocs.md, StateSharding.md.

---

## Sub-phase 2.0 — Staking Transaction Types

**From:** Protocol.md §Transaction Types, Validators.md §Lifecycle §Staking, Fees.md §Standard Fees, Architecture.md §mononium-lib module tree

Add 4 staking TxBody variants and wire through the state machine. Implement the full validator lifecycle (Registered → Staked → Active → Unstaking → Frozen → Thawed → Inactive) with era boundary hooks for Open (era 0) and Top-N (era 1+) election.

### TxBody type definitions — `core/transaction.rs`

**4 new variants:**
- `TxBody::RegisterValidator { public_key: [u8; 897] }` — one-time declaration tx. Falcon-512 public key. No amount — registration is gas-only.
- `TxBody::Stake { validator: [u8; 32], amount: U256 }` — lock MONEX to a registered validator. `validator` = 32-byte address of target. `amount` must be > 0.
- `TxBody::RegisterAndStake { validator: [u8; 32], amount: U256 }` — atomic convenience: register + stake in one tx. `validator` must equal sender's address (self-registration).
- `TxBody::Unstake { validator: [u8; 32], amount: U256 }` — begin withdrawal from a validator. `amount` must be ≤ validator's current stake. `release_era = current_era + 168` computed at execution time.

**Serialization:**
- SCALE enum tag order: Transfer=0, RegisterValidator=1, Stake=2, RegisterAndStake=3, Unstake=4, Burn=5 — confirm tag values don't break existing deserialization
- `Encode` + `Decode` derive for all 4 variants (parity-scale-codec)
- `Serialize` + `Deserialize` for JSON (serde, adjacently tagged)
- Symmetric roundtrip tests: SCALE encode → decode equals original for every variant
- Symmetric roundtrip tests: JSON serialize → deserialize equals original
- Edge case tests: `Stake { amount: U256::zero() }` serializes correctly (validation catches zero at state machine level)
- Edge case tests: `Unstake { amount: U256::MAX }` serializes correctly
- Edge case tests: `RegisterValidator { public_key: [0u8; 897] }` serializes correctly (validators must verify key != 0 at state machine level)
- Envelope serialization: `Transaction { chain_id, nonce, sender, fee, body, signature }` SCALE roundtrip with each new body variant
- Signature verification: `falcon_verify(SCALE(chain_id || nonce || sender || fee || body))` for each variant — test that signature covers all fields

### Validator state types — `core/validator.rs` (new file)

`ValidatorStatus` enum:
```rust
pub enum ValidatorStatus {
    Registered,                                    // registered, no stake yet
    Staked { stake: U256 },                        // has ≥ 1 MONEX (era 1+), in candidate pool
    Active,                                         // in active validator set
    Unstaking { release_era: u64, amount: U256 },   // cooldown in progress
    Frozen { frozen_until: u64 },                  // slashed, 72-era exclusion
    Thawed,                                         // freeze expired, back in pool
}
```

`ValidatorEntry` struct:
```rust
pub struct ValidatorEntry {
    pub address: [u8; 32],
    pub public_key: [u8; 897],       // Falcon-512 public key
    pub stake: U256,                  // total MONEX staked to this validator
    pub status: ValidatorStatus,
    pub registration_era: u64,        // era in which RegisterValidator was included
}
```

**SCALE/JSON derives:**
- `Encode` + `Decode` for `ValidatorStatus` (including inner fields for Unstaking/Frozen)
- `Encode` + `Decode` for `ValidatorEntry`
- `Serialize` + `Deserialize` for both
- Symmetric roundtrip tests: every ValidatorStatus variant, ValidatorEntry with edge-case values

### State machine staking operations — `core/state.rs`

**RegisterValidator:**
- Look up `NS_VALIDATORS ++ sender_addr` in SMT — if exists, error "already registered" (fee-only deduction)
- Verify sender has sufficient balance for fee (HybridFee) — if insufficient, fee-only deduction
- Deduct fee, deduct anti-spam deposit (0.33 MONEX), increment nonce
- Create ValidatorEntry { address: sender, public_key, stake: 0, status: Registered, registration_era: current_era }
- Store in SMT at `NS_VALIDATORS ++ sender_addr`
- Return `ExecutedResult { fee, deposit }`
- Error cases: already registered, insufficient balance for fee, invalid nonce, bad signature

**Stake:**
- Look up `NS_VALIDATORS ++ validator` — if not found, fee-only deduction, error "validator not found"
- If validator.status is Frozen or Unstaking → reject with appropriate error, fee-only deduction
- Deduct `amount` from sender's transferable balance — if insufficient (fee + amount + deposit), fee-only deduction
- Add `amount` to validator.stake, update entry in SMT
- Increment sender nonce, deduct fee + deposit
- Error cases: validator not found, validator frozen, validator unstaking, insufficient sender balance, amount = 0, U256 overflow on validator.stake

**RegisterAndStake:**
- Atomic: run RegisterValidator checks, then Stake checks in single transaction
- Single fee deduction, single nonce increment, single deposit
- If RegisterValidator fails → entire tx rejected (no partial state — validator NOT registered)
- If RegisterValidator succeeds but Stake fails → entire tx rejected (validator NOT registered)
- Era 1+ minimum: `amount >= 1 MONEX` required (era 0: no minimum)
- Error cases: same as RegisterValidator + Stake combined

**Unstake:**
- Look up `NS_VALIDATORS ++ validator` — if not found, fee-only deduction
- If validator is Frozen → reject (cannot unstake from frozen validator — must wait for thaw)
- If sender != unstaking address → reject (only the staker can unstake — or can anyone unstake from any validator? Per Validators.md, staker initiates unstake, not the validator. So sender must be the original staker.)
  - Actually, the stake tx moves amount from sender to validator.stake. The validator entry doesn't track "who staked what." All stake is fungible. Anyone can unstake — it reduces the validator's total stake. The amount goes to... the unstake tx sender? Or back to the original stakers? Per Validators.md §Unstaking: "Unstaked funds become available after the 168-era cooldown." The funds return to the sender of the Unstake tx (not the original staker). This means anyone can unstake from any validator, but the funds go to the Unstake tx sender.
  - Per Governance.md: "voting power = total staked MONEX" — stake is tracked per-validator-total, not per-individual-staker. No delegation in V1.
- **Decision:** Unstake tx sender receives `amount` after cooldown. Anyone can unstake from any validator (reduces validator's total stake). The validator operator cannot prevent this.
- Set `release_era = current_era + UNSTAKING_COOLDOWN` (168)
- Update validator entry in SMT
- Error cases: validator not found, validator → Unstaking with release_era, amount = 0, amount > validator.stake

### Validator lifecycle integration

**Era 0 Open election (hook in `consensus/era.rs`):**
- When `election_mode == Open`: iterate all validators with status != Frozen, set top N up to `max_validators` to Active
- No stake sorting — first-come-first-served (by registration_era) if count exceeds max_validators
- Validators registered mid-era become Active immediately (not at next era boundary)
- Unit tests: register 5 with max=3 → first 3 active, last 2 Registered; register mid-era → immediately active

**Era 1+ Top-N election (hook in `consensus/election.rs`):**
- Collect validators with `stake >= MIN_VALIDATOR_STAKE` (1 MONEX) and status != Frozen
- Sort by stake descending, take top `max_validators`, set to Active
- Ties broken by registration_era (earliest wins)
- Remaining validators stay Staked (candidate pool, eligible next era)
- Frozen/Unstaking validators excluded from candidate pool
- Thawed validators with remaining stake re-enter candidate pool

**Unstaking cooldown (era boundary hook):**
- For each validator with `status == Unstaking { release_era, amount }` and `release_era <= current_era`:
  - Full unstake: `amount == validator.stake` → remove entry from SMT (validator becomes Inactive)
  - Partial unstake: `amount < validator.stake` → `validator.stake -= amount`, status = Staked (if ≥ 1 MONEX) or Registered (if < 1 MONEX)
  - Transfer `amount` to the unstake tx sender's transferable balance
  - Note: this requires tracking who submitted the Unstake tx. Store `unstake_initiator` in the Unstaking struct or as a separate record.

**Validator status transitions (comprehensive state machine):**
- Registered → Staked (via Stake tx from any sender)
- Registered → Active (era 0 auto-promotion)
- Staked → Active (era boundary Top-N election)
- Active → Staked (era boundary, fell out of top N)
- Staked/Active → Frozen (equivocation evidence submitted)
- Frozen → Thawed (72 eras pass)
- Thawed → Staked (era boundary, re-enters candidate pool if stake ≥ 1 MONEX)
- Thawed → Registered (era boundary, stake < 1 MONEX)
- Staked/Active → Unstaking (via Unstake tx)
- Unstaking → Inactive (cooldown expires, fully unstaked)
- Unstaking → Staked (cooldown expires, partially unstaked)
- Unstaking → Registered (cooldown expires, remaining < 1 MONEX)

### Fee integration — `core/fee.rs`
- Standard `HybridFee` (flat 0.00667 + per-byte 0.000467 + tip) applies to staking txs
- RegisterValidator tx includes 897-byte public_key — larger per-byte component
- Anti-spam deposit (0.33 MONEX) applies (no exemption per Fees.md)
- Fee-only deduction on failed staking txs identically to Transfer/Burn
- Unit tests: each staking tx variant calculates correct fee, fee-only deduction on failure

### Constants — `core/constants.rs`
- `UNSTAKING_COOLDOWN_ERAS: u64 = 168` — 168-era cooldown (~7 days)
- `MIN_VALIDATOR_STAKE: U256 = U256::from_str(\"100000000000000000000000000000000\").unwrap()` — 1 MONEX in MOXX (10^32)
- `MAX_VALIDATORS_DEFAULT: usize = 21`
- Verify all new constants have unit tests

### Full test matrix — `core/state.rs` and `core/transaction.rs`

**Happy path:**
- RegisterValidator → ValidatorEntry created with correct fields
- Stake → validator.stake increased, sender balance decreased
- RegisterAndStake (era 0) → atomic register + stake in single tx
- RegisterAndStake (era 1+) → minimum stake enforced
- Unstake → status changes to Unstaking with correct release_era
- Cooldown expiry → balance returned, validator entry updated
- Cross-stake (sender != validator) → allowed, stake increases
- Self-stake (sender == validator) → allowed

**Error path (fee-only deduction for each):**
- Register when already registered
- Register with insufficient balance for fee
- Stake to nonexistent validator
- Stake to frozen validator
- Stake to unstaking validator
- Stake with amount = 0
- Stake with insufficient sender balance
- U256 overflow on validator's total stake
- Unstake from nonexistent validator
- Unstake amount > validator.stake
- Unstake from frozen validator
- Unstake amount = 0
- RegisterAndStake with amount < 1 MONEX (era 1+, full tx rejected)

**Era boundary:**
- Era 0: 5 validators register, max=3 → first 3 active
- Era 1+: stakes [100, 50, 30, 20, 10], max=3 → active = [100, 50, 30]
- Tie-breaking: equal stakes, different registration_era
- Frozen validator excluded from election
- Unstaking cooldown expires at era boundary

**Coverage target:** ≥ 95% region on staking state machine paths, ≥ 100% on TxBody variants

---

## Sub-phase 2.1 — P2P Networking (Core)

**From:** Network.md §P2P Layer §Topics §Peer Discovery §Transport Compression §Ports, Architecture.md §mononium-lib `network/` module tree, NodeConfig.md §network.*

Build the `network/` module. libp2p gossipsub with 4 topics, kademlia + mDNS discovery, Identify protocol, peer scoring with ban mechanics, snappy compression.

### Files to create — `mononium-lib/src/network/`

**`network/mod.rs` — Public API:**
- `P2pService` struct: `{ swarm: Swarm<Behaviour>, local_peer_id: PeerId, chain_id: u64, peer_scores: Arc<RwLock<PeerScoreRepo>> }`
- `P2pService::new(config: P2pConfig, chain_id: u64) -> Result<Self>` — construct swarm
- `P2pService::start(self) -> JoinHandle` — spawn async event loop
- `P2pService::stop(&self)` — graceful shutdown signal
- `P2pService::publish_tx(txs: &[Transaction]) -> Result<MessageId>`
- `P2pService::publish_block(block: &Block) -> Result<MessageId>`
- `P2pService::publish_vote(vote: &CommitVote) -> Result<MessageId>`
- `P2pService::publish_evidence(evidence: &EquivocationEvidence) -> Result<MessageId>`
- Event loop: poll swarm → match on event → dispatch to handler
- `P2pConfig`: `{ p2p_port: u16, bootstrap_peers: Vec<Multiaddr>, enable_mdns: bool, max_peers: usize }`

**`network/constants.rs`:**
- `DEFAULT_P2P_PORT: u16 = 30333`
- `PROTOCOL_VERSION: &str = \"mononium/1.0\"`
- `AGENT_VERSION: &str = \"mononium-node/0.1.0\"`
- `MAX_PEERS: usize = 50`
- `MAX_CONNECTIONS_PER_PEER: u32 = 1`
- `KADEMLIA_REPLICATION_FACTOR: usize = 20`
- Topic name templates: `\"mononium/txs/{chain_id}\"`, `\"mononium/blocks/{chain_id}\"`, `\"mononium/votes/{chain_id}\"`, `\"mononium/evidence/{chain_id}\"`

**`network/topics.rs` — Topic config + validation:**
- `TopicConfig { name: String, max_message_size: usize, max_rate_per_peer: u32 }`
- 4 instances matching Network.md table:
  - txs: 1_048_576 bytes, 20 msg/s
  - blocks: 512_000 bytes, 1 msg/s
  - votes: 1_024 bytes, 100 msg/s
  - evidence: 5_120 bytes, 5 msg/s
- `RateLimiter` — per-peer sliding window counter (1s window)
- `RateLimiter::check(peer_id, topic) -> bool` — returns false if exceeded
- `RateLimiter::increment(peer_id, topic)` — record message
- `validate_message_size(topic, raw_bytes) -> bool` — raw SCALE bytes, before deserialization
- Unit tests: rate limiter accepts under limit, rejects over limit, resets after window; size validation passes/fails at boundaries

**`network/messages.rs` — Wire message types:**
- `GossipMessage` enum (SCALE): `Txs(Vec<Transaction>)`, `Block(Box<Block>)`, `Vote(CommitVote)`, `Evidence(EquivocationEvidence)`
- SCALE `Encode` + `Decode` for all variants
- Each variant size-limited at publish time
- Unit tests: all 4 variants encode/decode symmetric, roundtrip with real data

**`network/discovery.rs` — Peer discovery:**
- Bootstrap: dial all `bootstrap_peers` multiaddrs concurrently (10s timeout)
- Kademlia: random walk every 60s, provider records for chain_id
- mDNS: local network discovery (localnet only)
- Identify: exchange agent version, protocol version, listen addrs
- `PeerMetadata { shards: Vec<u16> }` stored in `HashMap<PeerId, PeerMetadata>`

**`network/peer_score.rs` — Peer scoring + bans:**
- `PeerScore { score: i32, banned_at_height: Option<u64>, last_positive: Instant }`
- Range: [-100, 100], starts at 0
- `adjust(delta: i32)` — clamp to bounds
- `is_banned(current_height: u64) -> bool` — `banned_at_height.is_some() && current_height < banned_at_height + BAN_DURATION`
- `should_ban() -> bool` — `score < -20`
- `apply_ban(current_height: u64)` — set `banned_at_height`
- `BAN_DURATION: u64 = 720` blocks (~1 hour)
- Fresh-genesis edge case: if chain height < 720, use 1-hour wall-clock fallback
- Score adjustments (11 events from Network.md table):
  | Event | Delta | Triggering condition |
  |-------|-------|---------------------|
  | Valid block propagated | +1 | Block verification passed |
  | Valid vote propagated | +1 | Signature verified |
  | Successful sync batch | +2 | Batch matched + full verification passed |
  | Sync batch hash mismatch | -10 | ADR-018 rolling hash differs |
  | Sync batch verify fail | -20 | global_state_root mismatch |
  | Empty sync response | -2 | Peer has blocks but returns empty |
  | Sync timeout (2+ consecutive) | -4 | No response |
  | Invalid block gossiped | -10 | Block fails basic validation |
  | Invalid vote gossiped | -10 | Signature fails |
  | Connect/disconnect loop | -10 | >3 disconnects in 5 min |
  | Duplicate block gossip (>3 identical) | -2 | Repeated identical blocks |
- `PeerScoreRepo` — `HashMap<PeerId, PeerScore>` behind `Arc<RwLock<>>`
- Unit tests: every delta adjusts correctly, clamping, ban threshold at -20, ban expiry at +720 blocks, wall-clock fallback at height 0, recidivism (score persists after unban)

### Gossipsub configuration:
- `GossipsubConfigBuilder`: message_id_fn (BLAKE3 hash of raw bytes), max_transmit_size=1MB, history_length=10, gossip_factor=0.25
- Subscribe to all 4 topics on `P2pService::start()`
- Incoming message handler: deserialize → validate size/rate → store score → route to handler
- Outgoing: serialize → validate size → publish via gossipsub

### Transport:
- TCP with Noise XX + yamux (libp2p built-in)
- Snappy compression at transport layer
- DNS resolution for multiaddrs
- Port reuse for NAT traversal

### Config integration:
- `config/mod.rs`: `network.p2p_port` (u16, default 30333), `network.bootnodes` (Vec<String>, default []), `network.enable_mdns` (bool, default true)
- Validation: p2p_port != rest_port (9933), p2p_port != rpc_port (9944); bootnodes must be valid multiaddrs
- CLI flags: `--p2p-port`, `--bootnodes` (repeatable)
- `config/constants.rs`: `DEFAULT_P2P_PORT: u16 = 30333`
- Example configs: `node.localnet.yaml` (mdns=true, empty bootnodes), `node.devnet.yaml` (bootnodes populated)

### Integration tests:
- Two P2pService instances on loopback TCP → connect, subscribe, publish/receive message
- mDNS discovers peer on loopback
- Kademlia bootstrap with known peer
- Oversized message rejected at topic level, sender score decremented
- Rate-limited peer score decremented, recovers after window

---

## Sub-phase 2.2 — Block Propagation + Sync Protocol

**From:** Network.md §Sync Protocol (complete flow, BlockSyncRequest/Response, BlockByHashRequest/Response, SyncCursor, Disconnection, Fork Detection, Retry Logic, No-Peer Stall), ADR-018 (rolling batch hash)

Blocks gossiped on `mononium/blocks/{chain_id}`. Sync protocol via libp2p Request-Response for catch-up. SyncCursor persists position across restarts. Rolling BLAKE3 batch hash per ADR-018.

### Block gossip handler:
- On receive: deserialize → validate size ≤ 500KB (consensus cap) → verify proposer Falcon sig → verify parent_hash exists in chain → verify timestamp ±2s → verify chain_id → check duplicate (reject if exists) → queue for state machine → re-gossip
- Invalid block: log warning, score proposer peer -10, do NOT re-gossip

### Sync protocol messages (`network/messages.rs` additions):
- `BlockSyncRequest { start_height: u64, max_blocks: u16 (max 500), direction: SyncDirection, known_block_hash: Option<[u8;32]> }`
- `BlockSyncResponse { blocks: Vec<Block>, highest_height: u64, batch_hash: [u8;32] }`
- `BlockByHashRequest { block_hashes: Vec<[u8;32]> }` (max 100)
- `BlockByHashResponse { blocks: Vec<Block> }` (request order, missing omitted)
- `SyncDirection` enum: `Forward | Backward`
- `compute_batch_hash(genesis_hash, blocks) -> [u8;32]` — rolling BLAKE3 per ADR-018
- All types: SCALE encode/decode, validation (max_blocks ∈ [1,500], max_hashes ∈ [1,100])
- Registered as libp2p Request-Response protocols: `/mononium/sync/1.0`, `/mononium/hash-sync/1.0`

### SyncCursor (`network/sync.rs`):
- `SyncCursor { last_verified_height: u64, last_verified_hash: [u8;32], target_height: u64, pending_range: Option<HeightRange> }`
- `HeightRange { start: u64, end: u64, peer_id: PeerId }`
- Methods: `new(genesis_hash)`, `advance(to_height, to_hash)`, `set_target(height)`, `set_pending(range)`, `clear_pending()`, `gap() -> u64`, `needs_checkpoint() -> bool`
- Persistence: `save(path)` / `load(path)` — JSON file at `{data_dir}/{chain_id}/sync_cursor.json`
- On load failure: return `new(genesis_hash)` → full replay fallback
- Persist frequency: after every verified batch (100 blocks)

### Sync flow:
**Init:** Load cursor → connect to ≥1 peer (30s timeout, backoff) → send Backward request (max=1) → set `target_height = response.highest_height` → if `gap > 2*ERA_LENGTH` → checkpoint path (Phase 2.9) else → block catch-up

**Block catch-up loop:**
- Select peer (round-robin through Good peers, skip Neutral)
- Request 100 blocks with `known_block_hash = last_verified_hash`
- Timeout: 5s → 10s → 15s per peer; after 3 peers → 10s pause, restart range
- On response: compute `local_batch_hash` → compare to `response.batch_hash` → mismatch means fork
- If hash matches: verify parent_hash chain → verify each block (sig, timestamp, re-execute) → if any fails → entire batch rejected
- If all pass: `cursor.advance(len, last_hash)`, persist, continue

**known_block_hash anchor (fork prevention):**
- Requesting peer sends `last_verified_hash` as anchor
- Responding peer MUST verify anchor exists at `start_height - 1`; if not → return empty `blocks: []`
- Syncing node receives empty → tries different peer (first peer is on different fork)

**Fork detection:**
- Batch hash mismatch → disconnect, score -= 10, try next peer
- If peer B returns different blocks than peer A → request from peer C (tiebreaker, majority wins)
- Minority peer scored down (score -= 5)

**Disconnection handling:**
- Mid-batch disconnect: discard unverified blocks, clear pending_range, request same range from next peer with same known_block_hash
- Stateless: no session, no partial batch tracking

**Retry (per Network.md table):** 5s/10s/15s per attempt, 10s pause after 3 failures, exponential backoff (10s→30s→60s→120s→300s) after 10 consecutive batch failures across all peers
**No-peer stall:** retry forever (5s→10s→30s→30s), never exit or panic
**Sync complete:** `last_verified_height >= target_height` AND last block timestamp within 10s of local clock AND ≥1 peer AND no pending verification

### Integration tests:
- Two nodes (height 100 vs 0) → B syncs to A's tip, state roots match
- Disconnection mid-batch (50/100 blocks) → resume with new peer, no gaps
- Fork detection: peer A serves wrong fork → B detects via batch_hash, switches to peer C

---

## Sub-phase 2.3 — PoS Consensus Engine

**From:** Consensus.md §Finality §Flow §Slot Model §Missed Slots §Fork-Choice §DI Pattern, Architecture.md §consensus/finality.rs, Protocol.md §Block Application Order

BFT commit tracking, proposer schedule integrated with real block production, verification window (4 blocks), slot timer, fork-choice (heaviest chain), missed slot penalty (0.08 MONEX/slot).

### `consensus/finality.rs` — CommitTracker
- `CommitTracker { commits: BTreeMap<u64, Vec<CommitVote>>, stake_weights: HashMap<[u8;32], U256>, total_active_stake: U256, finalized: BTreeSet<u64> }`
- `new(stake_weights)` — load from state at era boundary
- `add_vote(vote) -> bool` — verify: active validator, valid Falcon sig, not duplicate; if new and `cumulative_weight > 2/3 * total_active_stake` → mark finalized
- `cumulative_weight(height) -> U256` — sum of unique validators' stakes who committed
- `is_final(height) -> bool`, `last_finalized_height() -> u64`, `finality_ratio(height) -> f64`
- Strict threshold: `> 2/3` (NOT ≥ 2/3)
- Unit tests: vote adds weight, finality at >2/3, NOT at exactly 2/3, duplicate rejected, non-active validator rejected, bad signature rejected, 3 validators (33% each) → 2 votes final, 1 vote not

### `consensus/proposer.rs` — ProposerSchedule
- `ProposerSchedule { active_set: Vec<[u8;32]>, era: u64, start_height: u64 }`
- `proposer_for_height(height) -> [u8;32]` — `active_set[height % len]`
- `is_scheduled_proposer(proposer, height) -> bool`
- Unit tests: 3 validators cycle correctly, single validator proposes every slot, empty set panics (guarded by era 0 guarantee)

### `consensus/mod.rs` — ConsensusEngine
- Dependencies: ConsensusConfig, ProposerSchedule, CommitTracker, StateMachine, StorageEngine, Mempool, P2pService, optional ValidatorKey
- `start_slot_timer()` — tokio interval at 5s, aligned to `genesis_time + N * block_time`
- Each slot tick: if proposer → `propose_block()`, else → `await_block(5s)` with timeout

**Proposer:**
- Collect txs from mempool (up to 500 txs / 500KB)
- Include collected CommitVotes (gossiped during verification window)
- Build BlockHeader, execute all txs → compute global_state_root, tx_root
- Sign with `proposer_signature: [u8;666]` (new field on BlockHeader)
- Store block in redb, publish via gossipsub

**Non-proposer:**
- Wait for block (5s timeout)
- On receive: verify proposer sig, proposer match, timestamp ±2s, parent_hash match, re-execute txs → verify global_state_root
- If valid: sign CommitVote, gossip, store block, advance height
- If invalid: reject, no vote, score proposer peer

**BlockHeader change:** Add `proposer_signature: [u8;666]` field. SCALE + JSON roundtrip. Backward compat: Phase 1 blocks don't have this field — dev-only, acceptable.

### Fork-choice: `select_canonical(chain_a, chain_b, stake_weights)`
- `total_stake_backing(chain)` — sum unique proposers' stake weights
- Heavier chain wins; equal weight → no switch
- Used in sync when peers disagree (tiebreaker from Phase 2.2)

### Missed slot penalty (`consensus/era.rs`):
- `MISSED_SLOT_PENALTY: U256 = 8 * 10^30 MOXX` (0.08 MONEX)
- At era boundary: for each active validator, `missed = ERA_LENGTH - blocks_proposed`, penalty = `missed * MISSED_SLOT_PENALTY`
- Penalty sent to Cap-Refill (`0x00..01`)
- If penalty > validator.stake → fully de-staked, ejected from active set (no debt)
- Unit tests: 10 missed slots → 0.8 MONEX penalty; 720 missed → 57.6 MONEX; penalty > stake → de-staked

### Fee distribution (end of each block):
- `total_fees = sum(all_tx_fees)`, `total_active_stake = sum(all_active_stakes)`
- Each validator: `share = total_fees * own_stake / total_active_stake`
- Remainder (integer truncation) → validator with lowest address among highest-stake

### Integration tests:
- Single proposer produces N blocks, height advances, blocks stored
- 3-validator cluster produces 20 blocks with >2/3 BFT commits on each
- Missed slot → empty slot, next proposer builds on last canonical
- Clock drift rejection (timestamp >2s from local)
- Fee distribution correct across all validators

---

## Sub-phase 2.4 — Slashing

**From:** Slashing.md §Evidence Format §Freeze Period §Thaw §Double-Slashing, Consensus.md §Equivocation Fork Resolution

Equivocation evidence type + verification. State machine: 90% stake slash + 10% reporter bounty. 72-era freeze management. Evidence gossiping.

### `EquivocationEvidence` struct (`consensus/slashing.rs`):
- `{ header_a: BlockHeader, signature_a: [u8;666], header_b: BlockHeader, signature_b: [u8;666], proposer: [u8;32] }`
- SCALE + JSON derives, roundtrip tests
- `verify_equivocation(evidence, public_key) -> Result<()>`:
  1. `header_a.height == header_b.height` — same height
  2. `header_a.parent_hash == header_b.parent_hash` — same parent
  3. `header_a != header_b` — distinct (hash differs)
  4. `falcon_verify(pk, SCALE(header_a), sig_a)` — valid
  5. `falcon_verify(pk, SCALE(header_b), sig_b)` — valid
- 5 distinct error variants in LibError for each failure
- Unit tests: all 5 checks with valid/invalid inputs; identical blocks rejected; one good + one bad sig rejected

### Evidence gossip:
- Published on `mononium/evidence/{chain_id}` via `P2pService::publish_evidence()`
- Incoming handler: validate size ≤ 5KB, queue for state machine
- Evidence submitted via P2P gossip (NOT as a transaction — direct state machine call)

### State machine: `apply_slash(evidence, reporter_addr)`:
- Load validator entry, verify evidence, verify not already frozen
- Compute: `slashed = stake * 90 / 100`, `burn = slashed * 90 / 100`, `bounty = slashed * 10 / 100`, `remaining = stake - slashed`
- Send `burn` to `0x00..00` (permanent destruction)
- Credit `bounty` to reporter's locked balance
- Reporter types: active validator → bounty added to stake; inactive staker → added to stake; non-validator → locked MONEX balance (not spendable)
- Update validator: `stake = remaining, status = Frozen { frozen_until: current_era + 72 }`
- Return `SlashResult { slashed_amount, burn_amount, bounty_amount, remaining_stake }`
- Unit tests: 1000 MONEX → 810 burn + 90 bounty + 100 remaining; 10 MONEX → 8.1 burn + 0.9 bounty + 1 remaining; already-frozen rejected; bad evidence rejected

### Freeze management (era boundary hook):
- For each validator with `status == Frozen` and `frozen_until <= current_era`:
  - If `stake >= MIN_VALIDATOR_STAKE` → status = Thawed (re-enters candidate pool)
  - If `stake < MIN_VALIDATOR_STAKE` → status = Registered (no stake, must re-stake)
- Frozen validators: excluded from proposer schedule, cannot vote, excluded from fee distribution
- Mid-era freeze: slot goes empty (not reassigned), no additional missed-slot penalty
- Era boundary processing order: thaw → election → proposer schedule
- Double-slashing: can be slashed again after thawed (fresh 72-era freeze)
- Already-frozen: secondary evidence ignored
- Unit tests: freeze countdown, thaw at era boundary, mid-era freeze excludes from fee distribution, thawed validator re-elected

### Integration tests:
- 3-validator cluster, A equivocates → B submits evidence → A slashed, frozen → A's remaining slots go empty → fork resolved by heaviest chain → 72 eras later A thaws

---

## Sub-phase 2.5 — Governance Module

**From:** Governance.md (complete — Proposal Lifecycle, Types, Tally, Execution, Era-boundary Hook, Parameter Bounds, State Machine Integration)

On-chain stake-weighted governance. Proposal submission, 7-era voting window, era-boundary tally (quorum ≥2/3, threshold >50%), automatic execution at next era boundary. 10 mutable parameters with bounds.

### Module structure — `governance/`
- `types.rs` — `Proposal { proposal_id, proposer, title, description, actions, deposit, submission_era, status }`
- `ProposalStatus { Active, Approved, Rejected, Expired, Cancelled }`
- `Vote { proposal_id, voter, approve: bool, weight: U256, block_height: u64 }`
- `GovernanceAction { UpdateParam { param: GovernanceParam, new_value: U256 }, IncreaseShards { new_count, effective_era } }`
- `GovernanceParam` enum (10 variants): MaxValidators, EraLength, BlockSizeCapBytes, BlockTxCap, FlatFee, PerByteRate, AntiSpamDeposit, MissedSlotPenalty, SupplyCeilingRate, SupplyHeadroomRate
- All types: SCALE + JSON derives, roundtrip tests
- `constants.rs` — `PROPOSAL_DEPOSIT = 100 * ONE_MONEX`, `VOTING_WINDOW_ERAS = 7`, `MAX_ACTIVE_PROPOSALS_PER_PROPOSER = 5`, `MAX_PROPOSALS_PER_ERA = 50`, quorum = 2/3 numerator/denominator, title max 256 bytes, desc max 4096 bytes
- `PARAM_BOUNDS: HashMap<GovernanceParam, (U256, U256)>` — min/max per param matching Governance.md bounds table

### GovernanceEngine (`governance/mod.rs`):
- `submit_proposal(tx_args)` — validate stake ≥ 100 MONEX, title/desc size, actions, param bounds, rate limits, param lock; deduct deposit; store in SMT at `NS_GOVERNANCE ++ prop_{id}`
- `cast_vote(tx_args)` — validate proposal exists and in window, voter has >0 stake, snapshot weight, store/overwrite vote at `NS_GOVERNANCE ++ vote_{id}_{voter}`
- `cancel_proposal(proposal_id, caller)` — only proposer, only before any votes, only during window; return deposit
- `tally_proposals(current_era)` — for each proposal at `submission_era + 7 == current_era`: sum votes, check quorum (≥2/3 total active stake), check threshold (>50% of participating), return deposit or forfeit
- `execute_approved(current_era)` — sort approved proposals by proposal_id hash, apply each action in order; last-write-wins for conflicting params
- SMT namespace: `NS_GOVERNANCE = 0x03`

### TxBody variants:
- Option A (chosen): Governance as regular transactions — `TxBody::Propose { proposal }`, `TxBody::Vote { proposal_id, approve }`, `TxBody::CancelProposal { proposal_id }`
- Standard fee + anti-spam deposit apply

### Era-boundary integration:
- After validator set recalculation, before proposer schedule reset:
  1. Tally proposals whose window closed
  2. Execute approved proposals
  3. Update consensus params from governance state

### 31 unit tests + 6 integration tests (from Governance.md test matrix):
- All validation rules (stake, size, bounds, rate limits, locks, param bounds for every param)
- All vote rules (window, stake, overwrite)
- Cancel rules (proposer, no votes, window)
- Tally: quorum met/passed, met/rejected, not met/expired; boundaries (exactly 2/3, 50/50 split)
- Execution: single param, multiple params, last-write-wins, sorted order
- Deposit: deduction, return, forfeit
- Full flow: propose → vote → tally → execute param change
- Parameter lock: same param blocked, different params allowed

---

## Sub-phase 2.6 — RPC (jsonrpsee + REST Expansion)

**From:** Architecture.md §RPC Interface (JSON-RPC methods table, error codes, subscriptions table), NodeConfig.md §network.rpc_port

Add jsonrpsee WebSocket server. 16 JSON-RPC methods + 3 subscriptions. Expand REST with missing endpoints. Both servers on separate ports.

### `rpc/jsonrpc.rs` — jsonrpsee server
- Port: `network.rpc_port` (default 9944)
- Methods (exact params + returns from Architecture.md table):
  - `tx_submit(Transaction)` → `TxHash`
  - `tx_status(TxHash)` → `{ status: \"pending\"|\"finalized\"|\"failed\", height?: u64, index?: u32 }`
  - `block_get(BlockId)` → `Block`
  - `block_header(BlockId)` → `BlockHeader`
  - `block_latest()` → `BlockHeader`
  - `state_get_balance(Address)` → `U256` (hex string)
  - `state_get_nonce(Address)` → `u64`
  - `validator_set()` → `Vec<ValidatorInfo>`
  - `validator_stake(Address)` → `U256`
  - `era_current()` → `u64`
  - `chain_get_height()` → `u64`
  - `chain_get_genesis()` → `Hash`
  - `chain_get_health()` → `{ status, height, peers, finalized_height }`
  - `network_peers()` → `Vec<PeerInfo>`
  - `governance_proposals(status?)` → `Vec<Proposal>`
  - `governance_params()` → `Vec<GovernanceParamValue>`
- Subscriptions: `subscribe_blocks` (→ BlockHeader), `subscribe_finality` (→ FinalityEvent), `subscribe_votes` (→ CommitVote)
- Error codes: 0=Success, -1=Internal, -2=Invalid params, -3=Tx validation, -4=Block not found, -5=Tx not found, -6=Address not found, -7=Rate limited

### REST expansion (`rpc/rest.rs`):
- GET `/nonce/{address}` — returns u64
- GET `/validators` — `Vec<ValidatorInfo>`
- GET `/validator/{address}` — `ValidatorInfo`
- GET `/genesis` — genesis block hash (hex)
- GET `/block/{hash}` — block by 32-byte hex hash

### Config integration:
- `network.rpc_port` (u16, default 9944)
- Validation: rpc_port != p2p_port (30333) and != rest_port (9933)
- CLI: `--rpc-port`, `--rest-port`

### Integration tests:
- Submit tx via jsonrpsee → query status → verify in block
- REST + JSON-RPC simultaneously on different ports
- WebSocket subscription receives block events
- Error codes correct per method

---

## Sub-phase 2.7 — Multi-Validator Node Mode

**From:** Architecture.md §Node Startup Lifecycle (steps 1-16) §Startup Preview, Consensus.md §Bootstrap Phase, Network.md §Network Configuration, Validators.md §Multi-Validator Simulation

Full node startup lifecycle with P2P, consensus, bootstrap phase. Observer mode. Docker compose for local multi-validator.

### Node startup (mononium-cli/src/node.rs):

**Step 1-6 (Phase 1):** CLI args → config → merge → key load → passphrase → key derivation. Verify all Phase 2 config flags added.

**Step 7 — Startup preview (NEW):**
```
Mononium Node v0.1.0 — Startup Preview
  Network:      Devnet (chain ID: 1)
  Role:         Validator (key: my-validator)
  Address:      0x3a1b...checksum
  Data dir:     ~/.mononium/data/devnet/
  Genesis:      configs/genesis.devnet.json
  P2P:          127.0.0.1:30333
  REST:         127.0.0.1:9933
  RPC:          127.0.0.1:9944
  Boot peers:   3
  Storage:      full mode

Start node? [y/N]
```
- If N → clean exit (code 0). Prevents running wrong key on wrong network.
- Observer mode: show \"Role: Observer (no signing)\"

**Step 8-10 (Phase 1):** Open DB → genesis → load state

**Step 11 — Start libp2p:** `P2pService::new()` with merged config, `::start()`

**Step 12 — Connect bootnodes:** Bootstrap → kademlia → ≥1 peer required (30s timeout)

**Step 13 — Init consensus:** Load active set, generate proposer schedule

**Step 14 — Start slot timer:** Begin block production/voting

**Step 15 — Start RPC:** axum + jsonrpsee

**Step 16 — Signal handlers:** SIGINT/SIGTERM → graceful shutdown (10s timeout)

### Bootstrap phase:
- Genesis `bootstrap { public_keys: [...], blocks: N }` field
- Bootstrap duration per tier: localnet=1, devnet=20, testnet=100, mainnet=100
- Round-robin over bootstrap keys for first N blocks
- Non-bootstrap blocks rejected during phase
- At N+1: snapshot registered validators, run Open election, commit active set
- Bootstrap keys have no special status after phase
- Era 0: any registered validator is active (no stake needed)
- Era 1+: Top-N by stake (≥1 MONEX minimum)
- Update configs: all 4 genesis files with bootstrap field

### Observer mode:
- `observer: true` or `--observer` CLI flag
- No key loaded, no passphrase, no consensus participation
- Full block validation (same as non-proposer), no votes, no proposals
- RPC fully functional, P2P fully functional

### Config files:
- `configs/node.localnet.yaml` — mDNS=true, empty bootnodes
- `configs/node.devnet.yaml` — explicit bootnodes
- `configs/node.observer.yaml` — observer mode example

### Docker compose:
- `Dockerfile` — multi-stage: `rust:1.85-slim-bookworm` build → `debian:bookworm-slim` runtime
- `docker/docker-compose.yml` — bootstrap + N validators + RPC observer
- Unique ports per container (30333+N, 9933+N, 9944+N)
- Shared docker network, mDNS disabled, explicit bootnodes
- Key generation script: `docker/generate-keys.sh`
- `docker compose up -d --scale validator=3` → 1 bootstrap + 3 validators + 1 RPC

### Integration tests:
- Full startup lifecycle (16 steps, mocked I/O for passphrase)
- 3-validator in-process cluster produces 20 blocks with consensus
- Bootstrap phase: only bootstrap keys propose blocks 1..N
- Era 0 → era 1+ transition
- Observer node syncs from genesis without signing

---

## Sub-phase 2.8 — CLI Stake/Unstake Commands + Validator Queries

**From:** Architecture.md §mononium-cli CLI tree (wallet stake, query validator), Protocol.md §Staking Tx, Validators.md §Staking

Add CLI commands for staking. REST integration for submission + queries.

### Commands:
- `mononium-cli wallet register [--key <name>]` — creates RegisterValidator tx, submits via POST /tx
- `mononium-cli wallet stake <validator_addr> <amount> --key <name>` — creates Stake tx
- `mononium-cli wallet register-and-stake <validator_addr> <amount> --key <name>` — atomic register + stake
- `mononium-cli wallet unstake <validator_addr> <amount> --key <name>` — creates Unstake tx
- `mononium-cli query validator <address>` — validator info via GET /validator/{address}
- `mononium-cli query validators` — list all via GET /validators
- `mononium-cli query nonce <address>` — current nonce via GET /nonce/{address}
- All: `--node <url>` flag to override REST endpoint (default http://localhost:9933)

### Amount parsing:
- User enters MONEX (decimal, e.g. \"1000.50\")
- Convert to MOXX: `amount_moxx = amount_monex * 10^32`
- Parse via `U256::from_dec_str()` with decimal point handling
- Display: MOXX → MONEX with 2 decimal places, comma-separated

### Output format:
- Tx submission: `Tx submitted: 0x{tx_hash}` (with `--wait` flag: poll until finalized)
- Validator query: address, status, stake (MONEX), registration era, frozen status
- Validators list: table sorted by active first (stake desc), then candidates (stake desc)

### Tests:
- CLI unit: parse args → correct TxBody for all 4 staking commands
- CLI unit: MONEX→MOXX conversion edge cases (0, decimals, large)
- CLI unit: error on missing --key, invalid address, invalid amount
- CLI e2e: register → stake → query validator status
- CLI e2e: unstake → verify cooldown

---

## Sub-phase 2.9 — Crash Recovery + State Checkpoints

**From:** Architecture.md §Crash Recovery, Storage.md §Checkpoints, Network.md §CheckpointRequest/Response

Automatic crash recovery on restart. State checkpoints at era boundaries for fast sync. Checkpoint serving via sync protocol.

### Crash recovery:
- On restart: read `current_height` from META table → load latest block → rebuild SMT from ACCOUNTS table → compare root with `global_state_root`
- If match → resume from `current_height + 1`; if mismatch → panic (redb ACID guarantee)
- After recovery: if `current_height < peer_tip` → enter sync mode (Phase 2.2)

### Checkpoint types (`storage/checkpoint.rs`):
- `CheckpointMeta { era: u64, height: u64, global_state_root: [u8;32], timestamp: u64, num_shards: u16, generation: u64 }`
- `ShardSnapshot { accounts: Vec<(Vec<u8>, Vec<u8>)>, validators: Vec<(Vec<u8>, Vec<u8>)>, meta: Vec<(Vec<u8>, Vec<u8>)> }`
- Both SCALE-encoded, stored in `CHECKPOINTS` redb table

### Checkpoint production (era boundary, background task):
- Spawn tokio task: snapshot all state tables → build ShardSnapshot → store checkpoint_meta + checkpoint_data
- Does NOT block block production (next slot starts immediately)
- If previous checkpoint still in progress → cancel via CancellationToken, start new (generation++)
- Readers always see last completed checkpoint (redb MVCC)

### Retention:
- Full mode (default): keep latest 2 — overwrite N-2 when writing N
- Compact mode: skip all checkpoint production
- Archive mode: retain all (opt-in)
- Config: `storage.mode` ∈ {full, compact, archive} — already in NodeConfig, wire into behavior

### Checkpoint serving (sync protocol):
- `CheckpointRequest { target_height }` / `CheckpointResponse { height, smt_nodes, validator_set, validator_set_hash, checkpoint_block_header, checkpoint_hash }`
- All SCALE-encoded, registered as `/mononium/checkpoint-sync/1.0` Request-Response
- Trust model: verify BFT commits against validator_set → rebuild SMT → compare global_state_root

### Tests:
- Checkpoint write/read roundtrip
- Retention: full keeps 2, compact skips, archive keeps all
- Crash recovery: simulate crash → restart → state consistent
- Checkpoint produced at era boundary, readable via sync protocol

---

## Sub-phase 2.10 — Benchmarks + In-Process Test Harness

**From:** Testing.md §Test Tiers 2-3 §Benchmarks §Target Metrics, Roadmap.md (100 tx/s target)

In-process multi-validator test harness. criterion benchmarks. Integration tests for all Phase 2 features. 100 tx/s target.

### In-process test harness (`tests/harness.rs`):
- `ClusterBuilder{ validators: Vec<(String, KeyPair)>, genesis_path, block_time }` + builder methods
- `Cluster::start()` — spawn validators with in-memory redb + loopback libp2p + fast block time (500ms)
- `Cluster::run_until(blocks) -> Result` — poll until all validators reach height, timeout = blocks * block_time * 2
- `Cluster::stop()` — graceful shutdown
- `ValidatorHandle{ state, storage, p2p, consensus, mempool, key }` via `cluster.validator(name)`
- `cluster.all_agree() -> bool` — same height + same state root across all validators

### Integration tests:
- `basic_transfer.rs` — submit tx → mempool → block → state updated
- `multi_validator.rs` — 3 validators produce 20 blocks, all agree on canonical chain, >2/3 BFT commits per block
- `era_transition.rs` — bootstrap → era 0 Open → era 1 Top-N, frozen validator excluded
- `slashing_scenarios.rs` — equivocation → slash → freeze → fork resolution → thaw
- `governance_flow.rs` — propose → vote → tally → execute param change
- `staking_scenarios.rs` — register, stake, cross-stake, unstake cooldown

### Benchmarks (criterion):
- `crypto.rs`: Falcon sign (<10ms), verify (<5ms), batch verify 10 (<20ms), SMT insert 1000 (<50ms), SMT root (<10ms)
- `state.rs`: Block apply 100 txs (<200ms), 500 txs (<1s), mempool insert 10000 (<50ms), mempool select 500
- `e2e.rs`: 3-validator cluster 100 blocks (>50 tx/s), 500 blocks (>100 tx/s stretch goal), consensus overhead
- `cargo bench -p mononium-lib` runs all; baseline comparison via `--baseline phase1`

### Coverage targets:
- All new `core/` state machine paths: ≥ 95% region
- `network/` module: ≥ 85% region
- `governance/` module: ≥ 95% region
- `consensus/finality.rs`, `consensus/slashing.rs`: ≥ 95% region
- `rpc/` module: ≥ 85% region
- CLI: 0 clippy warnings, ≥ 60% pure-function coverage
- Total lib tests: ≥ 450

---

## Sub-phase 2.11 — User Docs + Devnet Deployment

**From:** UserDocs.md, Roadmap.md (Phase 2 goal)

### User docs:
- `user_docs/README.md` — index + quick start (clone → configure → run)
- `user_docs/Devnet.md` — deployment guide:
  - Hardware requirements per tier
  - Bootstrap key generation
  - Genesis configuration template
  - Docker compose multi-validator
  - Monitoring: Prometheus + Grafana

### Docker deployment:
- `Dockerfile` — multi-stage build
- `docker/docker-compose.yml` — bootstrap + 3 validators + RPC observer
- `docker/grafana/dashboard.json`
- `docker/prometheus/prometheus.yml`

### Phase 2 exit criteria:
- [ ] `cargo build -p mononium-lib` passes
- [ ] `cargo build -p mononium-cli` passes
- [ ] `cargo nextest run -p mononium-lib` passes (≥ 450 tests)
- [ ] `cargo bench -p mononium-lib` runs all suites
- [ ] 3-validator in-process cluster produces 20 blocks with BFT finality, all validators agree
- [ ] `mononium-cli wallet register/stake/unstake` creates signed txs, validators process them
- [ ] P2P sync: node A at height 200, node B starts from genesis → B catches up to A, state roots match
- [ ] Slashing: equivocation evidence submitted → validator frozen → fork resolved by heaviest chain
- [ ] Governance: propose → vote (meets quorum) → era boundary tally → param change executes at next era boundary
- [ ] Docker compose: `docker compose up -d --scale validator=3` produces blocks with consensus
- [ ] Crash recovery: kill validator → restart → resumes from last verified height, state consistent
- [ ] Coverage: ≥ 90% region on all Phase 2 new modules (network, governance, consensus/finality, consensus/slashing)
- [ ] `cargo clippy -p mononium-lib` passes (0 warnings)