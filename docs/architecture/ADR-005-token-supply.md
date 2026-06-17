# ADR-005: Token Supply Model

**Status:** Accepted

**Context:** The token supply model determines inflation, validator incentives, and launch fairness.

**Decision:** Two-phase approach via DI:

- **Dev (Localnet/Devnet/Testnet):** Fixed supply. All MONEX minted at genesis. No inflation.
- **Mainnet:** Mixed supply. Starts at 0, minted via capped inflation. Fair launch.

```rust
pub trait SupplyPolicy: Send + Sync {
    fn block_reward(&self, height: u64) -> U256;
}
```

**Consequences:**

- Dev networks are simple — no inflation math
- Mainnet is a fair launch (no pre-mine, no insider allocation)
- Bootstrap problem solved via genesis validators (era 0) + inflation
- DI makes the swap clean when transitioning from testnet → mainnet
