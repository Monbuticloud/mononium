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
        let dir = std::env::temp_dir().join("mononium_sync_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("cursor.json");

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
}
