---
tags: [consensus, network, staking]
---

# Validators

## Overview

Mononium uses **Proof of Stake (PoS)**. Validators stake **Monium (MONEX)** to participate in block production and consensus. All protocol signatures use **Falcon-512** (post-quantum secure, constant-time).

## Target Hardware

The network explicitly targets **cheap VPS** hardware:

| Resource  | Target                                        |
| --------- | --------------------------------------------- |
| CPU       | Low — 1-2 vCPU                                |
| RAM       | Low — fixed memory footprint                  |
| Bandwidth | Low — 500 KB blocks imply modest traffic      |
| Disk      | Minimal write amplification via redb          |

The goal is accessibility — running a validator should not require data center infrastructure.

## Consensus Parameters

| Parameter         | Value                              |
| ----------------- | ---------------------------------- |
| Consensus         | PoS                                |
| Block time        | 5 seconds                          |
| Finality          | 20 seconds (4 blocks)              |
| Block size cap    | 500 KB                             |
| Throughput target | 100–200 TPS (emerges naturally with Falcon-512 sigs) |

## Bottlenecks

Priority order of validator bottlenecks (from most to least constrained):

1. **Network traffic** — consensus messages, block propagation (Falcon signatures are 666 bytes)
2. **Signature verification** — Falcon-512 batch verification (~10x slower than Ed25519)
3. **State / database access** — redb reads and writes
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
- Swappable via dependency injection (see [Architecture](Architecture.md#Validator Election DI))

## Validator Lifecycle

```
State: Inactive → Staked → Active → Unstaking → Inactive
```

- Stake MONEX to join the candidate pool
- Top N by stake become active
- Active validators produce blocks and vote on consensus
- Unstaking has a **7-day cooldown** — prevents gaming after violations
- Incentives: transaction fees (no block rewards in V1)

### Slashing

If a validator equivocates (signs two blocks at the same height):

| Penalty       | Value |
| ------------- | ----- |
| Stake burned  | **90%** of total staked MONEX |
| Reporter bounty | **10%** of slashed amount (paid to reporting validator) |

- Slashing is **equivocation only** in V1 (no liveness slashing)
- Inactive validators are simply replaced at the next era boundary
- Evidence is gossiped on the `mononium/evidence/{chain_id}` topic
- Any validator can submit evidence as a transaction

## Staking

- Staking is a native protocol feature — not a smart contract
- Transfers and staking are the first transaction types
- Delegation: not needed for V1 (handled by Phragmén in V2+)
- Unstaked funds become available after the 7-day cooldown

## Key Management

Validator keys use **Falcon-512** and are stored encrypted at rest:

| Step | Description |
|------|-------------|
| **Generation** | `mononium-cli wallet keygen --name my-validator` generates Falcon-512 keys (~10ms, offline) |
| **Encryption** | NaCl secretbox (XSalsa20-Poly1305) |
| **KDF** | Argon2id (1 GiB memory, 4 iterations, 4 parallel) — `argon2` crate |
| **File** | `~/.mononium/keys/my-validator.json` — contains public key (plaintext) + encrypted seed |
| **Loading** | `mononium-cli node --key my-validator` prompts for passphrase, decrypts, re-derives private key |
| **Unlock time** | ~5-10s due to Argon2id memory cost (one-time at startup) |

The public key (897 bytes) is stored in plaintext in the key file. Only the 48-byte seed is encrypted. The private key (1281 bytes) is re-derived from the seed at node startup.

## Rewards (V1)

Validators earn **transaction fees only** — no block rewards, no inflation.

- All fees from transactions included in a block go to that block's proposer
- Fee schedule: flat fee (0.00667 MONEX) + per-byte (0.000467 MONEX/byte) + optional tip
- In V2.0+, inflation can be added via [Protocol](Protocol.md#Token Supply) DI

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
| [Localnet](plans/V0.2.0/Network.md#Localnet) | Single-node development |
| [Devnet](plans/V0.2.0/Network.md#Devnet)     | Multi-validator testing |
| [Testnet](plans/V0.2.0/Network.md#Testnet)   | Public test network     |
| [Mainnet](plans/V0.2.0/Network.md#Mainnet)   | Production              |

---

**Related:** [Consensus](plans/V0.2.0/Consensus.md), [Network](plans/V0.2.0/Network.md)
