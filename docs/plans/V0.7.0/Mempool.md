---
tags: [consensus, mempool, fees]
---

# Mempool

Transaction pool ordering:

| Priority | Field         | Order          | Why                |
| -------- | ------------- | -------------- | ------------------ |
| 1        | Tip           | Highest first  | Economic incentive |
| 2        | Time received | Earliest first | Fairness           |
| 3        | Nonce         | Lowest first   | Prevent nonce gaps |

## Nonce Buffering

Out-of-order nonces are **buffered** — the mempool holds higher-nonce txs until lower nonces arrive. However, higher-nonce txs **can still be selected** for block inclusion even if lower nonces are still buffered, as long as the block producer orders them correctly (see [Block Selection](#block-selection) below). This prevents a front-running vector where a low-fee tx at a lower nonce could delay a high-fee tx at a higher nonce from the same sender.

- **Buffer expiry:** 10 minutes (matches mempool TTL)
- **Per-sender cap:** 30 buffered nonces — if exceeded, oldest buffered tx is dropped for that sender
- **Sender spam mitigation:** the cap prevents an adversary from filling the mempool with nonces from a single key

Once the missing lower nonce arrives, all buffered txs from that sender are released into the relay/selection pool in nonce order.

### Block Selection

When constructing a block, the proposer:

1. Selects txs from the mempool by **global priority** (Tip → Time → Nonce)
2. If a high-priority tx at nonce N is selected, but the same sender has buffered lower nonces (< N), the proposer **must also include those lower nonces** in the same block, ordered before N
3. Within the same sender's txs in a block, enforces **strict nonce order**:
   ```
   Block ordering: [nonce 4 (low fee), nonce 5 (high fee)]
   State machine: nonce 4 executes first (succeeds), nonce 5 executes next (succeeds)
   ```
4. This prevents the front-running attack: a low-fee nonce 4 cannot delay a high-fee nonce 5 because the block producer includes both (nonce 4 to satisfy the gap, nonce 5 for the tip)

**Trade-off:** Favors fee-priority over strict nonce ordering across blocks, but maintains correctness within a block. The per-account rate limit (50 txs/block) bounds the complexity of gap-filling — at worst, a proposer includes a few low-fee txs to unlock high-fee txs from the same sender.

## Configuration

```rust
pub struct MempoolConfig {
    pub max_size: usize,              // 10,000
    pub ttl: Duration,                // 10 minutes
    pub min_fee: U256,                // local filter — see [NodeConfig](./NodeConfig.md#mempoolmin_fee)
    pub max_pending_per_sender: u32,  // 30 — per-sender nonce buffer cap
    pub max_tx_per_account_per_block: u32, // 50 — per-account rate limit
}
```

The `min_fee` is a local node policy. Each operator sets their own threshold (default: `0.0667 MONEX`). A tx below this fee is rejected from the local mempool but is still valid if included by another validator. This lets operators tune their own spam tolerance without affecting consensus.

## Block Hard Cap

Blocks are limited to **500 transactions OR 1 MB of SCALE-encoded block data**, whichever is hit first. The proposer selects the highest-priority txs from the mempool up to this limit.

With the per-account rate limit of 50 txs/block, at least 10 distinct accounts are needed to fill a block. Batch operations (e.g., distributing rewards to 200 recipients) will span ~4 blocks (20s). Wallet and tooling developers should account for this constraint.

## Rate Limit

Each account is limited to **50 transactions per block** at the mempool level. This is a local node policy (like `min_fee`), not a consensus rule — a validator can tighten their own limit. Combined with the per-tx deposit model, this creates two independent anti-spam layers: economic (capital locks) and congestion (block space).

---

**Related:** [Consensus](plans/V0.7.0/Consensus.md), [NodeConfig](plans/V0.7.0/NodeConfig.md)
