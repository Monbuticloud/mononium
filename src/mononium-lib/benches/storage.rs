//! Storage-engine benchmarks: redb put / get / list keys throughput.
//!
//! Each `put` call opens and commits its own write transaction (current
//! `RedbEngine` behaviour). These benchmarks measure the real cost of
//! single-row operations as the database grows, which is the dominant
//! storage pattern during block production and sync.

use std::sync::atomic::{AtomicU64, Ordering};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tempfile::TempDir;

use mononium_lib::storage::redb::RedbEngine;
use mononium_lib::storage::tables;
use mononium_lib::storage::StorageEngine;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn setup_db() -> (TempDir, RedbEngine) {
    let dir = TempDir::with_prefix("mononium-bench-storage-").unwrap();
    let engine = RedbEngine::open(&dir.path().join("bench.redb")).unwrap();
    (dir, engine)
}

/// Pre-populate a database with `count` mock blocks (each ~1 KB).
fn prepopulate_blocks(engine: &RedbEngine, count: u64) {
    let value = vec![0xABu8; 1024]; // ~1 KB payload
    for i in 0..count {
        let key = i.to_be_bytes();
        engine.put(tables::BLOCKS, &key, &value).unwrap();
    }
}

// ---------------------------------------------------------------------------
// Put throughput (writes unique keys on each iteration)
// ---------------------------------------------------------------------------

fn bench_storage_put_256b(c: &mut Criterion) {
    let (_dir, engine) = setup_db();
    let value = vec![0xABu8; 256];
    let counter = AtomicU64::new(0);

    c.bench_function("storage/put/256b", |b| {
        b.iter(|| {
            let i = counter.fetch_add(1, Ordering::Relaxed);
            engine
                .put(tables::ACCOUNTS, &i.to_be_bytes(), black_box(&value))
                .unwrap();
            black_box(());
        })
    });
}

fn bench_storage_put_1k(c: &mut Criterion) {
    let (_dir, engine) = setup_db();
    let value = vec![0xABu8; 1024];
    let counter = AtomicU64::new(0);

    c.bench_function("storage/put/1k", |b| {
        b.iter(|| {
            let i = counter.fetch_add(1, Ordering::Relaxed);
            engine
                .put(tables::BLOCKS, &i.to_be_bytes(), black_box(&value))
                .unwrap();
            black_box(());
        })
    });
}

fn bench_storage_put_10k(c: &mut Criterion) {
    let (_dir, engine) = setup_db();
    let value = vec![0xABu8; 10_240];
    let counter = AtomicU64::new(0);

    c.bench_function("storage/put/10k", |b| {
        b.iter(|| {
            let i = counter.fetch_add(1, Ordering::Relaxed);
            engine
                .put(tables::BLOCKS, &i.to_be_bytes(), black_box(&value))
                .unwrap();
            black_box(());
        })
    });
}

// ---------------------------------------------------------------------------
// Get throughput (reads existing data)
// ---------------------------------------------------------------------------

fn bench_storage_get_existing(c: &mut Criterion) {
    let (_dir, engine) = setup_db();
    let key = 42u64.to_be_bytes();
    let value = vec![0xABu8; 1024];
    engine.put(tables::BLOCKS, &key, &value).unwrap();

    c.bench_function("storage/get/existing_1k", |b| {
        b.iter(|| {
            let val = engine.get(tables::BLOCKS, black_box(&key)).unwrap();
            black_box(val);
        })
    });
}

fn bench_storage_get_missing(c: &mut Criterion) {
    let (_dir, engine) = setup_db();

    c.bench_function("storage/get/missing", |b| {
        b.iter(|| {
            let val = engine.get(tables::BLOCKS, black_box(b"nonexistent")).unwrap();
            black_box(val);
        })
    });
}

fn bench_storage_get_with_1000_blocks(c: &mut Criterion) {
    let (_dir, engine) = setup_db();
    prepopulate_blocks(&engine, 1000);
    let key = 500u64.to_be_bytes();

    c.bench_function("storage/get/1000_blocks_mid", |b| {
        b.iter(|| {
            let val = engine.get(tables::BLOCKS, black_box(&key)).unwrap();
            black_box(val);
        })
    });
}

fn bench_storage_get_with_10000_blocks(c: &mut Criterion) {
    let (_dir, engine) = setup_db();
    prepopulate_blocks(&engine, 10_000);
    let key = 5000u64.to_be_bytes();

    c.bench_function("storage/get/10000_blocks_mid", |b| {
        b.iter(|| {
            let val = engine.get(tables::BLOCKS, black_box(&key)).unwrap();
            black_box(val);
        })
    });
}

// ---------------------------------------------------------------------------
// List keys (full table scan)
// ---------------------------------------------------------------------------

fn bench_storage_list_keys_100(c: &mut Criterion) {
    let (_dir, engine) = setup_db();
    prepopulate_blocks(&engine, 100);

    c.bench_function("storage/list_keys/100_blocks", |b| {
        b.iter(|| {
            let keys = engine.list_keys(tables::BLOCKS).unwrap();
            black_box(keys.len());
        })
    });
}

fn bench_storage_list_keys_10000(c: &mut Criterion) {
    let (_dir, engine) = setup_db();
    prepopulate_blocks(&engine, 10_000);

    c.bench_function("storage/list_keys/10000_blocks", |b| {
        b.iter(|| {
            let keys = engine.list_keys(tables::BLOCKS).unwrap();
            black_box(keys.len());
        })
    });
}

// ---------------------------------------------------------------------------
// criterion registration
// ---------------------------------------------------------------------------

criterion_group!(
    storage_benches,
    bench_storage_put_256b,
    bench_storage_put_1k,
    bench_storage_put_10k,
    bench_storage_get_existing,
    bench_storage_get_missing,
    bench_storage_get_with_1000_blocks,
    bench_storage_get_with_10000_blocks,
    bench_storage_list_keys_100,
    bench_storage_list_keys_10000,
);
criterion_main!(storage_benches);
