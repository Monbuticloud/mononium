---
tags: [protocol, fees, economics]
---

# Fees

## Standard Fees

Fee per transaction = **flat component** + **size component** + **optional tip**

```rust
pub struct HybridFee {
    pub flat_fee: U256,         // 0.00667 MONEX — minimum cost per tx
    pub per_byte_rate: U256,    // 0.000467 MONEX/byte — proportional to size
    // tip is set by sender as part of Transaction
}
```

| Component | Value              | Purpose                               | Set by             |
| --------- | ------------------ | ------------------------------------- | ------------------ |
| Flat fee  | **0.00667 MONEX**  | Minimum cost per tx (spam prevention) | Protocol parameter |
| Per-byte  | **0.000467 MONEX** | Proportional to state/storage cost    | Protocol parameter |
| Tip       | User-defined       | Priority for block inclusion          | Sender             |

```rust
impl FeePolicy for HybridFee {
    fn calculate_fee(&self, tx: &Transaction) -> U256 {
        match &tx.body {
            TxBody::Burn { .. } => U256::from(10),  // 10 MOXX flat
            _ => self.flat_fee + self.per_byte_rate * U256::from(tx.encoded_size()) + tx.tip,
        }
    }
}

Burn transactions bypass standard fee calculation — flat **10 MOXX** regardless of size or tip.

These values are the same across all network tiers (Localnet, Devnet, Testnet, Mainnet). Swappable via `FeePolicy` trait.

**Note on `min_fee`:** The mempool has a `min_fee` threshold (`0.0667 MONEX`) — this is a **local node policy**, not a consensus parameter. Each operator sets their own filter in the node config file. A tx below `min_fee` is rejected from the local mempool but would still be valid in a block proposed by another validator. See [NodeConfig](./NodeConfig.md#mempoolmin_fee) and [Mempool](./Mempool.md).

**Block hard cap:** 500 txs OR 1 MB per block. With the per-account rate limit of 50 txs/block, at least 10 distinct accounts are needed to fill a block. Batch operations (e.g., distributing to 200 recipients) will span multiple blocks (~20s at 5s block time). Wallet and tooling developers should account for this when designing batch workflows.

## Anti-Spam Deposit

Every transaction requires a **0.33 MONEX deposit** per tx, deducted from the sender's balance and held for **2 eras**. This is the primary anti-spam mechanism — the capital cost scales proportionally with volume (100 txs = 33 MONEX temporarily locked).

- **Per transaction:** Each tx locks 0.33 MONEX from the sender's balance. The deposit is held for **2 eras** from the era of inclusion
- **Auto-return:** Deposits are returned to the sender's balance at the **era N+2 boundary** (2 eras after the era of inclusion). If a tx is included in block 719 (last block of era N), the deposit returns at block 1440 (era N+2 boundary). If included in block 720 (first block of era N+1), the deposit returns at block 2160 (era N+3 boundary). No explicit reclaim tx needed
- **No exemption:** Burn txs also lock 0.33 MONEX — the 10 MOXX fee covers processing, but the deposit still applies
- **Capital cost example:** An account submitting 50 txs in one era has 16.5 MONEX temporarily locked. All return at the **era boundary after next** (2 eras from submission)
- **Observer nodes:** No deposit needed (no tx submission)
- **Dev networks:** 0.33 MONEX per tx on Devnet (100 MONEX/key) — 3 txs consume 1% of balance. Enforced for consistency
- **Rate limit:** Pair with the per-account rate limit (50 txs/block) — the deposit is economic anti-spam, the rate limit prevents block congestion

The per-tx deposit combined with the per-block rate limit creates two independent anti-spam layers. An attacker needs both significant capital (deposits) and multiple accounts (rate limit) to congest the network.

## Fee Distribution

Collected fees are **not** kept by the proposer. Instead, they are distributed **pro-rata by stake** across **all active validators** at the end of each block.

### Distribution Mechanics

```

Block applied with N transactions
→ total_fees = sum of all tx fees in the block
→ total_active_stake = sum of all active validators' stake
→ For each active validator V:
V's share = total_fees \* (V.stake / total_active_stake)
V's balance += V's share

```

**When it happens:** At the end of `apply_block()`, after all transactions have been processed (including failed ones that still paid fees), before computing the new state root.

**What gets distributed:** Every fee component — flat_fee + per_byte + tip. All three are pooled and split identically. Nothing is kept by the proposer beyond their pro-rata share based on their own stake.

**Where fees go:** Each validator's share is added to their **transferable balance** (not their stake). Validators can choose to withdraw, transfer, or re-stake their fee earnings.

**What about the proposer?** The proposer receives their pro-rata share like every other active validator. They are not special — their proposer role does not entitle them to extra fee income beyond what their stake weight dictates.

### State Machine Detail

The state machine maintains a **fee accumulator** per block:

```

1. Initialize block_fees = 0
2. For each tx in the block (processed in order):
   a. Validate tx (signature, nonce, balance, fee sufficiency)
   b. If valid: - Deduct fee from sender's balance - Execute the transaction (transfer, stake, etc.) - block_fees += fee
   c. If invalid: - Skip execution - Deduct fee from sender's balance (failed txs still pay) - block_fees += fee
3. After all txs processed:
   a. total_active_stake = sum of all stakes in active validator set
   b. For each active validator V with stake S_V:
   V.balance += block_fees \* S_V / total_active_stake
4. Compute new state root (includes updated balances)

```

**Precision:** All division uses integer arithmetic with U256. To handle rounding, the fee distribution must distribute the full `block_fees` across validators without losing wei to truncation. Implementation strategy: distribute using `block_fees * stake / total_stake` for each validator, then allocate the remainder (due to integer truncation) to the validator with the lowest address among those with the highest stake. This guarantees the full fee pool is distributed each block with a deterministic tie-breaker.

### Comparison to Options Considered

| Option | Description | Chosen? | Why rejected |
|--------|------------|---------|-------------|
| A | Proposer keeps 100% of fees | ❌ | Creates feast-or-famine reward pattern; non-proposing validators earn nothing for 105s on a 21-set |
| B | Split equally among active set | ❌ | Ignores stake weight; a 1 MONEX validator earns the same as a 1,000 MONEX validator |
| **C** | **Split pro-rata by stake** | **✅** | Rewards commitment proportionally; all validators earn every block; no special proposer bonus needed |

---

## Economic Security

The fee and staking model is designed to resist economic attacks. The table below summarizes each vector and the defenses in place:

| Attack Vector | Defense | Sufficient? |
|---------------|---------|-------------|
| **Sybil attack** (create many validators to dominate) | 1 MONEX minimum stake + Top-N election by stake weight | ✅ Yes — cost to reach Top-N scales linearly with network value |
| **Low-cost censorship** (spam many txs to fill blocks) | 0.33 MONEX anti-spam deposit per tx + 50 tx/block rate limit per account | ✅ Probably — attacker needs significant locked capital |
| **Fee market manipulation** (submit many low-fee txs to raise the floor) | Tip-based ordering + local `min_fee` filter per operator | ✅ Yes — `min_fee` is a local policy, each operator controls their own threshold |
| **Validator cartel** (collude to censor or reorg) | Slashing (90% equivocation penalty) + Top-N stake-weighted election makes collusion expensive | ⚠️ Possible but expensive — controlling > 2/3 of stake is the bar for meaningful attacks |
| **Long-range attack** (create alternative history from far back) | Checkpoints at era boundaries + genesis hash committed at peer connection | ⚠️ Requires out-of-band checkpoint verification (publish genesis hash + recent checkpoint hashes on a known website/social) |
| **Nothing at stake** (vote on both forks risk-free) | 90% equivocation slashing + 72-era freeze | ✅ Yes — cost of equivocation far exceeds any benefit |

---

**Related:** [Protocol](plans/V0.7.0/Protocol.md), [Genesis](plans/V0.7.0/Genesis.md), [Mempool](plans/V0.7.0/Mempool.md), [Validators](plans/V0.7.0/Validators.md)
```
