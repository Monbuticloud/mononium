# ADR-018: Sync Batch Hash (Rolling Chain Continuity)

**Status:** Accepted

**Context:**

The sync protocol defined in [Network.md](../plans/V0.6.0/Network.md) downloads blocks in batches from peers using `BlockSyncRequest` / `BlockSyncResponse`. After receiving a batch, the syncing node runs full verification — checking parent_hash links, re-executing blocks, and comparing state roots.

This creates a problem for fork detection during sync. If two peers serve different chains at the same height range, the syncing node only discovers the mismatch after downloading and attempting to verify a large batch — a slow, expensive process that wastes bandwidth and CPU. On cheap VPS hardware with Falcon-512 verification, this is especially costly.

A lightweight pre-verification check is needed that:

1. Detects chain disagreement before full block verification
2. Costs a single BLAKE3 hash per block (negligible vs. Falcon-512 verification)
3. Works across batch boundaries and peer switches
4. Commits the peer to their view of chain history

**Decision:**

Add a **rolling batch hash** to `BlockSyncResponse`. The responding peer computes a cumulative BLAKE3 hash over the batch's block hashes in order:

```
batch_hash = blake3(genesis_hash)
for block in blocks:
    batch_hash = blake3(batch_hash || block.hash)
```

The syncing node:

1. Computes the same rolling hash locally as blocks arrive
2. Compares against the peer's `batch_hash` after the batch is received
3. If mismatch → peer is on a different fork → discard batch, try different peer
4. If match → proceed to full parent_hash chain verification

The hash is **per-batch**, resetting for each `BlockSyncRequest`. This avoids accumulator state management across peer switches or disconnections. After a disconnection, the next batch starts from `last_verified_height + 1` with a fresh hash anchored to genesis.

**`BlockSyncResponse` update:**

```rust
struct BlockSyncResponse {
    pub blocks: Vec<Block>,
    pub highest_height: u64,
    pub batch_hash: [u8; 32],     // rolling BLAKE3 over peer's view of this batch
}
```

**Verification flow:**

```
1. Send BlockSyncRequest { start_height: N, max_blocks: 100 }
2. Receive BlockSyncResponse { blocks: B[N..N+99], batch_hash: H }
3. Compute local_hash = rolling_blake3(genesis, B[N..N+99])
4. If local_hash != H → discard, retry from different peer
5. If local_hash == H → run full verification:
   a. For each block: parent_hash must match previous block's hash
   b. Apply block → state_root must match
   c. If any check fails → peer sent structurally valid chain but invalid state → blacklist
```

**Key properties:**

- **Cheap:** one BLAKE3 per block (~0.1μs vs ~5ms for Falcon verify)
- **Stateless across peers:** each batch is self-verifying; no accumulator to reconcile
- **Fork-sensitive:** two peers on different forks at the same height produce different batch_hashes
- **Tamper-evident:** a malicious peer cannot forge a batch_hash without knowing the genesis hash (which is committed and gossiped at connection time)

**Consequences:**

**Positive:**

- Fast fork detection before expensive Falcon-512 verification
- Enables parallel downloads from multiple peers — compare batch_hashes to detect inconsistencies before committing
- Simple to implement: no consensus state needed, no signature aggregation
- Low overhead: 32 bytes per `BlockSyncResponse` (trivial bandwidth)

**Negative:**

- Both peers must agree on the genesis hash (they already do — mismatched genesis means connection rejection)
- Does not protect against a peer who serves a valid chain but withholds blocks (gap attack) — though parent_hash chain verification catches this
- Adds a round-trip dependency: the rolling hash can only be verified after the full batch is received (not streaming-friendly)

**Neutral:**

- The genesis hash anchor means the same block sequence always produces the same rolling hash across all honest peers — completely deterministic
- ADR updates the sync message format in [Network.md](../plans/V0.6.0/Network.md). Existing implementations must add the `batch_hash` field to `BlockSyncResponse`.

**Related:** [Network.md](../plans/V0.6.0/Network.md), [ADR-008](../architecture/ADR-008-p2p-networking.md)
