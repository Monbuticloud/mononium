---
tags: [governance, protocol, consensus]
---

# Governance

## Overview

Full on-chain governance. Any staker can submit a proposal; all stakers vote with stake-weighted ballots. Proposals that meet quorum and approval thresholds execute automatically at the next era boundary.

## Design Principles

1. **On-chain, always** — every proposal and vote is recorded on-chain. No off-chain signaling.
2. **Stake-weighted** — voting power = total staked MONEX (active + inactive stakers)
3. **Deliberate pacing** — 7-era voting window prevents rushed decisions; era-boundary execution prevents mid-era state inconsistency
4. **Spam-resistant** — proposals require a non-trivial deposit that is forfeited if the proposal expires without meeting quorum
5. **No runtime upgrades in V1** — governance modifies on-chain parameters only. Software upgrades are coordinated off-chain.

## Governance-Mutable Parameters

| Parameter              | Current value    | Governance scope    |
| ---------------------- | ---------------- | ------------------- |
| `max_validators`       | 21 (dev) / 101   | Adjust up or down   |
| `era_length`           | 720 blocks       | Adjust up or down   |
| `block_size_cap_bytes` | 500 KB           | Adjust up           |
| `block_tx_cap`         | 500 txs          | Adjust up or down   |
| `flat_fee`             | 0.00667 MONEX    | Adjust up or down   |
| `per_byte_rate`        | 0.000467 MONEX   | Adjust up or down   |
| `anti_spam_deposit`    | 0.33 MONEX       | Adjust up or down   |
| `missed_slot_penalty`  | 0.08 MONEX       | Adjust up or down   |
| `n_shards`             | 2                | Increase only       |
| `supply_ceiling_rate`  | 3.5%             | Adjust up or down   |
| `supply_headroom_rate` | 5.0%             | Adjust up or down   |

Parameters not listed (block time, chain ID, signature scheme, consensus mode) are compile-time constants — not governance-mutable in V1.

### Shard Count Special Case

`n_shards` can only be **increased** via governance (already specified in [StateSharding.md](./StateSharding.md)). The 24-era grace period and validator pre-compute window apply. Overwrites the shard-vote mechanism in StateSharding.md — shard count proposals use the standard governance flow instead of a specialized one.

## Proposal Lifecycle

```
Submit → Voting Window (7 eras) → Tally at Era Boundary → Execute or Expire
  │                                                       │
  └─ Cancelled (by proposer before 1st vote)              ├─ Approved → execute next era
                                                          └─ Rejected / No quorum → deposit forfeited
```

### 1. Submit

Any staker (active validator or inactive staker) submits a `Propose` transaction:

```rust
struct Proposal {
    proposal_id: [u8; 32],        // BLAKE3 of (proposer, nonce, title)
    title: Vec<u8>,                // UTF-8, max 256 bytes
    description: Vec<u8>,          // UTF-8, max 4096 bytes
    actions: Vec<GovernanceAction>,
    deposit: U256,                 // locked until resolution
}

enum GovernanceAction {
    UpdateParam { param: GovernanceParam, new_value: U256 },
    IncreaseShards { new_count: u16, effective_era: u64 },
}

enum GovernanceParam {
    MaxValidators,
    EraLength,
    BlockSizeCapBytes,
    BlockTxCap,
    FlatFee,
    PerByteRate,
    AntiSpamDeposit,
    MissedSlotPenalty,
    SupplyCeilingRate,
    SupplyHeadroomRate,
}
```

**Deposit:** 100 MONEX (held until proposal resolves). Forfeited if the proposal expires without meeting quorum. Returned to proposer if approved or rejected with quorum.

**Rate limits:**
- Max 5 active proposals per proposer at any time
- Max 50 proposals per era globally (prevents proposal spam even with deposits)
- Parameters are locked during an active proposal targeting them — a second proposal for the same param cannot be submitted until the first resolves

### 2. Voting Window

Opens immediately upon proposal submission. Lasts **7 eras** (~7 hours at 720 blocks/era, 5s blocks).

Stakers submit `Vote` transactions:

```rust
struct Vote {
    proposal_id: [u8; 32],
    voter: [u8; 32],               // staker address
    approve: bool,                 // true = for, false = against
    weight: U256,                  // voter's total stake at vote time
}
```

- Voting power is **snapshotted at vote submission time** — the voter's stake at the block height they submit the vote
- One vote per proposal per voter. Submitting a second vote overwrites the first (allows changing position)
- Voters can vote at any point during the 7-era window — no locked periods
- Weight is the voter's total staked MONEX (including frozen/thawed stake — slashed amounts are gone, remaining stake counts)

### 3. Tally at Era Boundary

At the era boundary following the close of the voting window:

```
1. Total participating stake = sum of all approve + all against weights
2. Total active stake = total MONEX staked across all stakers
3. If total_participating < (total_active_stake × 2/3):
       → Proposal FAILS (no quorum)
       → Deposit forfeited to 0x00..01 (Cap-Refill)
4. If total_participating >= (total_active_stake × 2/3):
       If approve_weight > total_participating / 2:
           → Proposal PASSES
           → Enqueue for execution at next era boundary
           → Deposit returned to proposer
       Else:
           → Proposal FAILS (majority against)
           → Deposit returned to proposer
```

**Quorum:** 2/3 of **total active stake** (not participating stake). Same bar as shard-count votes in StateSharding.md.

**Threshold:** Simple majority (>50%) of participating stake.

**Double-signing prevention:** A voter cannot vote on the same proposal twice for different weights (second vote overwrites). This prevents stake-transfer vote doubling.

### 4. Execution

Approved proposals execute **at the next era boundary after tallying** — never mid-era. This provides a full era for validators to observe the outcome and prepare.

Execution flow at era boundary:

```
1. Collect all approved proposals from the previous era's tally
2. For each proposal, apply its actions in order:
   a. UpdateParam → write new value to governance parameter state
   b. IncreaseShards → trigger shard migration (see StateSharding.md)
3. Emit governance execution events (stored in meta table)
4. Proceed with normal era boundary processing
```

If multiple approved proposals modify the same parameter, the **last one by era boundary order** wins (proposals are processed in proposal_id hash order for determinism).

### 5. Cancellation

The proposer can cancel their own proposal **before the first vote is cast**:

```rust
struct CancelProposal {
    proposal_id: [u8; 32],
}
```

- Only the original proposer may cancel
- Only valid before any votes are recorded
- Deposit is returned in full
- Prevents a proposer from changing their mind after learning new information

After the first vote, cancellation is impossible — the proposal must run its course.

## State Machine Integration

### New Tables

Added to `meta` namespace (or a separate governance namespace `0x03` in the SMT):

| Data                     | Key                        | Value               | Notes                          |
| ------------------------ | -------------------------- | ------------------- | ------------------------------ |
| Proposal metadata        | `prop_{proposal_id}`       | `Proposal`          | Full proposal record           |
| Vote records             | `vote_{proposal_id}_{voter}` | `Vote`            | One per voter per proposal     |
| Governance params        | `gov_param_{param_name}`   | `U256`              | Current value of mutable param |
| Active proposal counter  | `gov_active_count`         | `u64`               | Rate limit counter             |

### Era Boundary Hook

At every era boundary, after validator set recalculation but before the proposer schedule reset:

```
1. For each open proposal whose submission_era + 7 == current_era:
   a. Tally votes
   b. If quorum met and majority approves → mark approved
   c. If quorum not met → mark expired (deposit forfeited)
   d. If quorum met but majority rejects → mark rejected (deposit returned)
2. Apply all approved proposals' actions
3. Reset governance execution for this era
```

### Transaction Validation

The state machine validates governance txs before execution:

**Propose validation:**
- Proposer must have ≥ 100 MONEX staked (prevents dust-proposal spam)
- Proposal ID must not collide with an existing active proposal (checked against meta table)
- Title ≤ 256 bytes, description ≤ 4096 bytes
- Actions must reference valid governance parameters
- `IncreaseShards` must satisfy constraints from StateSharding.md (new_count > current, not already pending)
- Proposer must not already have 5+ active proposals
- Global proposal rate limit (50/era) not exceeded
- Deposit 100 MONEX deducted from proposer's balance

**Vote validation:**
- Proposal must be in voting window (submission_era ≤ current_era < submission_era + 7)
- Voter must have > 0 staked MONEX
- Weight snapshotted at current block
- Overwrites previous vote if one exists

**Cancel validation:**
- Only proposer may cancel
- Proposal must have zero votes
- Proposal must be in voting window (not expired)

## Relationship to Other Docs

| Doc              | Interaction                                                         |
| ---------------- | ------------------------------------------------------------------- |
| Consensus.md     | Governance executes at era boundary, after validator set changes    |
| StateSharding.md | Shard count increases use governance flow instead of custom votes   |
| Genesis.md       | Supply policy parameters are governance-mutable                     |
| Fees.md          | Fee parameters (flat, per-byte, deposit) are governance-mutable     |
| Slashing.md      | Slashing parameters not governance-mutable in V1 (safety constraint)|
| Protocol.md      | Governance tx types added to TxBody enum                            |

## V2 Future

Governance in V2 could be extended with:

- **Delegated voting** — proxy your voting power to a trusted voter
- **Timelock** — approved proposals have a minimum delay before execution (e.g., 7 more eras)
- **Emergency proposals** — fast-track with 2/3 super-majority and shorter window
- **Treasury** — inflation-diverted funds managed by governance
- **Runtime upgrades** — WASM blob replacement via governance vote
- **Parameter bounds** — governance cannot set parameters outside safe ranges (e.g., block_time can't go below 2s)

None of these are committed for any V2 timeline.

---

**Related:** [StateSharding](./StateSharding.md), [Consensus](./Consensus.md), [Protocol](./Protocol.md), [Genesis](./Genesis.md), [Fees](./Fees.md)
