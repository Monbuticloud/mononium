//! Network-layer benchmarks: SCALE encode/decode of gossip and sync messages,
//! and rolling batch-hash computation.
//!
//! These exercise the hot serialisation paths that fire on every block
//! production, gossip, and sync request/response.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use primitive_types::U256;

use mononium_lib::core::account::Address;
use mononium_lib::core::block::{Block, BlockBody, BlockHeader, CommitVote};
use mononium_lib::core::transaction::{Transaction, TxBody};
use mononium_lib::crypto::constants::FALCON_SIGNATURE_SIZE;
use mononium_lib::crypto::falcon::Falcon512Signature;
use mononium_lib::network::messages::{
    compute_batch_hash, BlockByHashRequest, BlockSyncRequest,
    BlockSyncResponse, EquivocationEvidence, GossipMessage, SyncDirection,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn dummy_sig() -> Falcon512Signature {
    Falcon512Signature::from_bytes(&[0xCDu8; FALCON_SIGNATURE_SIZE]).unwrap()
}

fn addr(b: u8) -> Address {
    Address::from([b; 32])
}

fn dummy_tx(nonce: u64) -> Transaction {
    Transaction {
        chain_id: 0,
        nonce,
        sender: addr(0x11),
        fee: U256::from(100),
        body: TxBody::Transfer {
            recipient: addr(0x22),
            amount: U256::from(500),
        },
        signature: dummy_sig(),
    }
}

fn dummy_block(tx_count: usize) -> Block {
    Block {
        header: BlockHeader {
            height: 42,
            parent_hash: [0xAA; 32],
            global_state_root: [0xBB; 32],
            tx_root: [0xCC; 32],
            timestamp: 1_700_000_000,
            proposer: addr(0xDD),
            chain_id: 0,
            proposer_signature: dummy_sig(),
        },
        body: BlockBody {
            transactions: (0..tx_count).map(|i| dummy_tx(i as u64)).collect(),
        },
    }
}

// ---------------------------------------------------------------------------
// GossipMessage SCALE encode
// ---------------------------------------------------------------------------

fn bench_scale_encode_txs(c: &mut Criterion) {
    let msg = GossipMessage::Txs((0..100).map(|i| dummy_tx(i)).collect());
    c.bench_function("scale_encode/gossip/100_txs", |b| {
        b.iter(|| {
            let encoded = parity_scale_codec::Encode::encode(black_box(&msg));
            black_box(encoded.len());
        })
    });
}

fn bench_scale_encode_block(c: &mut Criterion) {
    let msg = GossipMessage::Block(Box::new(dummy_block(100)));
    c.bench_function("scale_encode/gossip/block_100_txs", |b| {
        b.iter(|| {
            let encoded = parity_scale_codec::Encode::encode(black_box(&msg));
            black_box(encoded.len());
        })
    });
}

fn bench_scale_encode_vote(c: &mut Criterion) {
    let msg = GossipMessage::Vote(CommitVote {
        height: 42,
        block_hash: [0xEE; 32],
        validator: addr(0xFF),
        signature: dummy_sig(),
    });
    c.bench_function("scale_encode/gossip/vote", |b| {
        b.iter(|| {
            let encoded = parity_scale_codec::Encode::encode(black_box(&msg));
            black_box(encoded.len());
        })
    });
}

// ---------------------------------------------------------------------------
// GossipMessage SCALE decode
// ---------------------------------------------------------------------------

fn bench_scale_decode_block(c: &mut Criterion) {
    let msg = GossipMessage::Block(Box::new(dummy_block(100)));
    let encoded = parity_scale_codec::Encode::encode(&msg);

    c.bench_function("scale_decode/gossip/block_100_txs", |b| {
        b.iter(|| {
            let decoded: GossipMessage =
                parity_scale_codec::Decode::decode(&mut &black_box(&encoded)[..]).unwrap();
            black_box(decoded);
        })
    });
}

fn bench_scale_decode_vote(c: &mut Criterion) {
    let msg = GossipMessage::Vote(CommitVote {
        height: 42,
        block_hash: [0xEE; 32],
        validator: addr(0xFF),
        signature: dummy_sig(),
    });
    let encoded = parity_scale_codec::Encode::encode(&msg);

    c.bench_function("scale_decode/gossip/vote", |b| {
        b.iter(|| {
            let decoded: GossipMessage =
                parity_scale_codec::Decode::decode(&mut &black_box(&encoded)[..]).unwrap();
            black_box(decoded);
        })
    });
}

// ---------------------------------------------------------------------------
// BlockSyncResponse SCALE encode (typical sync batch: 100 blocks)
// ---------------------------------------------------------------------------

fn bench_scale_encode_sync_response(c: &mut Criterion) {
    let resp = BlockSyncResponse {
        blocks: (0..100).map(|_| dummy_block(10)).collect(),
        highest_height: 5000,
        batch_hash: [0xAB; 32],
    };
    c.bench_function("scale_encode/sync_response/100_blocks", |b| {
        b.iter(|| {
            let encoded = parity_scale_codec::Encode::encode(black_box(&resp));
            black_box(encoded.len());
        })
    });
}

fn bench_scale_decode_sync_response(c: &mut Criterion) {
    let resp = BlockSyncResponse {
        blocks: (0..100).map(|_| dummy_block(10)).collect(),
        highest_height: 5000,
        batch_hash: [0xAB; 32],
    };
    let encoded = parity_scale_codec::Encode::encode(&resp);

    c.bench_function("scale_decode/sync_response/100_blocks", |b| {
        b.iter(|| {
            let decoded: BlockSyncResponse =
                parity_scale_codec::Decode::decode(&mut &black_box(&encoded)[..]).unwrap();
            black_box(decoded);
        })
    });
}

// ---------------------------------------------------------------------------
// BlockSyncRequest / BlockByHash encode
// ---------------------------------------------------------------------------

fn bench_scale_encode_sync_request(c: &mut Criterion) {
    let req = BlockSyncRequest {
        start_height: 1000,
        max_blocks: 500,
        direction: SyncDirection::Forward,
        known_block_hash: Some([0xAB; 32]),
    };
    c.bench_function("scale_encode/sync_request", |b| {
        b.iter(|| {
            let encoded = parity_scale_codec::Encode::encode(black_box(&req));
            black_box(encoded.len());
        })
    });
}

fn bench_scale_encode_by_hash_request(c: &mut Criterion) {
    let req = BlockByHashRequest {
        block_hashes: vec![[0xAB; 32]; 100],
    };
    c.bench_function("scale_encode/by_hash_request/100_hashes", |b| {
        b.iter(|| {
            let encoded = parity_scale_codec::Encode::encode(black_box(&req));
            black_box(encoded.len());
        })
    });
}

// ---------------------------------------------------------------------------
// compute_batch_hash (ADR-018)
// ---------------------------------------------------------------------------

fn bench_compute_batch_hash_10_blocks(c: &mut Criterion) {
    let genesis = [0x01; 32];
    let blocks: Vec<Block> = (0..10).map(|i| {
        let mut b = dummy_block(50);
        b.header.height = i;
        b
    }).collect();

    c.bench_function("compute_batch_hash/10_blocks", |b| {
        b.iter(|| {
            black_box(compute_batch_hash(black_box(&genesis), black_box(&blocks)));
        })
    });
}

fn bench_compute_batch_hash_100_blocks(c: &mut Criterion) {
    let genesis = [0x01; 32];
    let blocks: Vec<Block> = (0..100).map(|i| {
        let mut b = dummy_block(10);
        b.header.height = i;
        b
    }).collect();

    c.bench_function("compute_batch_hash/100_blocks", |b| {
        b.iter(|| {
            black_box(compute_batch_hash(black_box(&genesis), black_box(&blocks)));
        })
    });
}

// ---------------------------------------------------------------------------
// EquivocationEvidence SCALE encode
// ---------------------------------------------------------------------------

fn bench_scale_encode_equivocation(c: &mut Criterion) {
    let evidence = EquivocationEvidence {
        header_a: BlockHeader {
            height: 42, parent_hash: [0xAA; 32], global_state_root: [0; 32],
            tx_root: [0; 32], timestamp: 100, proposer: addr(0xDD), chain_id: 0,
            proposer_signature: dummy_sig(),
        },
        signature_a: [0x11u8; 666],
        header_b: BlockHeader {
            height: 43, parent_hash: [0xBB; 32], global_state_root: [0; 32],
            tx_root: [0; 32], timestamp: 101, proposer: addr(0xEE), chain_id: 0,
            proposer_signature: dummy_sig(),
        },
        signature_b: [0x22u8; 666],
        proposer: [0xFFu8; 32],
    };
    c.bench_function("scale_encode/equivocation_evidence", |b| {
        b.iter(|| {
            let encoded = parity_scale_codec::Encode::encode(black_box(&evidence));
            black_box(encoded.len());
        })
    });
}

// ---------------------------------------------------------------------------
// criterion registration
// ---------------------------------------------------------------------------

criterion_group!(
    network_benches,
    bench_scale_encode_txs,
    bench_scale_encode_block,
    bench_scale_encode_vote,
    bench_scale_decode_block,
    bench_scale_decode_vote,
    bench_scale_encode_sync_response,
    bench_scale_decode_sync_response,
    bench_scale_encode_sync_request,
    bench_scale_encode_by_hash_request,
    bench_compute_batch_hash_10_blocks,
    bench_compute_batch_hash_100_blocks,
    bench_scale_encode_equivocation,
);
criterion_main!(network_benches);
