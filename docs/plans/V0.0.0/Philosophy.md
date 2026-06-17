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

Block time is fixed at 5s. Finality at 20s (4 blocks). Throughput emerges from block size + block time + tx format — it's not a fixed number. Target range is 100–10,000 TPS, with the actual ceiling determined by validator hardware constraints.

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
| Ed25519              | Speed, well-studied  | Falcon (post-quantum)    |
| ITTIA DB Lite        | Determinism, low RAM | General-purpose DBs      |
| No sharding V1       | Complexity avoidance | Scale                    |
| CLI first, GUI later | Focus                | Parallel delivery        |
| Fixed 5s block       | Predictability       | Variable timing          |

## Non-Goals for V1

- Smart contracts
- Sharding
- Cross-chain interoperability
- Governance
- Wallet UI / block explorer
- Mobile support
- Post-quantum security

---

**Next:** [Architecture](plans/V0.0.0/Architecture.md)
