# ADR-009: Serialization Formats

**Status:** Accepted

**Context:** Different layers have different serialization needs. The wire protocol needs compact binary, RPC needs human-readable.

**Decision:** Hybrid — SCALE for consensus wire format, JSON for RPC.

| Layer                     | Format | Library            |
| ------------------------- | ------ | ------------------ |
| Blocks, txs, votes (wire) | SCALE  | parity-scale-codec |
| State storage (redb rows) | SCALE  | parity-scale-codec |
| RPC responses             | JSON   | serde + serde_json |

```rust
#[derive(Encode, Decode, Serialize, Deserialize)]
pub struct Transaction { ... }  // SCALE + JSON from one definition
```

**Consequences:**

- Compact wire format = efficient bandwidth
- Human-readable RPC = easy debugging with curl
- Dual derives keep everything in sync
- No protobuf compilation step
