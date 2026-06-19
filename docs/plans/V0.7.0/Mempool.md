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

Out-of-order nonces are **buffered** — the mempool does not relay or select a tx until all lower nonces from the same sender have been received. This prevents nonce gaps from blocking block production.

- **Buffer expiry:** 10 minutes (matches mempool TTL)
- **Per-sender cap:** 30 buffered nonces — if exceeded, oldest buffered tx is dropped for that sender
- **Sender spam mitigation:** the cap prevents an adversary from filling the mempool with nonces from a single key

Once the missing lower nonce arrives, all buffered txs from that sender are released into the relay/selection pool in nonce order.

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
