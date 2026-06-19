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
- [x] **#1: Sharding gap in Protocol.md** — Fixed. `BlockHeader` now uses `global_state_root` matching StateSharding.md. Protocol.md updated V0.7.0.
- [x] **#6: Fee distribution rounding tie-breaker** — Fixed. Remainder goes to lowest address among highest-stake validators. Fees.md updated V0.7.0.
- [x] **#8: Anti-spam deposit return timing** — Fixed. Deposits return at next era boundary after the block. Block 719 → returns at 720. Block 720 → returns at 1440. Fees.md updated V0.7.0.
- [x] **#10: Nonce buffering + fee priority interaction** — Fixed. Block producer includes buffered lower nonces alongside higher-nonce txs to satisfy nonce gaps. Nonce order enforced within block, fee priority across blocks. Mempool.md updated V0.7.0.

## Validators & Consensus

- [x] **ValidtorId type** — `[u8; 32]` (address) everywhere. Header proposer, commit votes, evidence, transactions all use the full address. No index-based resolution needed. Settled V0.4.0.
- [x] **Clock drift tolerance** — ±2s. Unix timestamp seconds, validated locally, rejected blocks treated as missed slots. Settled V0.4.0.
- [x] **Fork handling** — no explicit fork-choice rule in V1. Follow proposer schedule + heaviest-by-stake-weight for ambiguous chains. Equivocators lose 90% → honest chain becomes heavier. Settled.
- [x] **CommitVote timing** — validators vote immediately after verification. Next proposer collects via gossip. Settled V0.4.0.
- [x] **Missed slot penalty destination** — 0.08 MONEX per missed slot sent to 0x00..01 (Cap-Refill), not burned. Applied at era boundary. Settled V0.6.0.
- [x] **Bootstrap key** — switched from single key to multi-key genesis config. Genesis `bootstrap` field accepts multiple public keys; any key in the list can propose during bootstrap phase. Settled V0.6.0.
- [x] **#2: Key file unlock stalls cheap VPS** — Fixed. Default memory reduced to 256 MiB, iterations increased to 16 (4x). Added `--unlock-timeout` flag (default 20s), configurable `crypto.argon2_memory_mib` / `crypto.argon2_iterations` (config-only, no CLI). Cryptography.md, NodeConfig.md, Architecture.md, README.md updated V0.7.0.
- [x] **#5: "2/3+" ambiguity** — Fixed. Standardized: `> 2/3` for finality (strictly greater), `≥ 2/3` for governance quorum. Consensus.md, Governance.md, StateSharding.md, Network.md updated V0.7.0.
- [x] **#7: Fork-choice rule clarity** — Fixed. Added explicit Fork-Choice Rule section to Consensus.md — heaviest chain by total stake backing is the V1 rule. Consensus.md updated V0.7.0.
- [x] **#12: Mainnet bootstrap chicken-and-egg** — Resolved. Era 0 is the free onboarding window (no stake required). Anyone who registers during era 0 earns rewards and accumulates stake for era 1+. Latecomers after era 0 need external MONEX (standard fair-launch model). Validators.md updated V0.7.0.

## Governance

- [x] **#3: Shard migration mechanics underspecified** — Fixed. Added pre-computation steps, shard discovery (Identify + `shards` metadata field), and catch-up protocol for offline validators (checkpoint-style SMT snapshot sync). StateSharding.md updated V0.7.0.
- [x] **#4: Governance parameter locking underdefined** — Fixed. Lock release conditions documented: resolution (any outcome), cancellation. Per-parameter locking. Governance.md updated V0.7.0.
- [x] **#11: Governance proposal rate limit** — Fixed. Set to 50 proposals/era (not 100 as initially spec'd). Governance.md updated V0.7.0.
- [x] **#18: Parameter bounds in governance** — Fixed. Added parameter bounds table (min/max for all mutable params). Governance.md updated V0.7.0.

## Networking & Sync

- [x] **Batch hash (rolling chain continuity)** — rolling BLAKE3 over batch blocks in `BlockSyncResponse` for fast fork detection before full verification. Per-batch, resets on each request. Doc'd in ADR-018. Settled V0.6.0.
- [x] **Mid-sync peer disconnection** — no sessions needed. Chain's parent_hash is the continuity mechanism. Sync cursor persisted locally (last_verified_height, last_verified_hash). On disconnect, discard incomplete batch, re-request with known_block_hash as fork anchor. Documented in Network.md. Settled V0.6.0.
- [x] **Checkpoint-era validator set delivery** — `CheckpointResponse` includes full `validator_set: Vec<ValidatorEntry>` so the syncing node can verify BFT commits without replay. Authenticated via `validator_set_hash`. Documented in Network.md. Settled V0.6.0.
- [x] **Sync stall / no available peers** — permanent retry: 5s → 10s → 30s → 30s (repeat). Node never gives up. Critical error logged after 5 min of zero connections but retry continues. Documented in Network.md. Settled V0.6.0.
- [x] **#1: Long-range checkpoint corruption** — Fixed. Progressive fallback: older checkpoint → `--trusted-checkpoint` CLI override → full genesis replay (last resort). Network.md + Storage.md updated V0.7.0.
- [x] **Gossip message limits** — per-topic size caps (txs: 1MB, blocks: 500KB, votes: 1KB, evidence: 5KB) + per-peer rate limits (txs: 20/s, blocks: 1/s, votes: 100/s, evidence: 5/s). Score penalties for violations. Documented in Network.md. Settled V0.6.0.
- [x] **Peer scoring** — 3-tier: Good (>0), Neutral (-20 to 0), Banned (< -20). Negatives doubled from original proposal. Ban = disconnect + 1-era banlist wipe. Era-based expiry (fallback: 1hr wall clock for fresh genesis). Documented in Network.md. Settled V0.6.0.
- [x] **#9: Peer scoring reset at era boundary** — Fixed. Bans now last 720 blocks from infraction height (not era-bound). Scores persist across bans. Network.md updated V0.7.0.
- [x] **#17: Checkpoint trust model clarity** — Fixed. Network.md now explicitly says votes are delivered alongside the checkpoint response, not stored in the header. Network.md updated V0.7.0.
- [x] **#22: Devnet deployment specifications** — Decided. Created `UserDocs.md` in V0.7.0 specifying user-doc requirements: hardware, bootstrap keys, genesis config, docker compose, monitoring. Implementation in Phase 1 alongside CLI and node config.

## Storage

- [x] **State pruning** — resolved via storage modes: full (default, everything forever), compact (opt-in, 2 eras full → headers only + proxy). Designed in NodeConfig.md, implementation deferred to Phase 3+.
- [x] **Checkpoint format** — full state snapshot at every era boundary (720 blocks). checkpoints_meta + checkpoint_data tables in chain.redb. Latest 2 retained (full), all (archive), none (compact). Hybrid P2P+HTTP serving protocol. Threshold: >2 eras triggers checkpoint sync. Verification: rebuild SMT, match state_root against block header. ~400 MB/era at 10M accounts. Documented in Storage.md.
- [x] **#15: redb write amplification and mmap concerns** — Fixed. Added "Known Redb Considerations" section covering write amplification, mmap crash safety, and memory pressure. Storage.md updated V0.7.0.

## CLI & Config

- [x] **Config file schema** — resolved. YAML + TOML, ~/.mononium/config, schema in NodeConfig.md
- [x] **Docker setup** — Settled. Base image: `istio/distroless` (~600 KB). Compose: explicit per-validator services (not `--scale`). Documented in `user_docs/Devnet.md` (Phase 1).
- [x] **CLI flag inventory** — documented in NodeConfig.md. --genesis, --key, --key-file, --observer, --p2p-port, --rpc-port, --rest-port, --bootnodes, --data-dir, --log-level, --log-format, --config
- [x] **#16: YAML vs TOML tie-break** — Fixed. Now errors if both exist (Option A). NodeConfig.md updated V0.7.0.
- [x] **#19: RPC server startup Phase 1/2 ambiguity** — Fixed. Startup step 15 now says axum (REST) in Phase 1, jsonrpsee added in Phase 2. Architecture.md updated V0.7.0.
- [x] **#28: logfmt command missing from CLI tree** — Fixed. Added `logfmt` to `mononium-cli` subcommand tree. Architecture.md updated V0.7.0.

## CI & Tooling

- [x] **CI pipeline** — GitHub Actions. `cargo-nextest`, `cargo-deny`, `cargo-clippy`, `cargo-fmt --check`. Clippy: normal mode (warnings as warnings), `clippy.toml` for bad-practices config. `deny.toml` created with allowlist. Benchmark CI threshold TBD.
- [x] **deny.toml** — Created with license allowlist (MIT, Apache-2.0, BSD, Zlib, ISC, Unicode-3.0, CC0-1.0), advisory bans, copyleft check.
- [ ] **Benchmark CI** — track criterion results across PRs. Regression threshold: 5% on critical paths (Falcon verify, block apply). TBD by operator.

## Workspace & Build

- [x] **Cargo workspace conversion** — Done. Virtual workspace at root, crate dirs under `src/mononium-rust-lib/` and `src/mononium-cli/`. Shared `[workspace.dependencies]` for libp2p, tokio, serde, etc.
- [x] **Crypto crate selection** — Zcash `falcon` (pure Rust), `primitive-types` for U256, `argon2` for KDF, `chacha20poly1305` for key encryption. Doc'd in ADR-019. Settled V0.6.0.
- [x] **mononium-rust-lib Cargo.toml** — Created with full dependency list from ADR-019 + workspace shared deps.
- [x] **mononium-cli Cargo.toml** — Created with clap, anyhow, tracing-subscriber + mononium-rust-lib path dep.
- [x] **#23: GUI skeleton** — Deferred to Phase 3 as planned. No Phase 2 skeleton.
- [x] **#24: Falcon crate security audit reference** — Fixed. Added GitHub reference to Zcash `falcon` crate audit status. ADR-019 updated V0.7.0.
- [x] **#26: max_validators for mainnet** — Fixed. Mainnet genesis spec now includes `max_validators: 101`. Genesis.md updated V0.7.0.

## Economics

- [x] **Anti-spam deposit** — 0.33 MONEX per tx (reduced from 1 MONEX), held from sender's balance until era boundary. Auto-returned (no reclaim tx). Scales with volume. Settled V0.6.0.
- [x] **Fee burning mechanics** — burn tx to 0x00..00 or 0x00..01 at 10 MOXX flat fee. Normal fees unchanged (pro-rata by stake distribution).
- [x] **Validator rewards distribution** — pro-rata by stake per block. Settled V0.4.0.
- [x] **Burn tx fee discount** — 10 MOXX flat for Burn and WithdrawTxDeposit types (bypasses standard fee calculation).
- [x] **Per-account rate limit** — 50 txs/account/block, local mempool policy.
- [x] **Block hard cap** — 500 txs OR 1 MB.
- [x] **Inflation curve** — `min(5% × headroom, 3.5% × effective_max)` per era. Flat (~55.5/block) until 30% minted, then decays smoothly. No supply cliff. Settled V0.6.0.
- [x] **#13: Economic model validation** — Fixed. Added Economic Security table covering 6 attack vectors with defenses. Fees.md updated V0.7.0.
- [x] **#20: Wallet backup/recovery** — Fixed. Added mnemonic backup section (BIP39, 24 words) with recovery flow. Cryptography.md updated V0.7.0.
- [x] **#25: Transaction size estimates** — Verified consistent with docs (~125 TPS at 800 B/tx). No change needed.

## Light Client & RPC

- [x] **#14: Light client** — Stub planning doc created (`V0.7.0/LightClient.md`). Existing affordances documented (SMT prove, tx_root, checkpoints). Open questions deferred to Phase 2 planning cycle.
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

## Archive Node Incentives (Late V1)

- [x] **Archive node incentive** — Virtual stake bonus: `effective_stake = real_stake × (1 + archive_bonus)`. Affects fee distribution and governance voting weight only. Not counted toward Top-N election, minimum stake, or slashing.
- [x] **Self-declared flag** — Node sets `archive: true` in config. False-claim slashing: penalty = `bonus_collected × 1.5`, node unflagged, re-declaration lockout for governance-set eras.
- [x] **Timeout penalty only if online** — An offline node (missed slots) cannot be penalized for failing to serve archive data. The archive flag is suspended while offline.
- [ ] **Archive bonus rate** — Governance-mutable parameter (e.g., 5%). Set when governance ships (late V1).
