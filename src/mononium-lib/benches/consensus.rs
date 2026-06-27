//! Consensus engine benchmarks: block building, validation, tx root computation.
//!
//! These cover the synchronous hot paths in `consensus::engine`:
//!   - `compute_tx_root` at various transaction counts
//!   - `build_block` with empty and populated bodies
//!   - `validate_block` with and without Falcon-512 signature verification
//!   - `block_header_unsigned_payload`
//!
//! Each benchmark uses the same helper types as the existing `state.rs` and
//! `crypto.rs` benches so setup overhead is comparable across the suite.

use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use primitive_types::U256;

use mononium_lib::consensus::engine::{
    block_header_unsigned_payload, compute_tx_root, ConsensusEngine,
};
use mononium_lib::consensus::proposer::ProposerSchedule;
use mononium_lib::consensus::ConsensusConfig;
use mononium_lib::core::account::Address;
use mononium_lib::core::block::{Block, BlockBody, BlockHeader};
use mononium_lib::core::state::StateMachine;
use mononium_lib::core::transaction::{Transaction, TxBody};
use mononium_lib::crypto::constants::FALCON_SIGNATURE_SIZE;
use mononium_lib::crypto::falcon::{Falcon512, Falcon512Signature};
use mononium_lib::crypto::signature::SignatureScheme;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn dummy_sig() -> Falcon512Signature {
    Falcon512Signature::from_bytes(&[0xCDu8; FALCON_SIGNATURE_SIZE]).unwrap()
}

fn addr(b: u8) -> Address {
    Address::from([b; 32])
}

fn dummy_block(height: u64, parent_hash: [u8; 32]) -> Block {
    Block {
        header: BlockHeader {
            height,
            parent_hash,
            global_state_root: [0; 32],
            tx_root: [0; 32],
            timestamp: 1_700_000_000 + height,
            proposer: addr(0xAA),
            chain_id: 0,
            proposer_signature: dummy_sig(),
        },
        body: BlockBody {
            transactions: vec![],
        },
    }
}

fn make_tx(nonce: u64) -> Transaction {
    Transaction {
        chain_id: 0,
        nonce,
        sender: addr(0xBB),
        fee: U256::from(1_000_000),
        body: TxBody::Transfer {
            recipient: addr(0xCC),
            amount: U256::from(100),
        },
        signature: dummy_sig(),
    }
}

// ---------------------------------------------------------------------------
// compute_tx_root — varying transaction counts
// ---------------------------------------------------------------------------

fn bench_compute_tx_root_empty(c: &mut Criterion) {
    let body = BlockBody {
        transactions: vec![],
    };
    c.bench_function("compute_tx_root/0_txs", |b| {
        b.iter(|| black_box(compute_tx_root(black_box(&body))))
    });
}

fn bench_compute_tx_root_10(c: &mut Criterion) {
    let body = BlockBody {
        transactions: (0..10).map(make_tx).collect(),
    };
    c.bench_function("compute_tx_root/10_txs", |b| {
        b.iter(|| black_box(compute_tx_root(black_box(&body))))
    });
}

fn bench_compute_tx_root_100(c: &mut Criterion) {
    let body = BlockBody {
        transactions: (0..100).map(make_tx).collect(),
    };
    c.bench_function("compute_tx_root/100_txs", |b| {
        b.iter(|| black_box(compute_tx_root(black_box(&body))))
    });
}

fn bench_compute_tx_root_500(c: &mut Criterion) {
    let body = BlockBody {
        transactions: (0..500).map(make_tx).collect(),
    };
    c.bench_function("compute_tx_root/500_txs", |b| {
        b.iter(|| black_box(compute_tx_root(black_box(&body))))
    });
}

// ---------------------------------------------------------------------------
// build_block — empty body and 100-transfer body
// ---------------------------------------------------------------------------

fn bench_build_block_empty(c: &mut Criterion) {
    let engine = ConsensusEngine::new(ConsensusConfig::default());
    let parent = dummy_block(0, [0u8; 32]);
    let proposer = addr(0xAA);
    let mut state = StateMachine::new(vec![]);

    c.bench_function("build_block/0_txs", |b| {
        b.iter(|| {
            let block = engine.build_block(
                &mut state,
                vec![],
                black_box(&parent),
                &proposer,
                1_700_000_001,
                dummy_sig(),
            );
            black_box(block);
        })
    });
}

fn bench_build_block_100_txs(c: &mut Criterion) {
    let engine = ConsensusEngine::new(ConsensusConfig::default());
    let parent = dummy_block(0, [0u8; 32]);
    let proposer = addr(0xAA);
    let mut state = StateMachine::new(vec![]);
    let txs: Vec<_> = (0..100).map(make_tx).collect();

    c.bench_function("build_block/100_txs", |b| {
        b.iter(|| {
            let block = engine.build_block(
                &mut state,
                black_box(txs.clone()),
                black_box(&parent),
                &proposer,
                1_700_000_001,
                dummy_sig(),
            );
            black_box(block);
        })
    });
}

fn bench_build_block_500_txs(c: &mut Criterion) {
    let engine = ConsensusEngine::new(ConsensusConfig::default());
    let parent = dummy_block(0, [0u8; 32]);
    let proposer = addr(0xAA);
    let mut state = StateMachine::new(vec![]);
    let txs: Vec<_> = (0..500).map(make_tx).collect();

    c.bench_function("build_block/500_txs", |b| {
        b.iter(|| {
            let block = engine.build_block(
                &mut state,
                black_box(txs.clone()),
                black_box(&parent),
                &proposer,
                1_700_000_001,
                dummy_sig(),
            );
            black_box(block);
        })
    });
}

// ---------------------------------------------------------------------------
// validate_block — scheduling checks only, and full Falcon-512 verify
// ---------------------------------------------------------------------------

fn bench_validate_block_no_sig(c: &mut Criterion) {
    let engine = ConsensusEngine::new(ConsensusConfig::default());
    let proposer = addr(0xAA);
    let schedule = ProposerSchedule::new(vec![proposer], 1, 0);
    let tip = dummy_block(0, [0u8; 32]);
    let candidate = {
        let mut state = StateMachine::new(vec![]);
        engine.build_block(
            &mut state,
            vec![],
            &tip,
            &proposer,
            1_700_000_001,
            dummy_sig(),
        )
    };

    c.bench_function("validate_block/schedule_check", |b| {
        b.iter(|| {
            let ok = engine.validate_block(
                black_box(&candidate),
                black_box(&tip),
                black_box(&schedule),
                Duration::from_secs(5),
                None,
            );
            black_box(ok);
        })
    });
}

fn bench_validate_block_with_falcon_verify(c: &mut Criterion) {
    // Generate a real keypair and sign a block
    let seed = [0xABu8; 48];
    let kp = Falcon512::generate(&seed).unwrap();
    let pk = Falcon512::public_key(&kp);
    let proposer_addr = mononium_lib::crypto::address::derive_address(&pk.0);
    let schedule = ProposerSchedule::new(vec![proposer_addr], 1, 0);
    let tip = dummy_block(0, [0u8; 32]);

    // Build a candidate block and sign it
    let parent_hash =
        mononium_lib::crypto::hash::blake3_hash(&parity_scale_codec::Encode::encode(&tip.header));
    let unsigned = BlockHeader {
        height: 1,
        parent_hash,
        global_state_root: [0; 32],
        tx_root: [0; 32],
        timestamp: 1_700_000_001,
        proposer: proposer_addr,
        chain_id: 0,
        proposer_signature: Falcon512Signature::from_bytes(&[0u8; FALCON_SIGNATURE_SIZE]).unwrap(),
    };
    let payload = parity_scale_codec::Encode::encode(&unsigned);
    let sig = Falcon512::sign(&kp, &payload).unwrap();
    let mut signed = unsigned;
    signed.proposer_signature = sig;
    let candidate = Block {
        header: signed,
        body: BlockBody {
            transactions: vec![],
        },
    };

    let engine = ConsensusEngine::new(ConsensusConfig::default());

    c.bench_function("validate_block/with_falcon_verify", |b| {
        b.iter(|| {
            let ok = engine.validate_block(
                black_box(&candidate),
                black_box(&tip),
                black_box(&schedule),
                Duration::from_secs(5),
                Some(black_box(&pk)),
            );
            black_box(ok);
        })
    });
}

// ---------------------------------------------------------------------------
// block_header_unsigned_payload
// ---------------------------------------------------------------------------

fn bench_block_header_unsigned_payload(c: &mut Criterion) {
    let header = BlockHeader {
        height: 42,
        parent_hash: [0xAB; 32],
        global_state_root: [0xCD; 32],
        tx_root: [0xEF; 32],
        timestamp: 1_700_000_000,
        proposer: addr(0xAA),
        chain_id: 0,
        proposer_signature: dummy_sig(),
    };

    c.bench_function("block_header_unsigned_payload", |b| {
        b.iter(|| {
            let payload = block_header_unsigned_payload(black_box(&header));
            black_box(payload);
        })
    });
}

// ---------------------------------------------------------------------------
// criterion registration
// ---------------------------------------------------------------------------

criterion_group!(
    consensus_benches,
    bench_compute_tx_root_empty,
    bench_compute_tx_root_10,
    bench_compute_tx_root_100,
    bench_compute_tx_root_500,
    bench_build_block_empty,
    bench_build_block_100_txs,
    bench_build_block_500_txs,
    bench_validate_block_no_sig,
    bench_validate_block_with_falcon_verify,
    bench_block_header_unsigned_payload,
);
criterion_main!(consensus_benches);
