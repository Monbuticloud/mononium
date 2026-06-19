---
tags: [design, principles]
---

# Philosophy

## Core Principles

### 1. Start Simple, Iterate

Don't overbuild V1. Ship a single-node prototype, then add validators, then networks. Features like smart contracts, sharding, and governance come later — or not at all if demand doesn't justify them.

### 2. Deterministic State Machine

State transitions are fully deterministic. Given the same block history, every node produces the same state. No nondeterminism, no ambiguity. This is non-negotiable.

### 3. Account-Based (Not UTXO)

UTXO was rejected for V1. Account model maps naturally to the mental model of "bank accounts". Simpler to implement, reason about, and build applications on. Balances are `U256` with 18 decimal places.

### 4. Performance Through Constraints

Block time is fixed at 5s. Finality at 20s (4 blocks). Throughput emerges from block size + block time + tx format — it's not a fixed number. Target range is 100–200 TPS (accounting for Falcon-512 signature sizes), with the actual ceiling determined by validator hardware constraints.

### 5. Cheap Validators First

Target hardware is a low-end VPS. Low CPU, low RAM, low bandwidth. The network should be accessible to run a node — not exclusive to data centers. Horizontal scaling (sharding) is a future option, not V1 requirement.

### 6. Native First, Smart Contracts Maybe

Native transfers and staking are the first-class features. Smart contracts are explicitly deferred — they add massive complexity, attack surface, and state bloat. If they come, it's only after the core chain is proven.

### 7. Rust

Rust for safety, performance, and ecosystem. No unsafe patterns unless absolutely necessary (and audited). The compiler is your first line of defense.

### 8. Resume-Worthy

This is a systems-level project spanning distributed systems, cryptography, databases, consensus, P2P networking, and performance engineering. Every component is designed to be a portfolio piece.

## Trade-Offs Made

| Decision             | Chose                | Over                     |
| -------------------- | -------------------- | ------------------------ |
| Account model        | Simplicity, UX       | UTXO's parallelizability |
| Falcon-512           | Post-quantum ready   | Ed25519 speed            |
| redb                 | Determinism, low RAM | General-purpose DBs      |
| No sharding V1       | Complexity avoidance | Sharding planned for late V1 |
| CLI first, GUI later | Focus                | Parallel delivery        |
| Fixed 5s block       | Predictability       | Variable timing          |

## Non-Goals for V1

- Smart contracts (V2+)
- Cross-chain interoperability (V2+)
- Wallet UI / block explorer (V2+)
- Mobile support (V2+)

### Late V1 (Phase 2+)

- **Governance** — on-chain stake-weighted voting, parameter mutation (spec in Governance.md)
- **Sharding** — state sharding with SMT per shard (spec in StateSharding.md)

Both are designed and spec'd during planning but implemented during late V1 development.

---

**Next:** [Architecture](plans/V0.7.0/Architecture.md)
