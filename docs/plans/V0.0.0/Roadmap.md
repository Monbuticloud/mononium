---
tags: [planning, milestones]
---

# Roadmap

## Phases

```mermaid
gantt
    title Mononium Roadmap
    dateFormat  YYYY-MM-DD
    axisFormat  Q1

    section Phase 1
    Core lib + CLI prototype      :p1, 2026-06-17, 60d
    section Phase 2
    3-validator localnet          :p2, after p1, 45d
    section Phase 3
    Public devnet + GUI start     :p3, after p2, 45d
    section Phase 4
    Public testnet                :p4, after p3, 60d
    section Phase 5
    Mainnet + GUI complete        :p5, after p4, 60d
```

## Phase 1 — Workspace Setup + Local Prototype

Ship `mononium-rust-lib` + `mononium-cli` first. GUI starts later.

- [ ] Cargo workspace: `mononium-rust-lib` + `mononium-cli`
- [ ] `mononium-rust-lib`: account types, U256, state machine
- [ ] `mononium-rust-lib`: Ed25519 signing + BLAKE3 hashing
- [ ] `mononium-rust-lib`: ITTIA DB Lite (mutable + append-only)
- [ ] `mononium-rust-lib`: transaction types + serialization
- [ ] `mononium-rust-lib`: block structure + hashing
- [ ] `mononium-rust-lib`: basic mempool
- [ ] `mononium-cli`: node daemon (single-node mode)
- [ ] `mononium-cli`: wallet keygen, transfer command

**Goal:** `mononium-cli node` produces blocks locally. `mononium-cli wallet transfer` sends txs.

## Phase 2 — Localnet

- [ ] `mononium-rust-lib`: P2P networking + peer discovery
- [ ] `mononium-rust-lib`: PoS consensus engine
- [ ] `mononium-rust-lib`: staking transaction types
- [ ] `mononium-cli`: multi-validator node mode
- [ ] `mononium-cli`: stake/unstake commands
- [ ] Block propagation and consensus votes
- [ ] Crash recovery and snapshots
- [ ] Benchmarks: 100 tx/s target

**Goal:** 3+ validators on local machine or cheap VPS.

## Phase 3 — Devnet + GUI Begins

- [ ] Public devnet deployment
- [ ] Genesis file + chain ID management
- [ ] Seed/bootstrap nodes
- [ ] Sync / catch-up mechanism
- [ ] Benchmarks: 500 tx/s target
- [ ] **Add `mononium-gui` to workspace**
- [ ] `mononium-gui`: connect to node via RPC
- [ ] `mononium-gui`: wallet view (balance, send)

**Goal:** Open devnet + functional GUI wallet.

## Phase 4 — Testnet

- [ ] Public testnet
- [ ] State pruning
- [ ] Performance optimization
- [ ] Security review
- [ ] Bug bounty or focused testing
- [ ] Governance / upgrade mechanism (basic)
- [ ] `mononium-gui`: block explorer view
- [ ] `mononium-gui`: validator monitoring

**Goal:** Pre-production network with community validators + feature-rich GUI.

## Phase 5 — Mainnet

- [ ] Genesis distribution
- [ ] Mainnet launch
- [ ] Monitoring and alerting
- [ ] Documentation and guides
- [ ] `mononium-gui`: v1.0 release
- [ ] `mononium-cli`: all commands stable

**Goal:** Production.

## Workspace Progression

```
Phase 1:  mononium-rust-lib + mononium-cli   (lib + node)
Phase 2+: mononium-rust-lib + mononium-cli   (multi-node)
Phase 3+: mononium-rust-lib + mononium-cli + mononium-gui  (full stack)
```

## Benchmark Targets

```mermaid
graph LR
    Q1[100 tx/s] --> Q2[500 tx/s]
    Q2 --> Q3[1000 tx/s]
```

Measure throughput at each phase. Optimize only after measuring. Cap block size / gas instead of chasing a fixed TPS number.

## Key Principles

- **Measure before optimizing** — Don't tune what you haven't measured
- **Avoid overbuilding V1** — Sharding, cross-chain, governance are future concerns
- **Horizontal scaling later** — Shard/partition only when real demand exists
- **Start simple** — A running prototype is worth more than a perfect design

---

**Related:** [Philosophy](Philosophy.md), [Network](Network.md), [Validators](Validators.md), [Architecture](Architecture.md)
