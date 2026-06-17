# ADR-002: Validator Election

**Status:** Accepted

**Context:** The chain needs a mechanism to select validators from the staked pool. NPoS (Phragmén) is the long-term target but complex for V1.

**Decision:** Two-phase approach via DI:

- **V1:** Top-N by stake. Sort candidates, take top N. Simple, works for small sets.
- **V2+:** Full Phragmén election with nominators.

```rust
#[async_trait]
pub trait ValidatorElection: Send + Sync {
    async fn elect(&self, candidates: &[ValidatorCandidate], max: usize) -> Vec<ValidatorId>;
}
```

**Consequences:**

- V1 is trivial to implement (sort + take)
- Era 0 is "Open" mode — no stake required (bootstrap)
- Full decentralization deferred to V2
- DI means zero disruption switching to Phragmén
