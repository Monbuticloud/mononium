# Grill Topics — Future Sessions

Open questions and decisions deferred for upcoming grilling sessions.

## Protocol & Encoding

- [ ] **SCALE encoding spec** — exact field ordering, sizes, and encoding for all wire types: Transaction, Block, BlockHeader, CommitVote, EquivocationEvidence, sync messages
- [ ] **Block format** — exact SCALE struct layout, what goes in the body vs header
- [ ] **Transaction format** — exact field types, encoding order, payload constraints
- [ ] **Failed tx handling** — skip failed txs within a block or invalidate the whole block?
- [ ] **Fee burning on mainnet** — burn a portion of fees (now) or let proposers keep 100% (deferred to V2)?

## Validators & Consensus

- [ ] **ValidtorId type** — how does `proposer_index: u16` in the block header resolve to a full public key? Lookup by era?
- [ ] **Clock drift tolerance** — validators need to agree on when a slot starts. What tolerance before a block is rejected as "too early" or "too late"? NTP dependency?
- [ ] **Fork handling** — beyond equivocation (BFT resolves). What if 2/3+ validators commit two different chains (network partition)? Social consensus or on-chain fork-choice rule?
- [ ] **CommitVote timing** — validators vote immediately after verifying, or wait until a specific point in the slot? How are commit votes gossiped and aggregated?

## Networking & Sync

- [ ] **Sync edge cases** — what happens when peers disagree on the canonical chain? Peer disconnection/reconnection during sync?
- [ ] **Gossip message limits** — max message size per topic? Rate limiting per peer?
- [ ] **Peer scoring** — punish bad peers (invalid blocks, spam, no-show)?

## Storage

- [ ] **State pruning** — permanent vs prunable table design (flagged for Phase 4 but designed from V1). What gets pruned, when, how?
- [ ] **Checkpoint format** — exact schema for 7-day state checkpoints in redb. Size estimate? Verification chain?

## CLI & Config

- [ ] **Config file schema** — `config.yaml` fields, override precedence (defaults < file < CLI flags)
- [ ] **Docker setup** — multi-validator compose file, image sizes (scratch vs distroless), docker-compose for localnet/devnet
- [ ] **CLI flag inventory** — complete list of `mononium-cli node` flags (p2p-port, rpc-port, rest-port, genesis, key, key-file, bootnodes, data-dir, config, etc.)

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

- [ ] **Fee burning mechanics** — if introduced, what portion? Burn address or new mechanism?
- [ ] **Validator rewards distribution** — fees split among all active validators, or go entirely to the proposer? (Currently: proposer keeps 100%. Confirm or reconsider.)
- [ ] **Inflation curve later** — does 3.5% stay constant until the cap or decay over time?

## Future Phases (Noted, Not Urgent)

- [ ] Governance / upgrade mechanism (Phase 4)
- [ ] Phragmén NPoS (V2+)
- [ ] VRF leader election (V2+)
- [ ] GRANDPA finality gadget (V2+)
- [ ] Treasury / dev fund from inflation (V2+)
- [ ] Smart contracts
- [ ] Sharding
