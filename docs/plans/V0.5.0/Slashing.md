---
tags: [consensus, validators, penalties]
---

# Slashing

If a proposer equivocates (proposes two blocks at the same slot), validators:

1. Reject duplicates at the protocol level
2. **Slash** 90% of the validator's stake (10% remains staked with the validator)
3. The **reporter** (validator who submitted the evidence) receives a **10% bounty** of the slashed amount, added to their validator stake
4. The next honest proposer resolves the fork

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

Example: validator with 1000 MONEX staked equivocates:

```
1000 stake
  ↓
  900 slashed (90%)   → 810 Burn, 90 to reporter's stake
  100 remains          → still staked with validator
```

**Bounty is staked, not liquid:** The 10% reporter bounty is added to the reporter's validator stake, not their transferable balance. This prevents two attack vectors:

1. **Slash-and-dump** — a validator who spots equivocation cannot immediately withdraw and cash out the reward
2. **Collusion exit** — the attacker and reporter cannot collude to bypass the unstaking cooldown (attacker intentionally equivocates, reporter gets liquid bounty, they split it out-of-band). With the bounty staked, both sides are bound by the 7-day unstaking lock.

**Validator's remaining 10% stays staked** — the validator is not ejected from the candidate pool. They keep a reduced stake and can continue validating (if still in Top-N) or choose to unstake with the standard 7-day cooldown.

**Two special addresses:**

| Address    | Role           | Effect                                                                                                                                             |
| ---------- | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `0x00..00` | **Burn**       | Slashed stake (90%) sent here. Permanently destroyed. No cap effect.                                                                               |
| `0x00..01` | **Cap-Refill** | Voluntarily send MONEX here to expand the mainnet inflation cap. Coins are a sink (irreversible). Effective max supply = 10B + cap_refill_balance. |

The Burn address and Cap-Refill address are known protocol constants. Anyone can send to either, but only slashing logic uses Burn automatically.

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

**Related:** [Consensus](plans/V0.5.0/Consensus.md), [Validators](plans/V0.5.0/Validators.md), [Protocol](plans/V0.5.0/Protocol.md)
