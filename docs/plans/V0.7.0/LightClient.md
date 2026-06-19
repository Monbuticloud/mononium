# Light Client (Phase 3+)

## Status

Planning stub. Full spec deferred to Phase 2 planning cycle.

## Goal

A minimal light client that can verify Mononium chain state without storing full state or replaying all blocks. Targets embedded/wallet use cases (mobile browser, CLI wallet, GUI wallet).

## Existing Affordances

These are already designed and will be leveraged by the light client:

| Affordance | Location | Purpose |
|---|---|---|
| **SMT `prove()`** | `mononium-rust-lib/src/crypto/trie.rs` | Generate Merkle proofs for any key (account balance, nonce, validator info) |
| **SMT `verify()`** | `mononium-rust-lib/src/crypto/trie.rs` | Verify a Merkle proof against a known state root |
| **`tx_root` Merkle tree** | Block header field, BLAKE3 over tx hashes | Prove a tx is included in a block without the full block body |
| **Checkpoints at era boundaries** | Storage.md, every 720 blocks | Trust anchor: checkpoint `global_state_root` is the verified root to anchor proofs against |
| **`validator_set` in CheckpointResponse** | Network.md sync protocol | Allows verifying BFT commits without replaying history |
| **CommitVotes stored per height** | `block_votes` table in redb | Prove finality of a given block height |
| **Header-only block storage** | `blocks` table stores `BlockEntry` (header + metadata) | Light client can sync headers without bodies |

## Trust Model

The light client trusts:

1. **Genesis hash** — obtained out-of-band (hardcoded in binary, published on website)
2. **Checkpoint state root** — verified via BFT commits from the trusted genesis chain
3. **Merkle proofs** — verified against the trusted state root

Everything else (blocks, transactions, intermediate state) is verified cryptography:

```
Trust anchor (genesis hash)
  → verify BFT commits at checkpoint → trust checkpoint state root
    → verify SMT proof against state root → trust balance, nonce, validator info
    → verify Merkle proof against tx_root → trust tx inclusion
```

## Minimum Viable Scope (Phase 3)

| Feature | Depends on |
|---|---|
| Header-only sync (download headers, verify parent_hash chain) | P2P sync protocol (Network.md) |
| Checkpoint download + BFT commit verification | CheckpointResponse + validator set (Network.md) |
| Balance proof verification | SMT prove/verify (Protocol.md) |
| Tx inclusion proof | tx_root Merkle tree (Protocol.md) |
| Finality verification | CommitVote chain (Consensus.md) |

## Constraints (Not Yet Designed)

- **Shard awareness**: Balance proofs need the per-shard SMT root, not just `global_state_root`. The light client needs to know which shard an address belongs to and request a proof for that shard's SMT.
- **Proof routing**: A light client connected to one peer needs to request proofs for shards that peer may not store. Cross-shard proof routing via P2P (discussed in StateSharding.md) applies to light clients too.
- **Checkpoint frequency**: Current checkpoints are every 720 blocks (~1 hour). A light client starting mid-era must sync headers from the last checkpoint + verify parent_hash chain. This is acceptable (720 headers = small).
- **Light client proof format**: The SMT `prove()` returns a `MerkleProof` struct — format TBD. Needs to be serializable (SCALE) for wire transfer.
- **No execution**: Light clients do NOT re-execute transactions. They verify state via proofs only. This means light clients trust the full node's execution — the proof only shows the result, not that the result was computed correctly from the inputs.

## Open Questions

- [ ] Light client P2P protocol: reuse existing sync protocol, or add dedicated request/response pair for proofs?
- [ ] Should light clients connect to full nodes only, or also to other light clients?
- [ ] Proof caching: can light clients cache verified proofs to reduce bandwidth?
- [ ] How does a light client discover which peer stores a given shard's SMT for proof requests?

---

**Related:** [Protocol](./Protocol.md) (SMT prove, tx_root), [Storage](./Storage.md) (checkpoints), [Network](./Network.md) (sync protocol), [StateSharding](./StateSharding.md) (cross-shard proof routing)
