//! Storage abstraction and database engines.
//!
//! Defines the [`StorageEngine`] trait that all database backends must
//! implement (currently redb, with RocksDB planned via DI per ADR-007).

pub mod genesis;
pub mod redb;
pub mod tables;

use std::path::Path;

use crate::error::Result;

/// Generic key-value storage backend.
///
/// All keys and values are opaque byte vectors. Serialisation / deserialisation
/// is the caller's responsibility.
pub trait StorageEngine: Send + Sync {
    /// Open (or create) the database at the given path.
    fn open(path: &Path) -> Result<Self>
    where
        Self: Sized;

    /// Insert or overwrite a value.
    fn put(&self, table: &str, key: &[u8], value: &[u8]) -> Result<()>;

    /// Retrieve a value, or `None` if the key is absent.
    fn get(&self, table: &str, key: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Delete a key-value pair.
    fn delete(&self, table: &str, key: &[u8]) -> Result<()>;

    /// Return `true` if the key exists in the given table.
    fn exists(&self, table: &str, key: &[u8]) -> Result<bool> {
        self.get(table, key).map(|v| v.is_some())
    }

    /// Return all keys in a table (best-effort; may be expensive).
    fn list_keys(&self, table: &str) -> Result<Vec<Vec<u8>>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::redb::RedbEngine;
    use tempfile::TempDir;

    fn setup_engine() -> (TempDir, RedbEngine) {
        let dir = TempDir::with_prefix("mononium-test-").unwrap();
        let db_path = dir.path().join("test.redb");
        let engine = RedbEngine::open(&db_path).unwrap();
        (dir, engine)
    }

    // -----------------------------------------------------------------------
    // StorageEngine contract tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_put_get_roundtrip() {
        let (_dir, engine) = setup_engine();
        engine.put(tables::ACCOUNTS, b"alice", b"1000").unwrap();
        let val = engine
            .get(tables::ACCOUNTS, b"alice")
            .unwrap()
            .expect("value should exist");
        assert_eq!(val, b"1000");
    }

    #[test]
    fn test_get_missing_returns_none() {
        let (_dir, engine) = setup_engine();
        let val = engine.get(tables::ACCOUNTS, b"nobody").unwrap();
        assert!(val.is_none());
    }

    #[test]
    fn test_overwrite() {
        let (_dir, engine) = setup_engine();
        engine.put(tables::ACCOUNTS, b"key", b"v1").unwrap();
        engine.put(tables::ACCOUNTS, b"key", b"v2").unwrap();
        let val = engine
            .get(tables::ACCOUNTS, b"key")
            .unwrap()
            .expect("value should exist");
        assert_eq!(val, b"v2");
    }

    #[test]
    fn test_delete() {
        let (_dir, engine) = setup_engine();
        engine.put(tables::ACCOUNTS, b"tmp", b"data").unwrap();
        assert!(engine.exists(tables::ACCOUNTS, b"tmp").unwrap());
        engine.delete(tables::ACCOUNTS, b"tmp").unwrap();
        assert!(!engine.exists(tables::ACCOUNTS, b"tmp").unwrap());
    }

    #[test]
    fn test_separate_tables_dont_clash() {
        let (_dir, engine) = setup_engine();
        engine.put(tables::ACCOUNTS, b"key", b"account-data").unwrap();
        engine.put(tables::META, b"key", b"meta-data").unwrap();
        assert_eq!(
            engine.get(tables::ACCOUNTS, b"key").unwrap().unwrap(),
            b"account-data"
        );
        assert_eq!(
            engine.get(tables::META, b"key").unwrap().unwrap(),
            b"meta-data"
        );
    }

    #[test]
    fn test_list_keys() {
        let (_dir, engine) = setup_engine();
        engine.put(tables::ACCOUNTS, b"a", b"1").unwrap();
        engine.put(tables::ACCOUNTS, b"b", b"2").unwrap();
        engine.put(tables::ACCOUNTS, b"c", b"3").unwrap();
        let mut keys = engine.list_keys(tables::ACCOUNTS).unwrap();
        let mut expected: Vec<Vec<u8>> =
            vec![b"a".to_vec(), b"b".to_vec(), b"c".to_vec()];
        keys.sort();
        expected.sort();
        assert_eq!(keys, expected);
    }

    #[test]
    fn test_multiple_databases_isolated() {
        let dir1 = TempDir::with_prefix("mononium-test-").unwrap();
        let dir2 = TempDir::with_prefix("mononium-test-").unwrap();
        let engine1 = RedbEngine::open(&dir1.path().join("test.redb")).unwrap();
        let engine2 = RedbEngine::open(&dir2.path().join("test.redb")).unwrap();

        engine1.put(tables::ACCOUNTS, b"shared", b"db1").unwrap();
        engine2.put(tables::ACCOUNTS, b"shared", b"db2").unwrap();

        assert_eq!(
            engine1.get(tables::ACCOUNTS, b"shared").unwrap().unwrap(),
            b"db1"
        );
        assert_eq!(
            engine2.get(tables::ACCOUNTS, b"shared").unwrap().unwrap(),
            b"db2"
        );
    }
}
