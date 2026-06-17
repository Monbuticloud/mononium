---
tags: [consensus, network, staking]
---

# Validators

## Overview

Mononium uses **Proof of Stake (PoS)**. Validators stake **Monium (MONEX)** to participate in block production and consensus.

## Target Hardware

The network explicitly targets **cheap VPS** hardware:

| Resource  | Target                                        |
| --------- | --------------------------------------------- |
| CPU       | Low — 1-2 vCPU                                |
| RAM       | Low — fixed memory footprint                  |
| Bandwidth | Low — 500 KB blocks imply modest traffic      |
| Disk      | Minimal write amplification via ITTIA DB Lite |

The goal is accessibility — running a validator should not require data center infrastructure.

## Consensus Parameters

| Parameter         | Value                              |
| ----------------- | ---------------------------------- |
| Consensus         | PoS                                |
| Block time        | 5 seconds                          |
| Finality          | 20 seconds (4 blocks)              |
| Block size cap    | 500 KB                             |
| Throughput target | 100–10,000 TPS (emerges naturally) |

## Bottlenecks

Priority order of validator bottlenecks (from most to least constrained):

1. **Network traffic** — consensus messages, block propagation
2. **Signature verification** — Ed25519 batch verification
3. **State / database access** — ITTIA DB reads and writes
4. **Consensus overhead** — message handling, timeouts
5. **Hashing** — BLAKE3 is fast, not a concern

## Validator Election

### V1: Top-N by Stake

The simplest possible election: sort all staked candidates by stake, take the top N.

```
staked_validators.sort_by(|a, b| b.stake.cmp(&a.stake));
active_set = staked_validators.take(N);
```

- N is a protocol parameter (e.g., 21 for devnet, 101 for mainnet)
- Recalculated every era (e.g., every hour / 720 blocks)
- Ties broken by registration time (first-registered wins)

### Future: Phragmén NPoS (V2.0+)

Full Nominated Proof of Stake with optimal proportional representation:

- Nominators back validators with their stake
- Phragmén sequential election algorithm
- Handles delegation, vote splitting, and optimal representation
- Swappable via dependency injection (see [[V0.1.0/Architecture#Validator Election DI]])

## Validator Lifecycle

```
State: Inactive → Staked → Active → Unstaking → Inactive
```

- Stake MONEX to join the candidate pool
- Top N by stake become active
- Active validators produce blocks and vote on consensus
- Unstaking has a cooldown period (to be designed)
- Incentives: block rewards + tx fees (to be designed)

## Staking

- Staking is a native protocol feature — not a smart contract
- Transfers and staking are the first transaction types
- Delegation: not needed for V1 (handled by Phragmén in V2+)
- Slashing conditions: to be defined (double-sign, liveness)

## Rewards (V1)

Validators earn **transaction fees only** — no block rewards, no inflation.

- All fees from transactions included in a block go to that block's proposer
- Fee schedule: to be determined (flat fee? per-byte? auction?)
- In V2.0+, inflation can be added via [[V0.1.0/Protocol#Token Supply]] DI

## Multi-Validator Simulation

During development, run multiple validators locally via Docker:

```
docker compose up -d --scale validator=5
```

Each container runs `mononium-cli node` with its own key, data dir, and RPC port. This simulates a multi-validator network on a single machine.

## Network Participation

Validators operate on 4 network tiers:

| Tier                                  | Purpose                 |
| ------------------------------------- | ----------------------- |
| [[V0.1.0/Network#Localnet\|Localnet]] | Single-node development |
| [[V0.1.0/Network#Devnet\|Devnet]]     | Multi-validator testing |
| [[V0.1.0/Network#Testnet\|Testnet]]   | Public test network     |
| [[V0.1.0/Network#Mainnet\|Mainnet]]   | Production              |

---

**Related:** [[V0.1.0/Consensus]], [[V0.1.0/Network]]
