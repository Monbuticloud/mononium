//! State machine + mempool benchmarks: block apply, tx selection.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use primitive_types::U256;

use mononium_lib::core::account::{Account, Address};
use mononium_lib::core::block::{Block, BlockBody, BlockHeader};
use mononium_lib::core::state::StateMachine;
use mononium_lib::core::transaction::{Transaction, TxBody};
use mononium_lib::crypto::falcon::Falcon512Signature;
use mononium_lib::mempool::{Mempool, MempoolConfig};

fn make_tx(sender: &Address, nonce: u64, amount: u64) -> Transaction {
    Transaction {
        chain_id: 0,
        nonce,
        sender: *sender,
        fee: U256::from(1_000_000u64),
        body: TxBody::Transfer {
            recipient: Address::from([0xBBu8; 32]),
            amount: amount.into(),
        },
        signature: Falcon512Signature::from_bytes(&[0xCDu8; 809]).unwrap(),
    }
}

fn bench_block_apply_100_txs(c: &mut Criterion) {
    let alice = Address::from([0xAAu8; 32]);
    let state = StateMachine::new(vec![(
        alice,
        Account::new(U256::from(1_000_000_000_000u64)),
    )]);

    let txs: Vec<_> = (0..100).map(|i| make_tx(&alice, i as u64, 1)).collect();
    let block = Block {
        header: BlockHeader {
            height: 1,
            parent_hash: [0u8; 32],
            global_state_root: [0u8; 32],
            tx_root: [0u8; 32],
            timestamp: 1_700_000_000,
            proposer: alice,
            chain_id: 0,
            proposer_signature: Falcon512Signature::from_bytes(&[0xCDu8; 809]).unwrap(),
        },
        body: BlockBody { transactions: txs },
    };

    c.bench_function("block_apply_100_txs", |b| {
        b.iter(|| {
            let mut s = state.clone();
            let _receipt = s.apply_block(black_box(&block));
            black_box(s);
        })
    });
}

fn bench_mempool_insert_10000(c: &mut Criterion) {
    let cfg = MempoolConfig {
        max_size: 100_000,
        ttl: std::time::Duration::from_secs(600),
        min_fee: U256::zero(),
        per_sender_cap: 10_000,
    };
    let txs: Vec<Transaction> = (0..10_000)
        .map(|i| {
            let addr_bytes: [u8; 16] = (i as u128).to_le_bytes();
            let mut arr = [0u8; 32];
            arr[..16].copy_from_slice(&addr_bytes);
            make_tx(&Address::from(arr), 0, 1)
        })
        .collect();

    c.bench_function("mempool_insert_10000", |b| {
        b.iter(|| {
            let mut pool = Mempool::new(cfg.clone());
            for tx in &txs {
                pool.insert(tx.clone());
            }
            black_box(pool);
        })
    });
}

fn bench_mempool_select_500_from_10000(c: &mut Criterion) {
    let cfg = MempoolConfig {
        max_size: 100_000,
        ttl: std::time::Duration::from_secs(600),
        min_fee: U256::zero(),
        per_sender_cap: 10_000,
    };
    let mut pool = Mempool::new(cfg);
    for i in 0..10_000 {
        let addr_bytes: [u8; 16] = (i as u128).to_le_bytes();
        let mut arr = [0u8; 32];
        arr[..16].copy_from_slice(&addr_bytes);
        pool.insert(make_tx(&Address::from(arr), 0, 1));
    }

    c.bench_function("mempool_select_500_from_10000", |b| {
        b.iter(|| {
            let selected = pool.select(500);
            black_box(selected);
        })
    });
}

criterion_group!(
    state_benches,
    bench_block_apply_100_txs,
    bench_mempool_insert_10000,
    bench_mempool_select_500_from_10000,
);
criterion_main!(state_benches);
