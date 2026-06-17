---
tags: [testing, quality]
---

# Testing

Every invariant discovered during design or development **must** be tested and documented. No exception.

## Test Structure

Tests live in `src/tests/` mirroring the `src/` tree:

```
mononium-rust-lib/src/
├── core/
│   ├── mod.rs
│   ├── account.rs
│   ├── transaction.rs
│   ├── block.rs
│   ├── state.rs
│   └── fee.rs
├── crypto/
│   ├── falcon.rs
│   ├── hash.rs
│   ├── trie.rs
│   └── address.rs
├── consensus/
│   ├── election.rs
│   ├── proposer.rs
│   ├── era.rs
│   ├── finality.rs
│   ├── slashing.rs
│   └── supply.rs
├── mempool/
│   ├── mod.rs
│   └── ordering.rs
├── storage/
│   ├── redb.rs
│   ├── tables.rs
│   └── genesis.rs
├── network/
│   ├── topics.rs
│   ├── discovery.rs
│   └── messages.rs
├── rpc/
│   └── types.rs
│
└── tests/                         ← mirrors src/ structure
    ├── mod.rs                     # shared test helpers, TestStateMachine
    ├── harness.rs                 # in-process multi-validator test harness
    ├── core/
    │   ├── mod.rs
    │   ├── transaction.rs         # tx serialization, validation
    │   ├── block.rs               # block structure, hashing
    │   ├── state.rs               # state transition determinism
    │   └── fee.rs                 # fee calculation
    ├── crypto/
    │   ├── mod.rs
    │   ├── falcon.rs              # sign/verify round-trip
    │   ├── trie.rs                # SMT insert/get/proof determinism
    │   └── address.rs             # address format, checksum validation
    ├── consensus/
    │   ├── mod.rs
    │   ├── election.rs            # Top-N election, tie-breaking
    │   ├── proposer.rs            # round-robin scheduling
    │   ├── era.rs                 # era boundary transitions
    │   ├── finality.rs            # BFT commit counting
    │   ├── slashing.rs            # equivocation detection, evidence
    │   └── supply.rs              # fixed supply, block rewards
    ├── mempool/
    │   ├── mod.rs                 # insertion, eviction, TTL
    │   └── ordering.rs            # tip → time → nonce priority
    ├── storage/
    │   ├── mod.rs                 # StorageEngine trait contract
    │   ├── genesis.rs             # genesis loading, duplicate detection
    │   └── integration.rs         # state + storage round-trip
    ├── network/
    │   └── messages.rs            # wire format encode/decode symmetry
    └── integration/
        ├── basic_transfer.rs      # full flow: keygen → tx → block → state
        ├── multi_validator.rs     # 3 validators, consensus finality
        ├── era_transition.rs      # validator set change at era boundary
        └── slashing_scenarios.rs  # equivocation, evidence, penalty
```

All test files use `#[cfg(test)]` and are compiled only during testing — zero bloat in release builds.

## Test Tiers

### Tier 1: Unit Tests

- Every public function has tests for happy path + error cases
- Pure logic (no I/O, no networking)
- Run on every `cargo test`
- Use `quickcheck` or `proptest` for property-based coverage where applicable

**Invariants that MUST be tested:**

| Invariant                                               | Location                      |
| ------------------------------------------------------- | ----------------------------- |
| Tx SCALE encode/decode is symmetric                     | `tests/core/transaction.rs`   |
| Tx JSON serde matches SCALE round-trip                  | `tests/core/transaction.rs`   |
| Block header hash depends on all fields                 | `tests/core/block.rs`         |
| SMT insert → get returns same value                     | `tests/crypto/trie.rs`        |
| SMT root is deterministc for same inserts               | `tests/crypto/trie.rs`        |
| SMT proof verify(prove(key)) == true                    | `tests/crypto/trie.rs`        |
| Falcon-512 sign → verify round-trip                     | `tests/crypto/falcon.rs`      |
| Fee calculation is deterministic                        | `tests/core/fee.rs`           |
| Mempool ordering: tip desc, time asc, nonce asc         | `tests/mempool/ordering.rs`   |
| Mempool TTL eviction removes stale txs                  | `tests/mempool/mod.rs`        |
| Top-N election sorts by stake desc, tie by registration | `tests/consensus/election.rs` |
| Equivocation detection catches duplicate blocks         | `tests/consensus/slashing.rs` |

### Tier 2: State Machine Integration

Drives the state machine deterministically — no networking, no clocks:

- `TestStateMachine` helper in `tests/harness.rs`
- Constructs blocks programmatically
- Verifies SMT root after each block
- Tests era transitions, validator set changes
- Tests slashing evidence submission and penalty enforcement

### Tier 3: Multi-Validator (In-Process)

The **test harness** (`tests/harness.rs`) spawns multiple validators in-process:

```rust
// Single machine, 3 validators, real consensus
let mut cluster = Cluster::builder()
    .with_validator("alice", key_alice)
    .with_validator("bob", key_bob)
    .with_validator("charlie", key_charlie)
    .with_genesis(genesis_devnet)
    .build();

cluster.start();          // all 3 start producing + voting
cluster.run_until(blocks: 10);  // let them produce 10 blocks
cluster.stop();

// Verify
assert_eq!(cluster.state("alice").height(), 10);
assert_eq!(cluster.state("alice").state_root(), cluster.state("bob").state_root());
```

Uses in-memory redb (no disk I/O), loopback networking (no real TCP). Fast enough to run in CI.

### Tier 4: Docker (Manual)

Docker compose with multiple containers, real libp2p networking, real Falcon-512 keys. Used for:

- Manual testing during development
- Network behavior under latency
- Crash recovery scenarios
- Performance benchmarking

Not run in CI (slow, resource-intensive).

### Tier 5: Fuzzing (Deferred to Phase 4)

`cargo-fuzz` with libFuzzer for:

- Malformed SCALE-encoded blocks
- Edge case transaction sequences
- SMT pathological inputs

## Invariant Documentation

When an invariant is discovered (during design, code review, or debugging):

1. **Add a test** in the corresponding `src/tests/` file
2. **Document the invariant** in this doc's invariants table
3. **Add a comment** in the production code explaining why the invariant matters

Example:

```rust
// src/core/state.rs
//
// INVARIANT: A block's state_root must match the SMT root after applying all
// transactions in order, before commit votes are processed. Commit votes do
// NOT affect the state root.
// Tested in: tests/core/state.rs::state_root_excludes_votes
```

## Running Tests

```bash
# All tests
cargo nextest run -p mononium-rust-lib

# Single module
cargo nextest run -p mononium-rust-lib --test trie

# With proptest (more iterations = more confidence)
PROPTEST_CASES=10000 cargo nextest run -p mononium-rust-lib

# Ensure no warnings in tests (tests must compile clean)
cargo clippy -p mononium-rust-lib --tests -- -D warnings
```

## Benchmarks

All benchmarks use **criterion** and live in `mononium-rust-lib/benches/`.

### Bench Suite

```
mononium-rust-lib/benches/
├── crypto.rs          # Falcon sign/verify, BLAKE3, SMT operations
├── state.rs           # Block apply, tx processing, mempool
└── e2e.rs             # In-process multi-validator cluster throughput
```

### Target Metrics

| Metric                                 | Suite  | Phase 2 Target | Method            |
| -------------------------------------- | ------ | -------------- | ----------------- |
| Falcon sign                            | crypto | <10ms          | `criterion`       |
| Falcon verify                          | crypto | <5ms           | `criterion`       |
| Falcon batch verify (10)               | crypto | <20ms          | `criterion`       |
| SMT insert 1000 accounts               | crypto | <50ms          | `criterion`       |
| SMT root after 1000 inserts            | crypto | <10ms          | `criterion`       |
| Block apply 100 tx (all Falcon verify) | state  | <200ms         | `criterion`       |
| Block apply 500 tx                     | state  | <1s            | `criterion`       |
| Mempool insert 10000                   | state  | <50ms          | `criterion`       |
| E2E 3-validator, 100 blocks            | e2e    | >50 tx/s       | `cluster harness` |

### Running

```bash
# All benchmarks
cargo bench -p mononium-rust-lib

# Specific suite
cargo bench -p mononium-rust-lib -- crypto

# Compare against baseline (after making changes)
cargo bench -p mononium-rust-lib -- --baseline main
cargo bench -p mononium-rust-lib -- --baseline my-branch
```

Benchmark results are tracked: regressions in critical paths (Falcon verify, block apply) are CI-failures after Phase 2.

---

**Related:** [Architecture](plans/V0.4.0/Architecture.md), [Roadmap](plans/V0.4.0/Roadmap.md)
