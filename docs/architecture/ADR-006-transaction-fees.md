# ADR-006: Transaction Fee Model

**Status:** Accepted

**Context:** Fees pay validators and prevent spam. The model must be simple but flexible enough for future congestion pricing.

**Decision:** Hybrid fee = flat component + per-byte component + optional tip.

```rust
pub trait FeePolicy: Send + Sync {
    fn calculate_fee(&self, tx: &Transaction) -> U256;
}
```

| Component | Purpose                              | Controls               |
| --------- | ------------------------------------ | ---------------------- |
| Flat fee  | Minimum tx cost, spam floor          | Protocol parameter     |
| Per-byte  | Proportional to state/storage impact | Protocol parameter     |
| Tip       | Priority inclusion                   | Sender (market-driven) |

**Consequences:**

- Predictable minimum cost per tx
- Larger txs pay proportionally more
- Tips create fee market during congestion
- DI-swappable for future models (e.g., EIP-1559 style)
