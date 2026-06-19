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
- [x] **Denomination** — 32 decimal places (10^32 MOXX per MONEX). Architecture.md updated. Settled V0.6.0.

## Validators & Consensus

- [x] **ValidtorId type** — `[u8; 32]` (address) everywhere. Header proposer, commit votes, evidence, transactions all use the full address. No index-based resolution needed. Settled V0.4.0.
- [x] **Clock drift tolerance** — ±2s. Unix timestamp seconds, validated locally, rejected blocks treated as missed slots. Settled V0.4.0.
- [x] **Fork handling** — no explicit fork-choice rule in V1. Follow proposer schedule + heaviest-by-stake-weight for ambiguous chains. Equivocators lose 90% → honest chain becomes heavier. Settled.
- [x] **CommitVote timing** — validators vote immediately after verification. Next proposer collects via gossip. Settled V0.4.0.
- [x] **Missed slot penalty destination** — 0.08 MONEX per missed slot sent to 0x00..01 (Cap-Refill), not burned. Applied at era boundary. Settled V0.6.0.
- [x] **Bootstrap key** — switched from single key to multi-key genesis config. Genesis `bootstrap` field accepts multiple public keys; any key in the list can propose during bootstrap phase. Settled V0.6.0.

## Networking & Sync

- [x] **Batch hash (rolling chain continuity)** — rolling BLAKE3 over batch blocks in `BlockSyncResponse` for fast fork detection before full verification. Per-batch, resets on each request. Doc'd in ADR-018. Settled V0.6.0.
- [x] **Mid-sync peer disconnection** — no sessions needed. Chain's parent_hash is the continuity mechanism. Sync cursor persisted locally (last_verified_height, last_verified_hash). On disconnect, discard incomplete batch, re-request with known_block_hash as fork anchor. Documented in Network.md. Settled V0.6.0.
- [x] **Checkpoint-era validator set delivery** — `CheckpointResponse` includes full `validator_set: Vec<ValidatorEntry>` so the syncing node can verify BFT commits without replay. Authenticated via `validator_set_hash`. Documented in Network.md. Settled V0.6.0.
- [x] **Sync stall / no available peers** — permanent retry: 5s → 10s → 30s → 30s (repeat). Node never gives up. Critical error logged after 5 min of zero connections but retry continues. Documented in Network.md. Settled V0.6.0.
- [ ] **Long-range checkpoint corruption** — checkpoint download succeeds, SMT rebuild fails. Node falls back to full replay from genesis? How far back does this go for an archive node?
- [x] **Gossip message limits** — per-topic size caps (txs: 1MB, blocks: 500KB, votes: 1KB, evidence: 5KB) + per-peer rate limits (txs: 20/s, blocks: 1/s, votes: 100/s, evidence: 5/s). Score penalties for violations. Documented in Network.md. Settled V0.6.0.
- [x] **Peer scoring** — 3-tier: Good (>0), Neutral (-20 to 0), Banned (< -20). Negatives doubled from original proposal. Ban = disconnect + 1-era banlist wipe. Era-based expiry (fallback: 1hr wall clock for fresh genesis). Documented in Network.md. Settled V0.6.0.

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
- [x] **Crypto crate selection** — Zcash `falcon` (pure Rust), `primitive-types` for U256, `argon2` for KDF, `chacha20poly1305` for key encryption. Doc'd in ADR-019. Settled V0.6.0.
- [ ] **mononium-rust-lib Cargo.toml** — full dependency list with features (non-crypto deps still open: libp2p, tokio, serde versions)
- [ ] **mononium-cli Cargo.toml** — CLI-only deps (clap, anyhow, tracing-subscriber)

## Economics

- [x] **Anti-spam deposit** — 0.33 MONEX per tx (reduced from 1 MONEX), held from sender's balance until era boundary. Auto-returned (no reclaim tx). Scales with volume. Settled V0.6.0.
- [x] **Fee burning mechanics** — burn tx to 0x00..00 or 0x00..01 at 10 MOXX flat fee. Normal fees unchanged (pro-rata by stake distribution).
- [x] **Validator rewards distribution** — pro-rata by stake per block. Settled V0.4.0.
- [x] **Burn tx fee discount** — 10 MOXX flat for Burn and WithdrawTxDeposit types (bypasses standard fee calculation).
- [x] **Per-account rate limit** — 50 txs/account/block, local mempool policy.
- [x] **Block hard cap** — 500 txs OR 1 MB.
- [x] **Inflation curve** — `min(5% × headroom, 3.5% × effective_max)` per era. Flat (~55.5/block) until 30% minted, then decays smoothly. No supply cliff. Settled V0.6.0.

## Light Client & RPC

- [ ] **Light client** — deferred to V2 but architecture might affect V1 RPC. What data must be kept available for light client SMT proofs?
- [x] **RPC method inventory** — 11 REST endpoints (Phase 1), 15 jsonrpsee methods + 3 subscriptions (Phase 2). Error codes: 0 to -7. Documented in Architecture.md. Settled V0.6.0.
- [x] **WebSocket subscriptions** — `subscribe_blocks`, `subscribe_finality`, `subscribe_votes`. Added Phase 2 alongside jsonrpsee. Settled V0.6.0.

## V2 (DeFi, developed late V1)

- [ ] **Native stableswap AMM** — built-in constant-product pools for MONEX trading
- [ ] **Bridge** — wrapped MONEX on Solana/Ethereum, or cross-chain messaging
- [ ] VRF leader election
- [ ] GRANDPA finality gadget
- [ ] Phragmén NPoS
- [ ] Treasury / dev fund from inflation
- [ ] Smart contracts
- [x] **State sharding** — hash prefix partitioning (`blake3(address)[0..2] % N` as u16), 2 shards genesis, gov-voted increase (2/3 quorum, 24-era grace, stake-weighted), full SMT per shard, snapshot sync on restart, cross-shard proofs pulled on demand from peers. Doc'd in StateSharding.md.

Governance moved to Phase 2 — [Governance.md](V0.6.0/Governance.md) covers the full design.
