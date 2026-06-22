//! Cryptographic benchmarks: Falcon-512 sign/verify, SMT operations, BLAKE3 throughput.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use mononium_lib::crypto::falcon::{Falcon512, Falcon512PublicKey};
use mononium_lib::crypto::hash;
use mononium_lib::crypto::signature::SignatureScheme;
use mononium_lib::crypto::trie::{SparseMerkleTree, NS_ACCOUNTS};

fn bench_falcon_sign(c: &mut Criterion) {
    let seed = [0xABu8; 48];
    let kp = Falcon512::generate(&seed).unwrap();
    let msg = b"benchmark message for falcon signing";

    c.bench_function("falcon_sign", |b| {
        b.iter(|| {
            let sig = Falcon512::sign(&kp, black_box(msg)).unwrap();
            black_box(sig);
        })
    });
}

fn bench_falcon_verify(c: &mut Criterion) {
    let seed = [0xABu8; 48];
    let kp = Falcon512::generate(&seed).unwrap();
    let pk = Falcon512PublicKey(kp.public_key_bytes());
    let msg = b"benchmark message for falcon verification";
    let sig = Falcon512::sign(&kp, msg).unwrap();

    c.bench_function("falcon_verify", |b| {
        b.iter(|| {
            let ok = Falcon512::verify(&pk, black_box(msg), black_box(&sig));
            assert!(ok);
        })
    });
}

fn bench_smt_insert(c: &mut Criterion) {
    let keys: Vec<[u8; 32]> = (0..1000)
        .map(|i: u64| {
            let mut k = [0u8; 32];
            k[..8].copy_from_slice(&i.to_be_bytes());
            k
        })
        .collect();

    c.bench_function("smt_insert_1000", |b| {
        b.iter(|| {
            let mut t = SparseMerkleTree::new();
            for (i, k) in keys.iter().enumerate() {
                let mut full_key = vec![NS_ACCOUNTS];
                full_key.extend_from_slice(k);
                t.insert(&full_key, vec![i as u8; 32]);
            }
            black_box(t);
        })
    });

    c.bench_function("smt_root_after_1000_inserts", |b| {
        let mut t = SparseMerkleTree::new();
        for (i, k) in keys.iter().enumerate() {
            let mut full_key = vec![NS_ACCOUNTS];
            full_key.extend_from_slice(k);
            t.insert(&full_key, vec![i as u8; 32]);
        }
        b.iter(|| {
            let root = t.root();
            black_box(root);
        })
    });
}

fn bench_blake3_throughput(c: &mut Criterion) {
    let data = vec![0xABu8; 1024 * 1024];

    c.bench_function("blake3_hash_1mb", |b| {
        b.iter(|| {
            let h = hash::blake3_hash(black_box(&data));
            black_box(h);
        })
    });

    let a: [u8; 32] = blake3::hash(&[0xAAu8; 32]).into();
    let b_val: [u8; 32] = blake3::hash(&[0xBBu8; 32]).into();
    c.bench_function("blake3_hash_pair", |b| {
        b.iter(|| {
            let h = hash::blake3_hash_pair(black_box(&a), black_box(&b_val));
            black_box(h);
        })
    });
}

criterion_group!(
    benches,
    bench_falcon_sign,
    bench_falcon_verify,
    bench_smt_insert,
    bench_blake3_throughput,
);
criterion_main!(benches);
