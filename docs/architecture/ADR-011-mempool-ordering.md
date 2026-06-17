# ADR-011: Mempool Ordering

**Status:** Accepted

**Context:** The mempool holds pending transactions. The proposer needs a deterministic ordering to select txs for block inclusion.

**Decision:** Priority by Tip → Time → Nonce.

| Priority | Field         | Order          | Why                |
| -------- | ------------- | -------------- | ------------------ |
| 1        | Tip           | Highest first  | Economic incentive |
| 2        | Time received | Earliest first | Fairness           |
| 3        | Nonce         | Lowest first   | Prevent nonce gaps |

```rust
pub struct MempoolConfig {
    pub max_size: usize,     // 10,000
    pub ttl: Duration,       // 10 minutes
    pub min_fee: U256,       // minimum fee to enter pool
}
```

**Consequences:**

- Simple, predictable ordering
- Fee market emerges naturally (higher tip = earlier inclusion)
- TTL prevents stale tx buildup
- No complex priority queue algorithm needed (BTreeMap suffices)
