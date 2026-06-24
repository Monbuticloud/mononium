//! Sync cursor — tracks the local node's sync progress.
//!
//! The [`SyncCursor`] records what portion of the chain the node has
//! fully verified.  It is persisted to disk so that the node can resume
//! from the last verified height after a restart.

use std::path::Path;
use std::time::Duration;

use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use parity_scale_codec::Encode;

use crate::core::block::Block;
use crate::network::messages::{
    compute_batch_hash, validate_sync_request, BlockSyncRequest, BlockSyncResponse, SyncDirection,
    MAX_SYNC_BLOCKS,
};
use crate::network::sync_protocol::{SyncRequest, SyncResponse};
use crate::network::{P2pEvent, P2pHandle};
use crate::storage::tables;
use crate::storage::StorageEngine;

// ---------------------------------------------------------------------------
// HeightRange — a contiguous range of blocks to sync from one peer
// ---------------------------------------------------------------------------

/// A contiguous block range assigned to a specific peer for downloading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeightRange {
    /// First block height (inclusive).
    pub start: u64,
    /// Last block height (exclusive — start ≤ height < end).
    pub end: u64,
    /// `PeerId` of the peer providing this range (serialised as a string).
    pub peer_id: String,
}

// ---------------------------------------------------------------------------
// SyncCursor
// ---------------------------------------------------------------------------

/// Tracks the verified frontier and pending sync ranges.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncCursor {
    /// Height of the last fully-verified block.
    pub last_verified_height: u64,
    /// Hash of the last fully-verified block.
    pub last_verified_hash: [u8; 32],
    /// Target height the node is trying to reach.
    pub target_height: u64,
    /// A range currently being downloaded (if any).
    pub pending_range: Option<HeightRange>,
}

impl SyncCursor {
    // -- construction --------------------------------------------------------

    /// Create a new cursor anchored at the genesis block.
    #[must_use]
    pub fn new(genesis_hash: [u8; 32]) -> Self {
        Self {
            last_verified_height: 0,
            last_verified_hash: genesis_hash,
            target_height: 0,
            pending_range: None,
        }
    }

    // -- mutation -----------------------------------------------------------

    /// Advance the cursor to `to_height` / `to_hash`.
    ///
    /// This is called after a batch of blocks has been fully verified.
    /// Panics if `to_height ≤ self.last_verified_height`.
    pub fn advance(&mut self, to_height: u64, to_hash: [u8; 32]) {
        assert!(
            to_height > self.last_verified_height,
            "advance requires height > current ({} ≤ {})",
            to_height,
            self.last_verified_height,
        );
        self.last_verified_height = to_height;
        self.last_verified_hash = to_hash;
    }

    /// Set the target height the node is syncing toward.
    pub fn set_target(&mut self, height: u64) {
        self.target_height = height;
    }

    /// Mark a range as currently being downloaded from a peer.
    pub fn set_pending(&mut self, range: HeightRange) {
        self.pending_range = Some(range);
    }

    /// Clear any pending range (e.g. on failure or completion).
    pub fn clear_pending(&mut self) {
        self.pending_range = None;
    }

    // -- queries -------------------------------------------------------------

    /// How many blocks remain between the verified frontier and the target.
    #[must_use]
    pub fn gap(&self) -> u64 {
        self.target_height.saturating_sub(self.last_verified_height)
    }

    /// Whether the gap is large enough that a checkpoint sync is warranted.
    ///
    /// `era_length` is the number of blocks per era (720 in the current spec).
    #[must_use]
    pub fn needs_checkpoint(&self, era_length: u64) -> bool {
        self.gap() >= 2 * era_length
    }

    // -- persistence ---------------------------------------------------------

    /// Persist the cursor to `path` as JSON.
    ///
    /// # Errors
    /// - I/O errors from the filesystem.
    /// - JSON serialisation errors.
    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(self)?;
        // ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load a previously persisted cursor.
    ///
    /// If the file does not exist or cannot be parsed the cursor falls back
    /// to [`SyncCursor::new`] (full replay).
    #[must_use]
    pub fn load(path: &Path, genesis_hash: [u8; 32]) -> Self {
        let json = match std::fs::read_to_string(path) {
            Ok(j) => j,
            Err(_) => return Self::new(genesis_hash),
        };
        match serde_json::from_str(&json) {
            Ok(cursor) => cursor,
            Err(_) => Self::new(genesis_hash),
        }
    }
}

// ---------------------------------------------------------------------------
// Sync loop — catch-up via Request-Response
// ---------------------------------------------------------------------------

/// Maximum time to wait for a sync response from a peer (per batch).
const SYNC_TIMEOUT: Duration = Duration::from_secs(10);

/// Number of consecutive failures before we abort the sync loop.
const MAX_CONSECUTIVE_FAILURES: usize = 10;

/// Run the sync catch-up loop.
///
/// 1. Loads or creates a [`SyncCursor`] from `cursor_path`.
/// 2. Contacts peers to discover the chain tip.
/// 3. Downloads and verifies blocks in batches (up to 100 per request).
/// 4. Stores verified blocks and persists the cursor periodically.
///
/// Returns `Ok(())` once the local node is caught up to the network tip.
/// When a peer fails to respond or returns invalid data, the loop tries
/// the next peer.  If all peers fail after `MAX_CONSECUTIVE_FAILURES`
/// attempts the function returns an error.
pub async fn run_sync_loop(
    p2p: &P2pHandle,
    storage: &dyn StorageEngine,
    genesis_hash: [u8; 32],
    cursor_path: &Path,
    era_length: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cursor = SyncCursor::load(cursor_path, genesis_hash);

    // --- Step 1: discover the network tip ---
    let peers = p2p.connected_peers().await;
    if peers.is_empty() {
        return Err("sync: no connected peers".into());
    }

    // Subscribe to events to receive SyncResponse
    let mut events = p2p.subscribe();

    // Send a tip-discovery request: ask for block at height 1, the response
    // includes `highest_height` from the peer's chain tip.
    let tip_req = SyncRequest::BlockSync(BlockSyncRequest {
        start_height: 1,
        max_blocks: 1,
        direction: SyncDirection::Forward,
        known_block_hash: None,
    });
    p2p.send_sync_request(peers[0], tip_req).await?;

    let tip_height = match wait_for_sync_response(&mut events, peers[0]).await {
        Some(resp) => {
            let SyncResponse::BlockSync(bsr) = resp else {
                return Err("sync: unexpected tip response type".into());
            };
            // Use the peer's reported highest_height
            bsr.highest_height
        }
        None => return Err("sync: no tip response from peer".into()),
    };

    if tip_height <= cursor.last_verified_height {
        info!(tip_height, "already caught up");
        return Ok(());
    }
    cursor.set_target(tip_height);

    // --- Step 2: check for checkpoint sync ---
    if cursor.needs_checkpoint(era_length) {
        info!(
            last_verified = cursor.last_verified_height,
            target = cursor.target_height,
            "gap is large, checkpoint sync would be ideal (falling back to block-by-block)"
        );
    }

    // --- Step 3: download blocks in batches ---
    let batch_size: u16 = 100.min(MAX_SYNC_BLOCKS);
    let mut consecutive_failures = 0;
    let mut peer_index = 0;

    while cursor.gap() > 0 {
        let peer = peers[peer_index % peers.len()];
        peer_index += 1;

        let start = cursor.last_verified_height + 1;
        let sync_req = SyncRequest::BlockSync(BlockSyncRequest {
            start_height: start,
            max_blocks: batch_size,
            direction: SyncDirection::Forward,
            known_block_hash: Some(cursor.last_verified_hash),
        });

        info!(start, batch_size, peer = %peer, "requesting sync batch");

        p2p.send_sync_request(peer, sync_req).await?;

        let response = match tokio::time::timeout(SYNC_TIMEOUT, wait_for_sync_response(&mut events, peer)).await
        {
            Ok(Some(resp)) => resp,
            Ok(None) => {
                warn!("sync: peer {peer} returned no response, trying next");
                consecutive_failures += 1;
                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    return Err("sync: too many consecutive failures".into());
                }
                continue;
            }
            Err(_) => {
                warn!("sync: timeout from peer {peer}, trying next");
                consecutive_failures += 1;
                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    return Err("sync: too many consecutive timeouts".into());
                }
                continue;
            }
        };

        let SyncResponse::BlockSync(bsr) = response else {
            warn!("sync: unexpected response type, trying next");
            consecutive_failures += 1;
            continue;
        };

        if bsr.blocks.is_empty() {
            warn!(height = start, "sync: empty batch (fork mismatch?)");
            consecutive_failures += 1;
            if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                return Err("sync: peer returned empty batches".into());
            }
            continue;
        }

        // Verify batch hash
        let local_hash = compute_batch_hash(&cursor.last_verified_hash, &bsr.blocks);
        if local_hash != bsr.batch_hash {
            warn!("sync: batch hash mismatch from {peer}, trying next");
            consecutive_failures += 1;
            if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                return Err("sync: batch hash mismatch".into());
            }
            continue;
        }

        // Verify parent chain continuity
        let mut prev_hash = cursor.last_verified_hash;
        let mut valid = true;
        for block in &bsr.blocks {
            if block.header.parent_hash != prev_hash {
                warn!(
                    height = block.header.height,
                    "sync: parent hash mismatch"
                );
                valid = false;
                break;
            }
            prev_hash = *blake3::hash(&block.header.encode()).as_bytes();
        }
        if !valid {
            consecutive_failures += 1;
            continue;
        }

        // Store blocks
        let last_block = bsr.blocks.last().expect("non-empty");
        for block in &bsr.blocks {
            let key = block.header.height.to_be_bytes();
            let encoded = parity_scale_codec::Encode::encode(block);
            if let Err(e) = storage.put(tables::BLOCKS, &key, &encoded) {
                error!("sync: failed to store block {}: {e}", block.header.height);
                return Err("sync: storage error".into());
            }
        }

        // Advance cursor
        let last_hash: [u8; 32] = *blake3::hash(&last_block.header.encode()).as_bytes();
        cursor.advance(last_block.header.height, last_hash);
        consecutive_failures = 0;

        // Persist after every batch
        if let Err(e) = cursor.save(cursor_path) {
            warn!("sync: failed to persist cursor: {e}");
        }

        info!(
            synced = cursor.last_verified_height,
            target = cursor.target_height,
            remaining = cursor.gap(),
            "sync batch complete"
        );
    }

    info!("sync complete at height {}", cursor.last_verified_height);
    Ok(())
}

/// Wait for a [`P2pEvent::SyncResponse`] from a specific peer.
async fn wait_for_sync_response(
    events: &mut broadcast::Receiver<P2pEvent>,
    peer: PeerId,
) -> Option<SyncResponse> {
    loop {
        match events.recv().await {
            Ok(P2pEvent::SyncResponse { peer: p, response }) if p == peer => {
                return Some(*response);
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("sync: receiver lagged by {n} messages");
            }
            Err(broadcast::error::RecvError::Closed) => return None,
            _ => continue,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    

    // -----------------------------------------------------------------------
    // construction
    // -----------------------------------------------------------------------

    #[test]
    fn test_sync_cursor_new_starts_at_genesis() {
        let genesis = [0xFE; 32];
        let cursor = SyncCursor::new(genesis);
        assert_eq!(cursor.last_verified_height, 0);
        assert_eq!(cursor.last_verified_hash, genesis);
    }

    // -----------------------------------------------------------------------
    // advance
    // -----------------------------------------------------------------------

    #[test]
    fn test_advance_moves_cursor_forward() {
        let genesis = [0x01; 32];
        let mut cursor = SyncCursor::new(genesis);
        cursor.advance(10, [0xAA; 32]);
        assert_eq!(cursor.last_verified_height, 10);
        assert_eq!(cursor.last_verified_hash, [0xAA; 32]);
    }

    // -----------------------------------------------------------------------
    // gap & needs_checkpoint
    // -----------------------------------------------------------------------

    #[test]
    fn test_gap_new_is_zero() {
        let cursor = SyncCursor::new([0; 32]);
        assert_eq!(cursor.gap(), 0);
    }

    #[test]
    fn test_gap_after_set_target() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.set_target(100);
        assert_eq!(cursor.gap(), 100);
    }

    #[test]
    fn test_gap_after_advance() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.set_target(200);
        cursor.advance(50, [0xAA; 32]);
        assert_eq!(cursor.gap(), 150);
    }

    #[test]
    fn test_needs_checkpoint_false_for_small_gap() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.set_target(500);
        // gap = 500, 2 × 720 = 1440, so 500 < 1440 → false
        assert!(!cursor.needs_checkpoint(720));
    }

    #[test]
    fn test_needs_checkpoint_true_for_large_gap() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.set_target(3000);
        // gap = 3000, 2 × 720 = 1440, so 3000 ≥ 1440 → true
        assert!(cursor.needs_checkpoint(720));
    }

    #[test]
    fn test_needs_checkpoint_edge_exactly_2_era() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.set_target(1440);
        // gap = 1440, 2 × 720 = 1440, exactly at threshold → checkpoint
        assert!(cursor.needs_checkpoint(720));
    }

    // -----------------------------------------------------------------------
    // pending range
    // -----------------------------------------------------------------------

    #[test]
    fn test_pending_none_after_new() {
        let cursor = SyncCursor::new([0; 32]);
        assert!(cursor.pending_range.is_none());
    }

    #[test]
    fn test_set_pending_stores_range() {
        let mut cursor = SyncCursor::new([0; 32]);
        let range = HeightRange {
            start: 1,
            end: 101,
            peer_id: "PeerA".into(),
        };
        cursor.set_pending(range.clone());
        assert_eq!(cursor.pending_range, Some(range));
    }

    #[test]
    fn test_clear_pending_removes_range() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.set_pending(HeightRange {
            start: 1,
            end: 101,
            peer_id: "PeerA".into(),
        });
        cursor.clear_pending();
        assert!(cursor.pending_range.is_none());
    }

    // -----------------------------------------------------------------------
    // persistence (save / load)
    // -----------------------------------------------------------------------

    #[test]
    fn test_save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cursor.json");

        // Write a known cursor
        let genesis = [0xAB; 32];
        {
            let mut cursor = SyncCursor::new(genesis);
            cursor.advance(50, [0xCD; 32]);
            cursor.set_target(200);
            cursor.set_pending(HeightRange {
                start: 51,
                end: 151,
                peer_id: "PeerA".into(),
            });
            cursor.save(&path).unwrap();
        }

        // Load it back
        let loaded = SyncCursor::load(&path, genesis);
        assert_eq!(loaded.last_verified_height, 50);
        assert_eq!(loaded.last_verified_hash, [0xCD; 32]);
        assert_eq!(loaded.target_height, 200);
        assert_eq!(
            loaded.pending_range,
            Some(HeightRange {
                start: 51,
                end: 151,
                peer_id: "PeerA".into(),
            })
        );

        // Clean up
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn test_load_nonexistent_returns_fresh_cursor() {
        let dir = std::env::temp_dir().join("mononium_sync_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("nonexistent.json");

        let genesis = [0x42; 32];
        let cursor = SyncCursor::load(&path, genesis);
        assert_eq!(cursor.last_verified_height, 0);
        assert_eq!(cursor.last_verified_hash, genesis);

        let _ = std::fs::remove_dir(&dir);
    }

    // -----------------------------------------------------------------------
    // invariant: advance must panic for non-increasing heights
    // -----------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "advance requires height > current")]
    fn test_advance_same_height_panics() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.advance(10, [0xAA; 32]);
        cursor.advance(10, [0xBB; 32]); // same height → panic
    }

    #[test]
    #[should_panic(expected = "advance requires height > current")]
    fn test_advance_lower_height_panics() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.advance(20, [0xAA; 32]);
        cursor.advance(15, [0xBB; 32]); // lower height → panic
    }

    #[test]
    #[should_panic(expected = "advance requires height > current")]
    fn test_advance_zero_panics_when_already_at_zero() {
        // advance(0, ...) should panic since 0 ≤ 0
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.advance(0, [0xFF; 32]);
    }

    // -----------------------------------------------------------------------
    // invariant: gap identity gap = target − last_verified
    // -----------------------------------------------------------------------

    #[test]
    fn test_gap_identity_after_multiple_advances() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.set_target(100);
        assert_eq!(cursor.gap(), 100); // 100 − 0

        cursor.advance(30, [0xAA; 32]);
        assert_eq!(cursor.gap(), 70);  // 100 − 30

        cursor.advance(80, [0xBB; 32]);
        assert_eq!(cursor.gap(), 20);  // 100 − 80

        cursor.advance(100, [0xCC; 32]);
        assert_eq!(cursor.gap(), 0);   // 100 − 100
    }

    #[test]
    fn test_gap_without_target_is_zero() {
        // set_target was never called, so target_height defaults to 0
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.advance(10, [0xAA; 32]);
        // Though last_verified = 10, target = 0, so saturating_sub → 0
        assert_eq!(cursor.gap(), 0);
    }

    // -----------------------------------------------------------------------
    // needs_checkpoint edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_needs_checkpoint_exactly_at_threshold_for_various_era_lengths() {
        let check = |gap, era_len, expected| {
            let mut cursor = SyncCursor::new([0; 32]);
            cursor.set_target(gap);
            assert_eq!(cursor.needs_checkpoint(era_len), expected,
                "gap={gap} era_len={era_len}");
        };
        check(0,   720, false);
        check(1,   720, false);
        check(1439, 720, false);
        check(1440, 720, true);  // 2 × 720
        check(1441, 720, true);
        check(0,   100, false);
        check(200, 100, true);   // 2 × 100
        check(199, 100, false);
    }

    #[test]
    fn test_needs_checkpoint_with_narrow_era_length() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.set_target(1);
        // 2 × 1 = 2, gap = 1, so false
        assert!(!cursor.needs_checkpoint(1));
    }

    #[test]
    fn test_needs_checkpoint_after_advance_reduces_gap() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.set_target(3000);
        assert!(cursor.needs_checkpoint(720));
        cursor.advance(2000, [0xAA; 32]);
        // gap = 1000, still < 2 × 720 = 1440
        assert!(!cursor.needs_checkpoint(720));
    }

    // -----------------------------------------------------------------------
    // pending range invariants
    // -----------------------------------------------------------------------

    #[test]
    fn test_set_pending_overwrites_previous() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.set_pending(HeightRange { start: 1, end: 10, peer_id: "A".into() });
        cursor.set_pending(HeightRange { start: 10, end: 20, peer_id: "B".into() });
        assert_eq!(cursor.pending_range.as_ref().unwrap().peer_id, "B");
        assert_eq!(cursor.pending_range.as_ref().unwrap().start, 10);
    }

    #[test]
    fn test_clear_pending_idempotent() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.clear_pending();           // nothing to clear
        assert!(cursor.pending_range.is_none());
        cursor.clear_pending();           // still nothing
        assert!(cursor.pending_range.is_none());
    }

    // -----------------------------------------------------------------------
    // persistence: load from corrupt file
    // -----------------------------------------------------------------------

    #[test]
    fn test_load_from_corrupt_file_returns_fresh_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("corrupt.json");
        std::fs::write(&path, "this is not valid json").unwrap();
        let genesis = [0x42; 32];
        let cursor = SyncCursor::load(&path, genesis);
        assert_eq!(cursor.last_verified_height, 0);
        assert_eq!(cursor.last_verified_hash, genesis);
    }

    // -----------------------------------------------------------------------
    // persistence: save error when parent missing (root dir is fine though)
    // -----------------------------------------------------------------------

    #[test]
    fn test_save_to_deep_path_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("subdir").join("cursor.json");
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.advance(5, [0xFF; 32]);
        cursor.save(&path).unwrap();
        assert!(path.exists());
        let loaded = SyncCursor::load(&path, [0; 32]);
        assert_eq!(loaded.last_verified_height, 5);
    }

    // -----------------------------------------------------------------------
    // set_target edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_set_target_reduces_gap_when_lower() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.set_target(100);
        cursor.advance(50, [0xAA; 32]);
        cursor.set_target(60); // lower target
        assert_eq!(cursor.gap(), 10); // 60 − 50
    }

    #[test]
    fn test_set_target_higher_increases_gap() {
        let mut cursor = SyncCursor::new([0; 32]);
        cursor.set_target(50);
        cursor.advance(30, [0xAA; 32]);
        cursor.set_target(100); // higher target
        assert_eq!(cursor.gap(), 70); // 100 − 30
    }

    // -----------------------------------------------------------------------
    // advance updates hash correctly
    // -----------------------------------------------------------------------

    #[test]
    fn test_advance_updates_hash_and_height_invariant() {
        let genesis = [0x01; 32];
        let mut cursor = SyncCursor::new(genesis);
        assert_eq!(cursor.last_verified_height, 0);
        assert_eq!(cursor.last_verified_hash, genesis);

        cursor.advance(7, [0x07; 32]);
        assert_eq!(cursor.last_verified_height, 7);
        assert_eq!(cursor.last_verified_hash, [0x07; 32]);

        cursor.advance(42, [0x2A; 32]);
        assert_eq!(cursor.last_verified_height, 42);
        assert_eq!(cursor.last_verified_hash, [0x2A; 32]);
    }

    // -----------------------------------------------------------------------
    // save/load roundtrip invariants for various states
    // -----------------------------------------------------------------------

    #[test]
    fn test_save_load_roundtrip_new_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cursor.json");
        let genesis = [0x99; 32];

        SyncCursor::new(genesis).save(&path).unwrap();
        let loaded = SyncCursor::load(&path, genesis);
        assert_eq!(loaded.last_verified_height, 0);
        assert_eq!(loaded.last_verified_hash, genesis);
        assert_eq!(loaded.target_height, 0);
        assert!(loaded.pending_range.is_none());
    }

    #[test]
    fn test_save_load_roundtrip_partial_sync() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cursor.json");
        let genesis = [0x99; 32];
        {
            let mut cursor = SyncCursor::new(genesis);
            cursor.advance(10, [0x0A; 32]);
            cursor.set_target(200);
            cursor.save(&path).unwrap();
        }
        let loaded = SyncCursor::load(&path, genesis);
        assert_eq!(loaded.last_verified_height, 10);
        assert_eq!(loaded.target_height, 200);
        assert!(loaded.pending_range.is_none());
    }

    // ── Property-based tests ───────────────────────────────────────

    proptest! {
        #[test]
        fn proptest_gap_identity(
            verified in any::<u64>(),
            target in any::<u64>(),
        ) {
            let mut cursor = SyncCursor::new([0; 32]);
            if verified > 0 {
                cursor.advance(verified, [0xAA; 32]);
            }
            cursor.set_target(target);
            let expected_gap = target.saturating_sub(verified);
            assert_eq!(cursor.gap(), expected_gap,
                "gap() target - last_verified: verified={verified} target={target}");
        }

        #[test]
        fn proptest_gap_decreases_after_advance(
            initial_target in 1u64..1_000_000u64,
            advance_to in 1u64..1_000_000u64,
        ) {
            let mut cursor = SyncCursor::new([0; 32]);
            cursor.set_target(initial_target);
            if advance_to > 0 && advance_to > cursor.last_verified_height {
                cursor.advance(advance_to, [0xBB; 32]);
            }
            if advance_to <= initial_target {
                let expected = initial_target - advance_to;
                assert_eq!(cursor.gap(), expected,
                    "gap decrease: initial_target={initial_target} advance_to={advance_to}");
            }
        }

        #[test]
        fn proptest_needs_checkpoint_monotonic(
            small_gap in 0u64..10_000u64,
            large_gap in 10_001u64..100_000u64,
            era_length in 1u64..1000u64,
        ) {
            let mut small = SyncCursor::new([0; 32]);
            small.set_target(small_gap);
            let mut large = SyncCursor::new([0; 32]);
            large.set_target(large_gap);
            if small.needs_checkpoint(era_length) {
                assert!(large.needs_checkpoint(era_length),
                    "needs_checkpoint not monotone: small={small_gap} large={large_gap} era={era_length}");
            }
        }

        #[test]
        fn proptest_needs_checkpoint_exact_threshold(
            gap in 0u64..10_000u64,
            era_length in 1u64..1000u64,
        ) {
            let mut cursor = SyncCursor::new([0; 32]);
            cursor.set_target(gap);
            let expected = gap >= 2 * era_length;
            assert_eq!(cursor.needs_checkpoint(era_length), expected,
                "needs_checkpoint mismatch at gap={gap} era={era_length}");
        }

        #[test]
        fn proptest_advance_monotonic(
            steps in proptest::collection::vec(1u64..10_000u64, 1..20usize),
        ) {
            let mut cursor = SyncCursor::new([0; 32]);
            let mut prev_height = 0u64;
            let mut cumulative = 0u64;
            for &step in &steps {
                cumulative += step;
                let hash_byte = (cumulative & 0xFF) as u8;
                cursor.advance(cumulative, [hash_byte; 32]);
                assert!(cursor.last_verified_height > prev_height,
                    "advance did not increase height: prev={prev_height}");
                assert_eq!(cursor.last_verified_height, cumulative);
                prev_height = cumulative;
            }
        }

        #[test]
        fn proptest_gap_invariant_after_mixed_ops(
            ops in proptest::collection::vec(
                proptest::prop_oneof![
                    (0u64..100_000u64).prop_map(|t| (0, t)),
                    (1u64..10_000u64).prop_map(|h| (1, h)),
                ],
                1..30usize,
            )
        ) {
            let mut cursor = SyncCursor::new([0; 32]);
            let mut verified = 0u64;
            let mut target = 0u64;

            for (op, val) in &ops {
                match *op {
                    0 => {
                        cursor.set_target(*val);
                        target = *val;
                    }
                    1 => {
                        let new_h = verified + *val;
                        cursor.advance(new_h, [0xCC; 32]);
                        verified = new_h;
                    }
                    _ => unreachable!(),
                }
                assert_eq!(cursor.gap(), target.saturating_sub(verified),
                    "invariant broken: verified={verified} target={target}");
            }
        }

        #[test]
        fn proptest_pending_clear_roundtrip(
            start in any::<u64>(),
            end in any::<u64>(),
        ) {
            let mut cursor = SyncCursor::new([0; 32]);
            let end = end.max(start + 1);
            cursor.set_pending(HeightRange {
                start,
                end,
                peer_id: "test".into(),
            });
            assert!(cursor.pending_range.is_some());
            cursor.clear_pending();
            assert!(cursor.pending_range.is_none());
        }
    }
}
