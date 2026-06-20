//! redb-based storage engine.
//!
//! Wraps `redb::Database` behind the [`StorageEngine`] trait. Uses
//! `TableDefinition<&[u8], &[u8]>` for all tables so keys and values are
//! opaque byte slices.

use std::path::Path;

use redb::{Database, ReadableTable, TableDefinition};

use crate::error::{LibError, Result};

use super::tables;

// ---------------------------------------------------------------------------
// Table definitions – one per logical table
// ---------------------------------------------------------------------------

macro_rules! table_defs {
    ($($name:ident => $str:literal),* $(,)?) => {
        $(
            const $name: TableDefinition<&[u8], &[u8]> =
                TableDefinition::new($str);
        )*
    };
}

table_defs! {
    T_ACCOUNTS    => "accounts",
    T_BLOCKS      => "blocks",
    T_BLOCK_HASHES => "block_hashes",
    T_TXS         => "transactions",
    T_VOTES       => "votes",
    T_VALIDATORS  => "validators",
    T_META        => "meta",
}

/// Resolve a table name string → its compile-time `TableDefinition`.
fn resolve_table(name: &str) -> Result<TableDefinition<'static, &'static [u8], &'static [u8]>> {
    match name {
        tables::ACCOUNTS => Ok(T_ACCOUNTS),
        tables::BLOCKS => Ok(T_BLOCKS),
        tables::BLOCK_HASHES => Ok(T_BLOCK_HASHES),
        tables::TXS => Ok(T_TXS),
        tables::VOTES => Ok(T_VOTES),
        tables::VALIDATORS => Ok(T_VALIDATORS),
        tables::META => Ok(T_META),
        other => Err(LibError::Storage(format!("unknown table: {other}"))),
    }
}

// ---------------------------------------------------------------------------
// Helpers to convert redb errors
// ---------------------------------------------------------------------------

fn map_db_err(e: &redb::DatabaseError) -> LibError {
    LibError::Storage(format!("redb database: {e}"))
}

fn map_txn_err(e: &redb::TransactionError) -> LibError {
    LibError::Storage(format!("redb transaction: {e}"))
}

fn map_table_err(e: &redb::TableError) -> LibError {
    LibError::Storage(format!("redb table: {e}"))
}

fn map_store_err(e: &redb::StorageError) -> LibError {
    LibError::Storage(format!("redb storage: {e}"))
}

fn map_commit_err(e: &redb::CommitError) -> LibError {
    LibError::Storage(format!("redb commit: {e}"))
}

// ---------------------------------------------------------------------------
// RedbEngine
// ---------------------------------------------------------------------------

/// A redb-backed storage engine.
///
/// Each `put` / `delete` call opens and commits its own write transaction.
/// This is safe for single-writer use and correct for the prototype phase;
/// a batched-transaction wrapper can be added later for block-apply
/// performance.
pub struct RedbEngine {
    db: Database,
}

impl RedbEngine {
    /// Open or create the database, ensuring all tables exist.
    fn ensure_tables(db: &Database) -> Result<()> {
        let txn = db.begin_write().map_err(|e| map_txn_err(&e))?;
        for table_name in tables::ALL_TABLES {
            let def = resolve_table(table_name)?;
            txn.open_table(def).map_err(|e| map_table_err(&e))?;
        }
        txn.commit().map_err(|e| map_commit_err(&e))?;
        Ok(())
    }
}

impl super::StorageEngine for RedbEngine {
    fn open(path: &Path) -> Result<Self> {
        let db = Database::create(path).map_err(|e| map_db_err(&e))?;
        Self::ensure_tables(&db)?;
        Ok(Self { db })
    }

    fn put(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()> {
        let def = resolve_table(table)?;
        let txn = self.db.begin_write().map_err(|e| map_txn_err(&e))?;
        {
            let mut t = txn.open_table(def).map_err(|e| map_table_err(&e))?;
            t.insert(key, value).map_err(|e| map_store_err(&e))?;
        }
        txn.commit().map_err(|e| map_commit_err(&e))?;
        Ok(())
    }

    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>> {
        let def = resolve_table(table)?;
        let txn = self.db.begin_read().map_err(|e| map_txn_err(&e))?;
        let t = txn.open_table(def).map_err(|e| map_table_err(&e))?;
        let val = t.get(key).map_err(|e| map_store_err(&e))?;
        Ok(val.map(|v| v.value().to_vec()))
    }

    fn delete(&self, table: &str, key: &[u8]) -> Result<()> {
        let def = resolve_table(table)?;
        let txn = self.db.begin_write().map_err(|e| map_txn_err(&e))?;
        {
            let mut t = txn.open_table(def).map_err(|e| map_table_err(&e))?;
            t.remove(key).map_err(|e| map_store_err(&e))?;
        }
        txn.commit().map_err(|e| map_commit_err(&e))?;
        Ok(())
    }

    fn list_keys(&self, table: &str) -> Result<Vec<Vec<u8>>> {
        let def = resolve_table(table)?;
        let txn = self.db.begin_read().map_err(|e| map_txn_err(&e))?;
        let t = txn.open_table(def).map_err(|e| map_table_err(&e))?;
        let mut keys = Vec::new();
        for item in t.iter().map_err(|e| map_store_err(&e))? {
            let (key, _) = item.map_err(|e| map_store_err(&e))?;
            keys.push(key.value().to_vec());
        }
        Ok(keys)
    }
}
