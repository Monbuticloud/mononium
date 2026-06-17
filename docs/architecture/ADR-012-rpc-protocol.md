# ADR-012: RPC Protocol

**Status:** Accepted

**Context:** The CLI wallet and future GUI need to communicate with the node. The protocol should support both simple queries and real-time subscriptions.

**Decision:** Hybrid — jsonrpsee (WebSocket) for mutations + subscriptions, REST (axum) for simple reads.

| Type          | Protocol                | Library   | Examples                         |
| ------------- | ----------------------- | --------- | -------------------------------- |
| Writes        | JSON-RPC 2.0            | jsonrpsee | send_tx, stake                   |
| Subscriptions | JSON-RPC over WebSocket | jsonrpsee | subscribe_blocks                 |
| Simple reads  | REST (HTTP GET)         | axum      | /balance/{addr}, /block/{height} |

**Consequences:**

- Blockchain standard (JSON-RPC) for writes
- REST for cacheable, idempotent reads
- WebSocket for real-time subscriptions
- CLI uses both: `mononium-cli wallet balance` = REST GET, `mononium-cli wallet transfer` = JSON-RPC POST
- Two servers to maintain, but each is simpler for its domain
