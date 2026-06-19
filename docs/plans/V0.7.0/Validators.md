---
tags: [consensus, network, staking]
---

# Validators

## Overview

Mononium uses **Proof of Stake (PoS)**. The chain starts with a **bootstrap key** (designated in the genesis config) as the sole proposer for the first N blocks, giving time for validators to register before normal consensus begins. After the bootstrap phase, validators stake **Monium (MONEX)** to participate in block production and consensus via era 0 Open election. All protocol signatures use **Falcon-512** (post-quantum secure, constant-time).

## Target Hardware

The network explicitly targets **cheap VPS** hardware:

| Resource  | Target                                                                                                                                                                                                                                                                         |
| --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| CPU       | Low — 1-2 vCPU                                                                                                                                                                                                                                                                 |
| RAM       | **~70-120 MB** (full mode, Devnet, 21 validators, minimal state) — application footprint is fixed; redb mmap grows with state size but OS-managed. **~55-80 MB** in compact mode (storage.mode: compact) — saves ~15-40 MB by dropping historical block bodies and checkpoints |
| Bandwidth | Low — 500 KB blocks imply modest traffic                                                                                                                                                                                                                                       |
| Disk      | Minimal write amplification via redb                                                                                                                                                                                                                                           |

The goal is accessibility — running a validator should not require data center infrastructure.

## Consensus Parameters

| Parameter         | Value                                                |
| ----------------- | ---------------------------------------------------- |
| Consensus         | PoS                                                  |
| Block time        | 5 seconds                                            |
| Finality          | 20 seconds (4 blocks)                                |
| Block size cap    | 500 KB                                               |
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
Era 0:   Inactive → Registered → Active (no stake needed)
Era 1+:  Inactive → Registered → Staked → Active ──→ Unstaking → Inactive
                                            │
                                            └──→ Frozen ──→ Thawed ──→ Active (back in pool)
                                               (slashing)   (72 eras)
```

- **RegisterValidator** — one-time tx declaring intent with public key.
- **Era 0 (Open):** Registered validators are automatically active (up to `max_validators`). No stake required. They earn fees to accumulate starting stake for era 1+.
- **Era 1+ (Top-N):** Staking is required. Minimum **1 MONEX** to enter the candidate pool — prevents dust entries.
- **RegisterAndStake** — convenience tx that registers + stakes atomically for era 1+ onboarding.
- Staked validators are sorted by stake; Top N become active at each era boundary.
- Active validators produce blocks and vote on consensus.
- **Slashing** triggers a **Frozen** state — 72 eras of exclusion from proposing, voting, and rewards (see [Slashing](#slashing)).
- Unstaking has a **7-day cooldown** — prevents gaming after violations.
- Incentives: transaction fees (no block rewards in V1).

### Slashing

Slashing mechanics are documented in [Slashing](plans/V0.7.0/Slashing.md). In summary: equivocation causes 90% stake loss plus a **72-era freeze** during which the validator cannot propose, vote, or earn rewards. See [ADR-017](../../architecture/ADR-017-slashing-freeze.md) for the freeze design rationale.

## Staking

- Staking is a native protocol feature — not a smart contract
- Transfers and staking are the first transaction types
- Delegation: not needed for V1 (handled by Phragmén in V2+)
- Unstaked funds become available after the 7-day cooldown

## Key Management

Key generation, storage, and loading are documented in [Cryptography](plans/V0.7.0/Cryptography.md#Key-Storage).

## Rewards

Fee income is distributed **pro-rata by stake** across all active validators at the end of every block — **not** kept by the proposer. See [Fees](plans/V0.7.0/Fees.md) for the full distribution mechanics.

### Dev Networks (Localnet, Devnet, Testnet)

**Transaction fees only** — no block rewards, no inflation.

- Fees from all transactions in a block are pooled, then split among all active validators in proportion to each validator's stake
- Fee schedule: flat fee (0.00667 MONEX) + per-byte (0.000467 MONEX/byte) + optional tip

### Mainnet

**Transaction fees + block rewards** — capped inflation provides the block reward component.

- Fees distributed identically to dev networks (pro-rata by stake, per block)
- Block rewards defined by `CappedInflation` policy (see [Genesis](plans/V0.7.0/Genesis.md#Token-Supply))
- Block rewards are minted per block and added to the fee pool before distribution, or distributed separately — the state machine handles both identically (both go to validators pro-rata by stake)

## Multi-Validator Simulation

During development, run multiple validators locally via Docker:

```
docker compose up -d --scale validator=5
```

Each container runs `mononium-cli node` with its own key, data dir, and RPC port. This simulates a multi-validator network on a single machine.

## Network Participation

Validators operate on 4 network tiers:

| Tier                                         | Purpose                 |
| -------------------------------------------- | ----------------------- |
| [Localnet](plans/V0.7.0/Network.md#Localnet) | Single-node development |
| [Devnet](plans/V0.7.0/Network.md#Devnet)     | Multi-validator testing |
| [Testnet](plans/V0.7.0/Network.md#Testnet)   | Public test network     |
| [Mainnet](plans/V0.7.0/Network.md#Mainnet)   | Production              |

---

**Related:** [Consensus](plans/V0.7.0/Consensus.md), [Network](plans/V0.7.0/Network.md), [Slashing](plans/V0.7.0/Slashing.md), [Cryptography](plans/V0.7.0/Cryptography.md)
