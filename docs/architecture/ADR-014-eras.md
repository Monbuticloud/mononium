# ADR-014: Eras and Bootstrap

**Status:** Accepted

**Context:** The chain needs a periodic mechanism to recalculate the active validator set. Era 0 needs special handling since no one has stake yet.

**Decision:**

- **Era length:** 720 blocks (~1 hour at 5s)
- **Era 0:** Open participation (no stake required)
- **Era 1+:** Standard Top-N election

```rust
pub enum ElectionMode {
    Open,                // Era 0 only
    TopN { max_validators: usize },  // Era 1+
}
```

**What changes at era boundaries:**

| Element              | Changes?               |
| -------------------- | ---------------------- |
| Active validator set | ✅ New election result |
| Proposer schedule    | ✅ Resets              |
| Block production     | ❌ Continuous          |
| Mempool              | ❌ Continuous          |
| Finality             | ❌ Continuous          |

**Consequences:**

- Era 0 solves the bootstrap problem without a pre-mine
- Short eras make testing validator turnover easy
- 720 blocks = fast enough to iterate, long enough to collect data
- No time-based scheduling (height-based only — deterministic)
