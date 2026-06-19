# State Sharding

## Approach

**State sharding** (not execution sharding). All 21 validators validate all transactions, but each validator only stores a subset of the SMT state. This scales throughput without raising validator hardware requirements (~100 MB RAM target).

## Partitioning

Accounts and validators are assigned to a shard via:

```
let hash = blake3(address);           // one BLAKE3 call
shard_id = u16::from_le_bytes([hash[0], hash[1]]) % N_SHARDS   // first 2 bytes as u16
```

`N_SHARDS` is a `u16` (max 65,535). Deterministic. Even distribution regardless of address pattern (sequential, vanity, smart contract).

| Parameter  | Genesis value                                                               |
| ---------- | --------------------------------------------------------------------------- |
| `N_SHARDS` | 2                                                                           |
| Governance | Gov-voted increase, 24-era (24h) grace period for validators to pre-compute |

### Governance Voted Increase

Shard count increases use the **standard governance flow** (see [Governance.md](./Governance.md)), not a custom vote type. `IncreaseShards` is a `GovernanceAction` variant with special validation:

| Parameter    | Value                                                    |
| ------------ | -------------------------------------------------------- |
| Proposer     | Any staker (active or inactive)                          |
| Quorum       | ≥ 2/3 of total active stake (same as Governance.md)      |
| Threshold    | >50% of participating stake approves                     |
| Window       | Standard 7-era governance voting window                  |
| Grace period | `effective_era` must be ≥ `current_era + 24` (enforced by state machine) |

### Process

1. Any staker submits a standard governance proposal with `IncreaseShards { new_count, effective_era }` action
2. Stakers vote using the standard `Vote` transaction (see Governance.md)
3. Tally and execution follow standard governance flow (at era boundary after voting window closes)
4. Additional constraint: `effective_era >= current_era + 24` enforced at proposal validation time
5. Validators pre-compute the new shard layout during the 24-era grace period (see [Migration Details](#migration-details) below)
6. At `effective_era` boundary, migration triggers automatically
7. Any validator that did not complete migration (e.g., was offline during the grace period) enters migration sync at the effective era boundary — they request their newly assigned shard snapshots from peers, verify against the global state root, and resume participation once verified

The shard count is stored in the **genesis consensus config** (not hardcoded in the binary), allowing future governance votes to modify it.

### Migration Details

**Pre-computation (grace period):** After the governance vote passes (at era boundary), each validator:
1. Computes their new shard assignment based on the new `N_SHARDS` value
2. For each shard they **already store** that splits (because `N_SHARDS` increased), they partition their local SMT into the new shard boundaries
3. For shards they are **newly assigned** but do not yet store, they request the SMT snapshot from peers during the grace period — same mechanism as checkpoint sync (authenticated via the global state root at the vote era boundary)
4. Stores the pre-computed shard SMTs locally, ready for the migration

**Shard discovery:** Validators announce which shards they store via the Identify protocol (a `shards: Vec<u16>` field in peer metadata). At migration boundaries, the new shard assignments are deterministic (hash-based partitioning) — any validator can compute which shards any other validator stores based on their peer ID and the current `N_SHARDS`.

**Catch-up (offline validator):** A validator that was offline during the entire grace period (unlikely given 24 hours, but handled):
1. At the migration block, they discover they are assigned to shards they don't have
2. They request the shard SMT snapshot from any peer storing those shards — same mechanism as [Restart Sync](#restart-sync) (snapshot with SMT proof, verified against the global state root at the migration era boundary)
3. Once all assigned shards are verified, they resume consensus participation
4. If no peer can serve the snapshot, fall back to full replay from the last checkpoint before migration

## State Layout

Each shard maintains its own SMT. The global state root is a Merkle tree of shard SMT roots:

```
GlobalRoot = root_of([shard_0_root, shard_1_root])
```

When querying state for an account, the node:

1. Computes `shard_id = u16::from_le_bytes([blake3(address)[0], blake3(address)[1]]) % N_SHARDS`
2. If the shard is stored locally → read from local SMT
3. If the shard is not stored locally → request SMT proof from a peer who stores it

## Cross-Shard State Access

Validators fetch state proofs for shards they don't store from peers who do.

1. Validator computes target shard from address
2. Sends proof request to a peer storing that shard (discovered via identify protocol)
3. Peer returns SMT proof (key, value, Merkle path to shard root)
4. Validator verifies proof against the shard's SMT root in the global state root
5. If valid, uses the value for transaction execution

This is an **on-demand pull model** — no replication, no broadcast of state.

## Validator Storage

A validator stores the **full SMT** (all key-value pairs + all Merkle tree nodes) for their assigned shards. This enables serving SMT proofs to peers on demand without recomputation. The SMT overhead is negligible compared to the state data and block history.

For unassigned shards, they store only the shard root (32 bytes per shard), which is derived from the global state root.

### Restart Sync

When a validator restarts, they have two paths to rebuild their assigned shard state:

| Path                        | Trust model                                                                                                                              | Time    |
| --------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| **Snapshot sync** (default) | Peer provides authenticated SMT snapshot at last era boundary. Validator verifies against global state root. Trust-minimized (verified). | Minutes |
| **Full replay** (fallback)  | Re-download all blocks from genesis, re-execute all txs touching the validator's shards. Zero trust.                                     | Hours   |

**Snapshot sync** is the default. Era boundaries serve as natural checkpoint points — any peer can serve the shard SMT snapshot at that height, and the shard root committed in the global state root proves correctness. If snapshot sync fails (no peer responds, verification fails), the validator falls back to full replay.

## Relationship to Storage.md Tables

State sharding only affects the **mutable** (live state) tables:

- `accounts` — split by shard
- `validators` — split by shard
- `meta` — replicated across all shards (small, <1 KB)

The **append-only** (history) tables (`blocks`, `tx_body`, `tx_lookup`, `block_votes`) are **not sharded** — every validator stores full history for their configured retention period (full or compact mode).

## V1 Constraints

- 2 shards fixed in genesis config
- Migration to 4+ shards is designed but not implemented until the chain needs it
- The SMT is per-shard from V1 to avoid a future migration of the SMT itself
