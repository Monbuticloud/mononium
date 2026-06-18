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

Every transaction requires a **1 MONEX deposit** per tx, deducted from the sender's balance and held until the era boundary. This is the primary anti-spam mechanism — the capital cost scales proportionally with volume (100 burn txs = 100 MONEX temporarily locked).

- **Per transaction:** Each tx locks 1 MONEX from the sender's balance for the remainder of the current era
- **Auto-return:** All deposits are returned to the sender's balance at the **era boundary** (block % 720 == 0). No explicit reclaim tx needed
- **No exemption:** Burn txs also lock 1 MONEX — the 10 MOXX fee covers processing, but the deposit still applies
- **Capital cost example:** An account submitting 50 txs in one era has 50 MONEX temporarily locked. At the era boundary, all 50 return to the sender's available balance
- **Observer nodes:** No deposit needed (no tx submission)
- **Dev networks:** 1 MONEX per tx is trivial on Devnet (100 MONEX/key) but enforced for consistency
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
      V's share = total_fees * (V.stake / total_active_stake)
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
     b. If valid:
          - Deduct fee from sender's balance
          - Execute the transaction (transfer, stake, etc.)
          - block_fees += fee
     c. If invalid:
          - Skip execution
          - Deduct fee from sender's balance (failed txs still pay)
          - block_fees += fee
3. After all txs processed:
     a. total_active_stake = sum of all stakes in active validator set
     b. For each active validator V with stake S_V:
          V.balance += block_fees * S_V / total_active_stake
4. Compute new state root (includes updated balances)
```

**Precision:** All division uses integer arithmetic with U256. To handle rounding, the fee distribution must distribute the full `block_fees` across validators without losing wei to truncation. Implementation strategy: distribute using `block_fees * stake / total_stake` for each validator, then allocate the remainder (due to integer truncation) to the validator with the highest stake. This guarantees the full fee pool is distributed each block.

### Comparison to Options Considered

| Option | Description | Chosen? | Why rejected |
|--------|------------|---------|-------------|
| A | Proposer keeps 100% of fees | ❌ | Creates feast-or-famine reward pattern; non-proposing validators earn nothing for 105s on a 21-set |
| B | Split equally among active set | ❌ | Ignores stake weight; a 1 MONEX validator earns the same as a 1,000 MONEX validator |
| **C** | **Split pro-rata by stake** | **✅** | Rewards commitment proportionally; all validators earn every block; no special proposer bonus needed |

---

**Related:** [Protocol](plans/V0.5.0/Protocol.md), [Genesis](plans/V0.5.0/Genesis.md), [Mempool](plans/V0.5.0/Mempool.md), [Validators](plans/V0.5.0/Validators.md)
