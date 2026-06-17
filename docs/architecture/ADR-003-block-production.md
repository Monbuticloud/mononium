# ADR-003: Block Production

**Status:** Accepted

**Context:** Validators need to agree on who proposes each block. The proposer schedule must be deterministic and predictable.

**Decision:** Round-robin for V1, VRF leader election for V2+.

```rust
pub trait ProposerSelection: Send + Sync {
    fn select_proposer(&self, slot: u64, active_set: &[ValidatorId]) -> ValidatorId;
}
```

**Consequences:**

- Simple, deterministic, easy to debug
- Works well with Docker-based multi-validator testing (predictable order)
- VRF later adds fairness and sybil resistance for larger validator sets
