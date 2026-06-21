//! Sync cursor — tracks the local node's sync progress.
//!
//! The [`SyncCursor`] records what portion of the chain the node has
//! fully verified.  It is persisted to disk so that the node can resume
//! from the last verified height after a restart.

use std::path::Path;
use serde::{Deserialize, Serialize};

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
        let _ = (to_height, to_hash);
        todo!()
    }

    /// Set the target height the node is syncing toward.
    pub fn set_target(&mut self, height: u64) {
        let _ = height;
        todo!()
    }

    /// Mark a range as currently being downloaded from a peer.
    pub fn set_pending(&mut self, range: HeightRange) {
        let _ = range;
        todo!()
    }

    /// Clear any pending range (e.g. on failure or completion).
    pub fn clear_pending(&mut self) {
        todo!()
    }

    // -- queries -------------------------------------------------------------

    /// How many blocks remain between the verified frontier and the target.
    #[must_use]
    pub fn gap(&self) -> u64 {
        todo!()
    }

    /// Whether the gap is large enough that a checkpoint sync is warranted.
    ///
    /// `era_length` is the number of blocks per era (720 in the current spec).
    #[must_use]
    pub fn needs_checkpoint(&self, era_length: u64) -> bool {
        let _ = era_length;
        todo!()
    }

    // -- persistence ---------------------------------------------------------

    /// Persist the cursor to `path` as JSON.
    ///
    /// # Errors
    /// - I/O errors from the filesystem.
    /// - JSON serialisation errors.
    pub fn save(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let _ = path;
        todo!()
    }

    /// Load a previously persisted cursor.
    ///
    /// If the file does not exist or cannot be parsed the cursor falls back
    /// to [`SyncCursor::new`] (full replay).
    #[must_use]
    pub fn load(path: &Path, genesis_hash: [u8; 32]) -> Self {
        let _ = (path, genesis_hash);
        todo!()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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
}
