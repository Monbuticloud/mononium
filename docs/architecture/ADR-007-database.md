# ADR-007: Database Engine

**Status:** Accepted

**Context:** The chain needs an embedded database for state (mutable) and ledger (append-only). Must be open source, Rust-friendly, and run on Linux VPS.

**Decision:** redb for V1, with RocksDB as a future option via DI.

- **redb:** Pure Rust, ACID, memory-mapped, MIT license. Used for V1.
- **RocksDB:** Beefier option if redb hits limits at scale.

```rust
pub trait StorageEngine: Send + Sync {
    fn open(path: &Path) -> Result<Self> where Self: Sized;
    fn put(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()>;
    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>>;
}
```

**Consequences:**

- Zero C dependencies (pure Rust = fast builds)
- Memory-mapped reads are instant for balance lookups
- ACID transactions for atomic state updates
- Column families: accounts, validators, blocks, txs, votes
- Can swap to RocksDB later if needed without touching consensus
