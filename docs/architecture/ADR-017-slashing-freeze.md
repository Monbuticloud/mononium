# ADR-017: Slashing Freeze Period

**Status:** Accepted

**Context:** Currently, when a validator is slashed for equivocation, they lose 90% of their stake but remain eligible for block production and consensus voting with the remaining 10%. This means a slashed validator can immediately return to proposing and voting, which dilutes the deterrent power of slashing. A slashed validator may also attempt to recoup losses through future fees rather than facing meaningful consequences. Additionally, while the validator is still active, the network must continue to rely on a party that has already demonstrated malicious behavior.

**Decision:**

In addition to the existing 90% stake penalty, a slashed validator enters a **Frozen** state for **72 eras** (~3 days at 5s blocks, 720 blocks/era).

### Freeze Mechanics

A frozen validator is:

- Excluded from the proposer schedule — cannot be assigned block production slots
- Excluded from BFT commit voting — cannot participate in finality
- Excluded from fee/reward distribution — no income during freeze
- Remaining 10% stake remains staked but the validator cannot participate
- Listed as frozen in the active set — visible to other validators and RPC queries

### Freeze Countdown

- Freeze clocks start counting from the **era in which the slashing evidence block is included**
- Duration: exactly 72 eras (absolute, not relative to the network's total era count)
- At each era boundary, the consensus engine decrements each frozen validator's counter
- When the counter reaches zero, the validator thaws automatically

```rust
struct FreezeRecord {
    validator_id: [u8; 32],
    frozen_at_era: u64,       // era when evidence was included
    remaining_eras: u16,      // counted down at each era boundary
}
```

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
  │                                                                        └─ If still staked → candidate pool (re-electable)
  │                                                                        └─ If unstaked → Inactive
  │
  └─ (normal unstaking) ──→ Unstaking (7d) ──→ Inactive
```

### Interaction with Unstaking

- A frozen validator **may** submit an Unstake transaction for their remaining 10% stake
- The 7-day unstaking cooldown runs concurrently with the freeze period
- At thaw, the validator's remaining stake is evaluated:
  - Still staked (didn't unstake or unstake not yet complete) → enters candidate pool at next election
  - Fully unstaked → becomes Inactive

### Freeze and the Reporter Bounty

- The 10% reporter bounty is credited at slashing time as before
- The reporter is **not** affected by the frozen validator's freeze status — the bounty is added to the reporter's stake immediately
- If the reporter is themselves frozen (they submitted evidence while frozen), the bounty still adds to their stake — it unlocks when their own freeze expires

### Era Boundary Processing

At each era boundary, before the new active set is elected:

1. Decrement `remaining_eras` for all frozen validators
2. Validators with `remaining_eras == 0` are thawed (status set to `Thawed`)
3. The election runs on all staked candidates **excluding** those with `Frozen` status
4. Thawed validators with remaining stake join the candidate pool normally

### Evidence Format

No changes to `EquivocationEvidence`. The freeze is triggered by the same evidence flow — it is an additional consequence applied atomically alongside the stake slashing.

### Consequences

- Stronger deterrent — equivocation costs not just stake but also the ability to validate for ~3 days
- Active set quality improves — malicious parties are quickly removed from production
- Additional implementation surface: `FreezeRecord` storage, freeze counter at era boundaries, freeze exclusion in proposer selection and election
- Minimal state overhead — one row per frozen validator in a `frozen_validators` table (~40 bytes per entry at most, and there are at most N active validators, realistically 0-2 frozen at any time)
- No governance complexity — freeze is automatic and deterministic

**Related:** Consenus.md (proposer schedule), Slashing.md (freeze period detail), Validators.md (lifecycle update)
