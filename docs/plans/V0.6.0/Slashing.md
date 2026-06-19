---
tags: [consensus, validators, penalties]
---

# Slashing

If a proposer equivocates (proposes two blocks at the same slot), validators:

1. Reject duplicates at the protocol level
2. **Slash** 90% of the validator's stake (10% remains staked with the validator)
3. The **reporter** (validator who submitted the evidence) receives a **10% bounty** of the slashed amount, added to their validator stake
4. The validator is **frozen** for 72 eras (~3 days) — excluded from proposing, voting, and rewards
5. The next honest proposer resolves the fork

## Slashing Details

| Dimension              | Value                                                                        |
| ---------------------- | ---------------------------------------------------------------------------- |
| **Equivocation**       | 90% of stake is slashed; 10% remains staked with the validator               |
| **Burn**               | 90% of slashed amount → Burn address (`0x00..00`)                            |
| **Reporter bounty**    | 10% of slashed amount → added to reporter's **validator stake**              |
| **Validator retains**  | 10% of original stake **stays staked** — not ejected from candidate pool     |
| **Burn effect**        | Coins at Burn address are permanently destroyed. No effect on inflation cap. |
| **Liveness**           | Not slashed in V1 (replaced at era boundary if inactive)                     |
| **Evidence topic**     | Gossiped on `mononium/evidence/{chain_id}`                                   |
| **Unstaking cooldown** | 7 days (constant, prevents gaming after violations)                          |
| **Freeze duration**    | 72 eras (~3 days) — excluded from proposer schedule, voting, and rewards     |

Example: validator with 1000 MONEX staked equivocates:

```
1000 stake
  ↓
  900 slashed (90%)   → 810 Burn, 90 to reporter's stake
  100 remains          → still staked, validator frozen for 72 eras
```

**Bounty is staked, not liquid:** The 10% reporter bounty is added to the reporter's validator stake, not their transferable balance. This prevents two attack vectors:

1. **Slash-and-dump** — a validator who spots equivocation cannot immediately withdraw and cash out the reward
2. **Collusion exit** — the attacker and reporter cannot collude to bypass the unstaking cooldown (attacker intentionally equivocates, reporter gets liquid bounty, they split it out-of-band). With the bounty staked, both sides are bound by the 7-day unstaking lock.

**Validator's remaining 10% stays staked, validator frozen** — the validator is ejected from the active set and enters the **Frozen** state for 72 eras (~3 days). During this period, the validator cannot propose blocks, vote on consensus, or earn rewards. The remaining 10% stake stays locked but inaccessible for participation. See [Freeze Period](#freeze-period) for full mechanics.

**Two special addresses:**

| Address    | Role           | Effect                                                                                                                                             |
| ---------- | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0x00..00` | **Burn**       | Slashed stake (90%) sent here. Permanently destroyed. No cap effect.                                                                               |
| `0x00..01` | **Cap-Refill** | Voluntarily send MONEX here to expand the mainnet inflation cap. Coins are a sink (irreversible). Effective max supply = 10B + cap_refill_balance. |

The Burn address and Cap-Refill address are known protocol constants. Anyone can send to either, but only slashing logic uses Burn automatically.

## Freeze Period

Getting slashed also **freezes** the validator for **72 eras** (~3 days at 720 blocks/era × 5s). During the freeze, the validator is excluded from all active participation:

1. **Cannot be assigned proposer slots** — excluded from proposer schedule generation at era boundaries
2. **Cannot vote** — BFT commit participation blocked
3. **Cannot earn rewards** — excluded from fee distribution
4. **Remaining 10% stake stays staked** — locked for the freeze duration (the existing 7-day unstaking cooldown already prevents immediate withdrawal)

### State Machine

```
Active (normal operation)
  │
  ├─ Equivocation evidence included on-chain ──→ Frozen (72 eras)
  │                                                  │
  │                                                  ├─ Can unstake remaining 10%
  │                                                  │  (7-day cooldown runs concurrently)
  │                                                  │
  │                                                  └─ After 72 eras ──→ Thawed
  │                                                                        │
  │                                                                        └─ Stake remains → candidate pool (re-electable)
  │                                                                        └─ Fully unstaked → Inactive
  │
  └─ (normal unstaking) ──→ Unstaking (7d) ──→ Inactive
```

### Freeze Countdown

```rust
struct FreezeRecord {
    validator_id: [u8; 32],
    frozen_at_era: u64,       // era when evidence block was included
    remaining_eras: u16,      // starts at 72, decremented at each era boundary
}
```

- Freeze starts from the **era in which the slashing evidence was included** in a block
- At each era boundary, the consensus engine decrements `remaining_eras` for all frozen validators
- When `remaining_eras == 0`, the validator is automatically thawed

### Era Boundary Processing

At every era boundary, the following happens in order:

1. Decrement `remaining_eras` for all `FreezeRecord` entries
2. Validators with `remaining_eras == 0` are thawed (status changed from `Frozen` to `Thawed`)
3. The validator election (`TopN`) evaluates all staked candidates **excluding** those with `Frozen` status
4. Thawed validators with remaining stake join the candidate pool and are eligible for re-election
5. Slashing evidence that arrived during the era is processed before the election — a validator slashed in the last block of era N enters era N+1 as Frozen

### Remaining Stake During Freeze

| Action            | Allowed? | Effect                                               |
| ----------------- | -------- | ---------------------------------------------------- |
| Propose blocks    | ❌       | Excluded from proposer schedule                      |
| Vote on consensus | ❌       | Blocked during freeze                                |
| Earn rewards      | ❌       | Excluded from fee distribution                       |
| Submit Unstake tx | ✅       | 7-day cooldown runs concurrent with freeze           |
| Re-stake more     | ❌       | Cannot increase stake while frozen (must thaw first) |
| Thaw naturally    | ✅       | Automatic after 72 eras                              |

### Thaw

When a validator thaws:

- If the validator still has staked MONEX (didn't fully unstake during the freeze), they re-enter the candidate pool at the next era boundary
- If the validator fully unstaked (remaining 10% withdrawn during the freeze), they become Inactive and must re-register and stake from scratch
- The validator's key is not burned or invalidated — they simply return to the election pool like any other candidate

### Freeze vs. Unstaking

| Aspect                | Freeze (72 eras)      | Unstaking (7 days)               |
| --------------------- | --------------------- | -------------------------------- |
| Trigger               | Equivocation evidence | Voluntary Unstake tx             |
| Participation allowed | None                  | None (already ejected)           |
| Stake locked          | ✅                    | ✅ (7-day cooldown)              |
| Automatic end         | After 72 eras         | After 7 days                     |
| Re-entry              | Automatic (if staked) | Re-register + stake from scratch |

The freeze period (72 eras ≈ 51840 blocks = 72h) is significantly longer than the unstaking cooldown (7 days ≈ 120960 blocks). Even if the validator unstakes immediately when frozen, the freeze outlasts the cooldown in some cases — preventing a validator from avoiding the freeze by unstaking first.

### Double-Slashing

A validator can only be slashed once. After the freeze expires and the validator returns to the active set, they can be slashed again for a new equivocation — starting a fresh 72-era freeze.

A validator who is already frozen cannot submit evidence (they are not participating in consensus). If a frozen validator's earlier equivocation is discovered after they froze, secondary evidence against them is ignored — the single slashing and freeze covers the event.

## Evidence Format

Slashing evidence is an `EquivocationEvidence` message containing the **block headers + Falcon signatures** (not the full blocks). This keeps evidence small (~1.5 KB per event vs ~1 MB for full blocks).

```rust
/// Proof that a validator signed two different blocks at the same height
struct EquivocationEvidence {
    pub header_a: BlockHeader,
    pub signature_a: [u8; 666],
    pub header_b: BlockHeader,
    pub signature_b: [u8; 666],
    pub proposer: [u8; 32],       // validator address — evidence must be verifiable outside era context
}
```

**Verification:**

1. `header_a.height == header_b.height`
2. `header_a.parent_hash == header_b.parent_hash` (same slot — resolves to same parent)
3. `header_a != header_b` (distinct blocks — proves equivocation, not re-gossip)
4. `falcon_verify(proposer_pk, header_a, signature_a)` — both genuinely signed
5. `falcon_verify(proposer_pk, header_b, signature_b)` — both genuinely signed

If all checks pass, the proposer is slashed 90% and the reporter receives a 10% bounty.

- Slashing is **equivocation only** in V1 (no liveness slashing)
- Inactive validators are simply replaced at the next era boundary
- Evidence is gossiped on the `mononium/evidence/{chain_id}` topic
- Any validator can submit evidence as a transaction
- Coins sent to Burn are permanently destroyed — no effect on supply cap
- The Cap-Refill address (`0x00..01`) is unrelated to slashing; see [Supply](./Genesis.md#Token-Supply)

---

**Related:** [Consensus](plans/V0.6.0/Consensus.md), [Validators](plans/V0.6.0/Validators.md), [Protocol](plans/V0.6.0/Protocol.md), [ADR-017](../../architecture/ADR-017-slashing-freeze.md)
