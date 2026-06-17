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

## Validator Lifecycle

```
State: Inactive → Staked → Active → Unstaking → Inactive
```

- Stake MONEX to join the active set
- Active validators produce blocks and vote on consensus
- Unstaking has a cooldown period (to be designed)
- Incentives: block rewards + tx fees (to be designed)

## Staking

- Staking is a native protocol feature — not a smart contract
- Transfers and staking are the first transaction types
- Delegation: not needed for V1 (validator set is small initially)
- Slashing conditions: to be defined (double-sign, liveness)

## Network Participation

Validators operate on 4 network tiers:

| Tier                                         | Purpose                 |
| -------------------------------------------- | ----------------------- |
| [Localnet](plans/V0.0.0/Network.md#Localnet) | Single-node development |
| [Devnet](plans/V0.0.0/Network.md#Devnet)     | Multi-validator testing |
| [Testnet](plans/V0.0.0/Network.md#Testnet)   | Public test network     |
| [Mainnet](plans/V0.0.0/Network.md#Mainnet)   | Production              |

---

**Related:** [Consensus](plans/V0.0.0/Consensus.md), [Network](plans/V0.0.0/Network.md)
