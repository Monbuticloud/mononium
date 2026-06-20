//! Sparse Merkle Tree (SMT) — 256-depth, BLAKE3 hashed, key-value state storage.
//!
//! Per the protocol spec:
//! - 256-depth binary Sparse Merkle Tree using BLAKE3
//! - Single tree with namespace prefixing (`0x00` accounts, `0x01` validators, `0x02` meta)
//! - Custom implementation — no external trie dependency
//!
//! # Default (empty) tree root
//!
//! For a 256-depth SMT, the default leaf hash is `[0u8; 32]` and each internal
//! node is `BLAKE3(left_child || right_child)`. The default root is computed
//! by hashing up 256 levels of default values.

use std::collections::HashMap;

/// Default empty-node hash (leaf level).
const EMPTY_HASH: [u8; 32] = [0u8; 32];

/// Maximum depth of the SMT (256 bits = 32 bytes).
const DEPTH: usize = 256;

// ---------------------------------------------------------------------------
// Sparse Merkle Tree
// ---------------------------------------------------------------------------

/// A 256-depth Sparse Merkle Tree using BLAKE3 for hashing.
///
/// Only stores non-default leaves. The root hash is computed lazily.
/// Keys are arbitrary byte slices — the trie hashes them with BLAKE3 to
/// determine the 256-bit leaf path.
///
/// # Panics
///
/// Panics if any internal invariant is broken (programming error).
#[derive(Debug, Clone)]
pub struct SparseMerkleTree {
    /// Non-default leaves: `key_hash → SCALE_bytes`.
    leaves: HashMap<[u8; 32], Vec<u8>>,
    /// Cached root hash for efficient repeated reads.
    cached_root: Option<[u8; 32]>,
}

impl SparseMerkleTree {
    /// Create a new empty SMT.
    #[must_use]
    pub fn new() -> Self {
        Self {
            leaves: HashMap::new(),
            cached_root: None,
        }
    }

    /// Return the current root hash.
    ///
    /// For an empty tree, this is the 256-level default hash (all empty
    /// subtrees hashed up to the root).
    #[must_use]
    pub fn root(&self) -> [u8; 32] {
        // For now, just return the empty default root.
        // Full computation is implemented in the next commit.
        self.cached_root.unwrap_or(EMPTY_HASH)
    }

    /// Insert a value at the given key.
    ///
    /// The value is stored as-is (caller should SCALE-encode before inserting).
    pub fn insert(&mut self, key: &[u8], value: Vec<u8>) {
        let key_hash = blake3::hash(key);
        let hash_bytes = *key_hash.as_bytes();
        self.leaves.insert(hash_bytes, value);
        self.cached_root = None; // invalidate
    }

    /// Retrieve a value by key, if it exists.
    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        let key_hash = blake3::hash(key);
        let hash_bytes = *key_hash.as_bytes();
        self.leaves.get(&hash_bytes).map(|v| v.as_slice())
    }
}

impl Default for SparseMerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Compute the 256-level default root hash.
    ///
    /// For an empty tree, every leaf is `[0u8; 32]` and each internal node
    /// is `blake3(left_child || right_child)`. After 256 levels of hashing
    /// the default up, we get the empty-tree root.
    fn compute_empty_root() -> [u8; 32] {
        let mut h = EMPTY_HASH;
        for _ in 0..DEPTH {
            let mut combined = [0u8; 64];
            combined[..32].copy_from_slice(&h);
            combined[32..].copy_from_slice(&h);
            let hash = blake3::hash(&combined);
            h = *hash.as_bytes();
        }
        h
    }

    #[test]
    fn test_empty_smt_has_correct_default_root() {
        let smt = SparseMerkleTree::new();
        let expected = compute_empty_root();
        assert_eq!(smt.root(), expected);
        // Also verify it's not just all zeros
        assert_ne!(smt.root(), [0u8; 32]);
    }
}
