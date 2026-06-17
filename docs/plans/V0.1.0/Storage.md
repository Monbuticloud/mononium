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

## Column Families / Tables

redb's table abstraction maps cleanly to our data model:

### Mutable (Live State)

| Table        | Key                  | Value                   | Notes                                      |
| ------------ | -------------------- | ----------------------- | ------------------------------------------ |
| `accounts`   | `[u8; 32]` (address) | `(U256, u64, [u8; 32])` | balance, nonce, code_hash                  |
| `validators` | `[u8; 32]` (pubkey)  | `(U256, u8)`            | stake, status (active/staking/unstaking)   |
| `meta`       | string key           | any                     | Chain metadata, current height, state root |

### Append-Only (History/Ledger)

| Table          | Key                  | Value                   | Notes                   |
| -------------- | -------------------- | ----------------------- | ----------------------- |
| `blocks`       | `u64` (height)       | Block header + tx count | Canonical chain         |
| `tx_by_hash`   | `[u8; 32]` (tx hash) | `(u64, u32, u32)`       | height → index → offset |
| `tx_by_height` | `(u64, u32)`         | Transaction bytes       | Order within block      |
| `votes`        | `(u64, u32)`         | Consensus votes         | Per-block commit votes  |

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

// Column families defined at compile time
const ACCOUNTS: TableDefinition<[u8; 32], [u8; 64]> = TableDefinition::new("accounts");
const BLOCKS: TableDefinition<u64, &[u8]> = TableDefinition::new("blocks");

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

---

**Related:** [Architecture](Architecture.md), [Protocol](Protocol.md)
