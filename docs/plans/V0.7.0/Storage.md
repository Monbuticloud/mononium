---
tags: [storage, database, performance]
---

# Storage

## Database: redb

Mononium uses **redb** as its embedded database engine. redb is a pure-Rust, memory-mapped, ACID-compliant key-value store.

## Why redb

| Requirement          | How redb Meets It                                                |
| -------------------- | ---------------------------------------------------------------- |
| **Pure Rust**        | ✅ `cargo build`, no C++ compilation, no system deps             |
| **ACID**             | ✅ Full transactional guarantees (read-committed + serializable) |
| **Read performance** | ✅ Memory-mapped — reads are page-fault driven, instant          |
| **Embedded**         | ✅ No server process, links directly into mononiumd              |
| **Rust ergonomics**  | ✅ Native Rust API, serde integration, zero unsafe code          |
| **License**          | ✅ MIT — fully open source                                       |

### Known Redb Considerations

- **Write amplification:** redb is not append-only (unlike LSM trees). Internal page management can cause write amplification. This is acceptable for V1 with modest throughput (100-200 TPS) but should be monitored on cheap VPS with limited disk IOPS.
- **mmap + process crash:** redb uses mmap for reads. If the process crashes during a write transaction, the WAL ensures consistency on next open. However, OS-level mmap changes may not be fully synced in an uncontrolled shutdown. The ACID write transactions mitigate this, but operators should use a `--storage-sync-interval` option if they want to tune the durability-performance tradeoff.
- **mmap + memory pressure:** On a 2 GB VPS, the OS may page out parts of the memory-mapped database during high memory usage. This causes page faults on access — performance degrades but the node continues operating. Testing under memory pressure is recommended before production deployment.

## Column Families / Tables

redb's table abstraction maps cleanly to our data model:

### Mutable (Live State)

| Table        | Key                  | Value       | Notes                                      |
| ------------ | -------------------- | ----------- | ------------------------------------------ |
| `accounts`   | `[u8; 32]` (address) | `Account`   | balance, nonce, code_hash                  |
| `validators` | `[u8; 32]` (pubkey)  | `Validator` | stake, status                              |
| `meta`       | string key           | `Vec<u8>`   | Chain metadata, current height, state root |

```rust
struct Account {
    balance: U256,              // MOXX
    nonce: u64,
    code_hash: Option<[u8; 32]>,
}

struct Validator {
    stake: U256,                // MOXX
    status: ValidatorStatus,    // Active | Staking | Unstaking | Slashed
}

enum ValidatorStatus {
    Active,
    Staking,
    Unstaking { release_era: u64 },
    Slashed,
}
```

### Append-Only (History/Ledger)

| Table         | Key                        | Value             | Notes                                   |
| ------------- | -------------------------- | ----------------- | --------------------------------------- |
| `blocks`      | `u64` (height)             | `BlockEntry`      | Header only, canonical chain            |
| `tx_lookup`   | `[u8; 32]` (tx hash)       | `TxLocation`      | Maps tx hash → position in chain        |
| `tx_body`     | `(u64, u32)` (height, idx) | `Transaction`     | Full SCALE-encoded tx, indexed in-block |
| `block_votes` | `u64` (height)             | `Vec<CommitVote>` | All commit votes for a block            |

`BlockHeader` structure is defined in [Protocol](plans/V0.7.0/Protocol.md#Block-Structure).

```rust
struct BlockEntry {
    header: BlockHeader,  // defined in Protocol.md
    tx_count: u32,
    total_bytes: u32,           // sum of all tx + vote SCALE sizes
}

struct TxLocation {
    height: u64,
    index: u32,                  // position within the block
}
```

Key storage is documented in [Cryptography](plans/V0.7.0/Cryptography.md#Key-Storage).

## Storage Engine DI

```rust
/// Pluggable storage backend
pub trait StorageEngine: Send + Sync {
    type Table: Send + Sync;

    fn open(path: &Path) -> Result<Self>
    where Self: Sized;

    fn put(&self, table: &Self::Table, key: &[u8], value: &[u8]) -> Result<()>;
    fn get(&self, table: &Self::Table, key: &[u8]) -> Result<Option<Vec<u8>>>;
    fn delete(&self, table: &Self::Table, key: &[u8]) -> Result<()>;
}
```

### V1: redb implementation

```rust
use redb::{Database, TableDefinition, ReadableTable, WriteTransaction};

// redb table definitions (value is always SCALE-encoded bytes of the struct)
const ACCOUNTS:     TableDefinition<[u8; 32], &[u8]> = TableDefinition::new("accounts");
const BLOCKS:       TableDefinition<u64, &[u8]>     = TableDefinition::new("blocks");
const TX_LOOKUP:    TableDefinition<[u8; 32], &[u8]> = TableDefinition::new("tx_lookup");
const TX_BODY:      TableDefinition<&[u8], &[u8]>   = TableDefinition::new("tx_body");
const BLOCK_VOTES:  TableDefinition<u64, &[u8]>     = TableDefinition::new("block_votes");

pub struct RedbEngine {
    db: Database,
}

impl StorageEngine for RedbEngine {
    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let txn = self.db.begin_read()?;
        let t = txn.open_table(ACCOUNTS)?;
        Ok(t.get(key)?.map(|v| v.value().to_vec()))
    }
}
```

### Future: RocksDB (V2.0+)

If performance requirements outgrow redb (larger state, higher throughput), swap to RocksDB via the same `StorageEngine` trait. The DI pattern means zero changes to consensus or state machine code.

### Table Layout

| Separation            | Why                                                                        |
| --------------------- | -------------------------------------------------------------------------- |
| Mutable ≠ Append-only | State tables use write transactions; history is append-only with iterators |
| By content type       | accounts, validators, blocks, and txs have different access patterns       |

## Checkpoints

### Purpose

Full state snapshots taken at every **era boundary** (block height % 720 == 0). Enable fast sync for nodes that have been offline >2 eras — replay from checkpoint instead of genesis.

### Tables

| Table             | Key                         | Value            | Notes                                     |
| ----------------- | --------------------------- | ---------------- | ----------------------------------------- |
| `checkpoint_meta` | `u64` (era)                 | `CheckpointMeta` | Era metadata, state root, timestamp       |
| `checkpoint_data` | `&[u8]` (SCALE(era, shard)) | `&[u8]` (SCALE)  | Full per-shard state dump at era boundary |

```rust
struct CheckpointMeta {
    era: u64,
    height: u64,                    // era * 720
    global_state_root: [u8; 32],    // Merkle of per-shard SMT roots — trust anchor for verification
    timestamp: u64,
    num_shards: u16,                // shard count at this era
}

struct ShardSnapshot {
    accounts: Vec<Account>,
    validators: Vec<ValidatorEntry>,
}

struct ValidatorEntry {
    address: [u8; 32],
    data: Validator,
}
```

The `checkpoint_data` key is SCALE-encoded `(era, shard_id)`. Since shard count can increase via governance, no fixed table-per-shard — a single table with composite key works regardless of shard count.

### Retention Policy

| Mode        | Checkpoints Retained | `checkpoint_server` default |
| ----------- | -------------------- | --------------------------- |
| **Full**    | Latest 2             | `true` (serves last 2)      |
| **Compact** | None                 | `false`                     |
| **Archive** | All                  | `true` (serves all)         |

- New checkpoints written at each era boundary. Full mode: `checkpoint_era_N` overwrites `checkpoint_era_N-2` (oldest dropped). Archive mode: no overwrite.
- Compact mode skips checkpoint production entirely — saves write IO and disk.

### Serving Protocol

Hybrid: **P2P discovery + HTTP preferred, libp2p stream fallback.**

1. Syncing node discovers peers via gossipsub
2. Requests checkpoint at era N via libp2p request/response
3. Responding peer provides:
   - HTTP URL (if peer has `checkpoint_server: true`): `http://{peer_ip}:{rpc_port}/checkpoint/{era}`
   - OR direct libp2p stream if no HTTP
4. Syncing node prefers HTTP (resumable, faster), falls back to libp2p stream
5. On success: start block replay from era N+1

`checkpoint_server` field in `NodeConfig` (default varies by storage mode — see table above).

### Sync Threshold

```
gap = network_tip_height - local_tip_height

if gap > (2 * ERA_LENGTH) blocks:     # >2 eras behind
    request latest checkpoint
    verify global_state_root
    replay blocks from checkpoint height + 1
else:
    replay blocks from local tip + 1   # ≤2 eras, just catch up
```

Threshold: **2 eras** (1440 blocks, ~2 hours). Under that, block replay is faster than checkpoint download + rebuild.

### Verification Chain

1. **Download checkpoint** for era N (height H, `global_state_root` R, accounts list A)
2. **Rebuild SMT** from account list A across all shards → `computed_global_root`
3. **Assert** `computed_global_root == R`
4. **Fetch block header** at height H from trusted peers
5. **Assert** `block_header.global_state_root == R`
6. **Checkpoint is valid** — start replaying from H+1

If SMT rebuild fails or global state roots mismatch, discard checkpoint and retry from a different peer. If all peers fail, fall back to progressively older checkpoints (era N-1, N-2…). If none verify, use `--trusted-checkpoint <hash>` CLI override. Last resort: full replay from genesis.

### Size Estimate

| Component                  | Size (10M accounts) |
| -------------------------- | ------------------- |
| Accounts (10M × ~40 B)     | ~400 MB             |
| Validators (~1000 × ~80 B) | ~80 KB              |
| Meta                       | ~100 B              |
| **Total per checkpoint**   | **~400 MB**         |
| Peak (full mode, latest 2) | **~800 MB**         |

Written at every era boundary (720 blocks @ 5s = 1 per hour). Archive nodes retain all:

- Steady rate: ~400 MB/era ≈ **9.6 GB/day** ≈ **3.5 TB/year**
- Full mode peak: ~800 MB (latest 2 checkpoints)
- Archive mode growth is acceptable — archive nodes opt in to massive storage
- Full mode is the default and keeps storage flat at ~800 MB for checkpoints

### Implementation Notes

- Checkpoint production: after applying the block at the era boundary, the node **spawns a background task** to write checkpoint_meta + checkpoint_data for each shard. Block production continues immediately — the checkpoint write does not block the next block. The background task uses its own redb write transaction
- If a checkpoint write is still in progress when the next era boundary arrives, the previous background task is cancelled and a new one starts (the new checkpoint subsumes the old)
- Checkpoint reads: serve from the **last completed** checkpoint. If a background write is still in progress, the sync server returns the previous completed checkpoint. Once the new one finishes, data is atomically visible — use a **generation counter** in checkpoint_meta to distinguish, or allow readers to observe the new data naturally via redb's MVCC
- Checkpoint reading: separate redb read transaction, open checkpoint_meta + checkpoint_data tables
- SMT rebuild is CPU-intensive (10M accounts). Show progress via tracing. Estimate: 5-15 seconds on modern hardware.
- No compression on the SCALE bytes within redb — redb uses its own page-level management
- Future optimization: snappy-compressed checkpoint blobs before SCALE encoding inside redb (deferred)

## Design Decisions

- **Mutable** tables hold current live state (accounts, validators)
- **Append-only** tables hold the immutable ledger (blocks, txs, votes)
- State and ledger tables are physically separate in the same database
- No historical mutation — history is append-only and immutable
- Write transactions are atomic — state updates either fully commit or fully roll back

## Compression

- Append-only tables can be snapshotted/archived for long-term storage
- redb supports compression at the value level for large entries (>4 KB)
- The memory-mapped architecture handles page caching automatically
- No block-level compression — lean on libp2p's transport-layer snappy for wire savings

---

**Related:** [Architecture](plans/V0.7.0/Architecture.md), [Protocol](plans/V0.7.0/Protocol.md), [Cryptography](plans/V0.7.0/Cryptography.md)
