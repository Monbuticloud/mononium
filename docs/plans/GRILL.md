# Grill Topics — Future Sessions

Open questions and decisions deferred for upcoming grilling sessions.

## Protocol & Encoding

- [x] **SCALE encoding spec** — settled. Transaction (common envelope + TxBody enum), Block (header + body, votes separate DB), CommitVote struct. Rust enums over flat structs.
- [x] **Block format** — header + body. tx_root = BLAKE3 Merkle over tx hashes. Votes in DB, not block.
- [x] **Transaction format** — common fields in struct, type-specific fields in TxBody enum. Falcon-512 sig over envelope.
- [x] **Burn tx type** — added to V1 tx set. Burn sends to 0x00..00 or 0x00..01, 10 MOXX flat fee. No WithdrawTxDeposit needed — deposits auto-return at era boundary.
- [x] **Failed tx handling** — skip-and-continue, fee still paid to proposer. Settled in V0.4.0 session.
- [x] **Fee burning on mainnet** — deferred. Fees go to validators pro-rata by stake; no burning in V1.
- [x] **Validator rewards distribution** — pro-rata by stake, per-block, all active validators. Settled V0.4.0.

## Validators & Consensus

- [x] **ValidtorId type** — `[u8; 32]` (address) everywhere. Header proposer, commit votes, evidence, transactions all use the full address. No index-based resolution needed. Settled V0.4.0.
- [x] **Clock drift tolerance** — ±2s. Unix timestamp seconds, validated locally, rejected blocks treated as missed slots. Settled V0.4.0.
- [x] **Fork handling** — no explicit fork-choice rule in V1. Follow proposer schedule + heaviest-by-stake-weight for ambiguous chains. Equivocators lose 90% → honest chain becomes heavier. Settled.
- [x] **CommitVote timing** — validators vote immediately after verification. Next proposer collects via gossip. Settled V0.4.0.

## Networking & Sync

- [ ] **Sync edge cases** — what happens when peers disagree on the canonical chain? Peer disconnection/reconnection during sync?
- [ ] **Gossip message limits** — max message size per topic? Rate limiting per peer?
- [ ] **Peer scoring** — punish bad peers (invalid blocks, spam, no-show)?

## Storage

- [x] **State pruning** — resolved via storage modes: full (default, everything forever), compact (opt-in, 2 eras full → headers only + proxy). Designed in NodeConfig.md, implementation deferred to Phase 3+.
- [x] **Checkpoint format** — full state snapshot at every era boundary (720 blocks). checkpoints_meta + checkpoint_data tables in chain.redb. Latest 2 retained (full), all (archive), none (compact). Hybrid P2P+HTTP serving protocol. Threshold: >2 eras triggers checkpoint sync. Verification: rebuild SMT, match state_root against block header. ~400 MB/era at 10M accounts. Documented in Storage.md.

## CLI & Config

- [x] **Config file schema** — resolved. YAML + TOML, ~/.mononium/config, schema in NodeConfig.md
- [ ] **Docker setup** — multi-validator compose file, image sizes (scratch vs distroless), docker-compose for localnet/devnet
- [x] **CLI flag inventory** — documented in NodeConfig.md. --genesis, --key, --key-file, --observer, --p2p-port, --rpc-port, --rest-port, --bootnodes, --data-dir, --log-level, --log-format, --config

## CI & Tooling

- [ ] **CI pipeline** — `cargo-nextest`, `cargo-deny`, `cargo-clippy`, `cargo-fmt` checks. Benchmark CI (compare against baseline, fail on regression)
- [ ] **deny.toml** — license allowlist (MIT, Apache-2.0, BSD, Zlib), advisory bans, copyleft check
- [ ] **Benchmark CI** — track criterion results across PRs, fail on >5% regression in critical paths (Falcon verify, block apply)

## Workspace & Build

- [ ] **Cargo workspace conversion** — root Cargo.toml → virtual workspace with shared `[workspace.dependencies]`. Version table for shared deps (libp2p, tokio, serde, etc.)
- [ ] **mononium-rust-lib Cargo.toml** — full dependency list with features
- [ ] **mononium-cli Cargo.toml** — CLI-only deps (clap, anyhow, tracing-subscriber)

## Light Client & RPC

- [ ] **Light client** — deferred to V2 but architecture might affect V1 RPC. What data must be kept available for light client SMT proofs?
- [ ] **RPC method inventory** — complete list of jsonrpsee methods + REST endpoints. Error types and codes.
- [ ] **WebSocket subscriptions** — which events are subscribable? New blocks, new txs, finality events?

## Economics

- [x] **Fee burning mechanics** — burn tx to 0x00..00 or 0x00..01 at 10 MOXX flat fee. Normal fees unchanged (pro-rata by stake distribution).
- [x] **Validator rewards distribution** — pro-rata by stake per block. Settled V0.4.0.
- [ ] **Inflation curve later** — does 3.5% stay constant until the cap or decay over time?
- [x] **Burn tx fee discount** — 10 MOXX flat for Burn and WithdrawTxDeposit types (bypasses standard fee calculation).
- [x] **Anti-spam deposit** — 1 MONEX per tx, held from sender's balance until era boundary. Auto-returned (no reclaim tx). Scales with volume.
- [x] **Per-account rate limit** — 50 txs/account/block, local mempool policy.
- [x] **Block hard cap** — 500 txs OR 1 MB.

## V2 (DeFi, developed late V1)

- [ ] **Native stableswap AMM** — built-in constant-product pools for MONEX trading
- [ ] **Bridge** — wrapped MONEX on Solana/Ethereum, or cross-chain messaging
- [ ] VRF leader election
- [ ] GRANDPA finality gadget
- [ ] Phragmén NPoS
- [ ] Treasury / dev fund from inflation
- [ ] Smart contracts
- [x] **State sharding** — hash prefix partitioning (`blake3(address)[0..2] % N` as u16), 2 shards genesis, gov-voted increase (2/3 quorum, 24-era grace, stake-weighted), full SMT per shard, snapshot sync on restart, cross-shard proofs pulled on demand from peers. Doc'd in StateSharding.md.

## V2 Governance

- [ ] Governance / upgrade mechanism
