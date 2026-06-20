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

use primitive_types::U256;
use std::collections::HashMap;

/// Default empty-node hash (leaf level).
const EMPTY_HASH: [u8; 32] = [0u8; 32];

/// Maximum depth of the SMT (256 bits = 32 bytes).
const DEPTH: usize = 256;

// ---------------------------------------------------------------------------
// Trie trait (per protocol spec)
// ---------------------------------------------------------------------------

/// A key-value trie capable of producing a Merkle root.
///
/// This is the abstraction that the state machine uses for storage.
/// The [`SparseMerkleTree`] is the V1 implementation.
pub trait Trie {
    /// Retrieve a value by key, if it exists.
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    /// Insert a value at the given key.
    fn insert(&mut self, key: &[u8], value: Vec<u8>);
    /// Return the Merkle root hash.
    fn root(&mut self) -> [u8; 32];
    /// Generate a Merkle proof for the given key (future light clients).
    fn prove(&self, _key: &[u8]) -> Vec<u8> {
        todo!() // placeholder for future use
    }
}

// ---------------------------------------------------------------------------
// Default hash precomputation
// ---------------------------------------------------------------------------

/// Precompute the default hash at each depth.
///
/// `defaults[d]` = hash of a subtree of depth `d` where all leaves are empty.
/// - `defaults[0]` = EMPTY_HASH (leaf level)
/// - `defaults[d]` = BLAKE3(defaults[d-1] || defaults[d-1])
fn compute_defaults() -> [[u8; 32]; DEPTH + 1] {
    let mut defaults = [[0u8; 32]; DEPTH + 1];
    defaults[0] = EMPTY_HASH;
    for d in 1..=DEPTH {
        let mut combined = [0u8; 64];
        combined[..32].copy_from_slice(&defaults[d - 1]);
        combined[32..].copy_from_slice(&defaults[d - 1]);
        let hash = blake3::hash(&combined);
        defaults[d] = *hash.as_bytes();
    }
    defaults
}

/// Return the precomputed defaults (lazily computed once).
fn defaults() -> &'static [[u8; 32]; DEPTH + 1] {
    static DEFAULTS: std::sync::LazyLock<[[u8; 32]; DEPTH + 1]> =
        std::sync::LazyLock::new(compute_defaults);
    &DEFAULTS
}

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
    /// Cached root hash after last mutation.
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
    pub fn root(&mut self) -> [u8; 32] {
        if let Some(cached) = self.cached_root {
            return cached;
        }
        if self.leaves.is_empty() {
            return defaults()[DEPTH];
        }
        let root = self.compute_root();
        self.cached_root = Some(root);
        root
    }

    /// Insert a value at the given key.
    ///
    /// The value is stored as-is (caller should SCALE-encode before inserting).
    pub fn insert(&mut self, key: &[u8], value: Vec<u8>) {
        let key_hash = *blake3::hash(key).as_bytes();
        self.leaves.insert(key_hash, value);
        self.cached_root = None; // invalidate
    }

    /// Retrieve a value by key, if it exists.
    #[must_use]
    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        let key_hash = *blake3::hash(key).as_bytes();
        self.leaves.get(&key_hash).map(|v| v.as_slice())
    }

    /// Internal root computation by walking up from all leaves.
    fn compute_root(&self) -> [u8; 32] {
        let defs = defaults();

        // Convert leaf positions to U256 for bitwise operations
        let mut nodes: HashMap<U256, [u8; 32]> = HashMap::new();
        for (key_hash, value) in &self.leaves {
            let pos = U256::from_big_endian(key_hash);
            let leaf_hash = *blake3::hash(value).as_bytes();
            nodes.insert(pos, leaf_hash);
        }

        // Walk up from depth 0 (leaf) to depth 255 (just below root)
        for depth in 0..DEPTH {
            let mut parents: HashMap<U256, [u8; 32]> = HashMap::new();

            for (&pos, &hash) in &nodes {
                let sibling_pos = pos ^ U256::one();
                let parent_pos = pos >> 1;

                let sibling_hash = nodes.get(&sibling_pos).copied().unwrap_or(defs[depth]);

                // Left child has the lower (even) position
                let (left, right) = if pos < sibling_pos {
                    (hash, sibling_hash)
                } else {
                    (sibling_hash, hash)
                };

                let mut combined = [0u8; 64];
                combined[..32].copy_from_slice(&left);
                combined[32..].copy_from_slice(&right);
                let parent_hash = *blake3::hash(&combined).as_bytes();
                parents.insert(parent_pos, parent_hash);
            }

            nodes = parents;
        }

        // After DEPTH iterations we should have exactly one node (the root)
        debug_assert_eq!(nodes.len(), 1, "root computation must produce exactly one node");
        nodes.into_values().next().unwrap_or(defs[DEPTH])
    }
}

impl Default for SparseMerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

impl Trie for SparseMerkleTree {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        SparseMerkleTree::get(self, key).map(|s| s.to_vec())
    }

    fn insert(&mut self, key: &[u8], value: Vec<u8>) {
        SparseMerkleTree::insert(self, key, value);
    }

    fn root(&mut self) -> [u8; 32] {
        SparseMerkleTree::root(self)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_smt_has_correct_default_root() {
        let mut smt = SparseMerkleTree::new();
        let expected = defaults()[DEPTH];
        assert_eq!(smt.root(), expected);
        assert_ne!(smt.root(), [0u8; 32]);
    }

    #[test]
    fn test_insert_and_get_value() {
        let mut smt = SparseMerkleTree::new();
        smt.insert(b"alice", vec![1, 2, 3, 4]);
        assert_eq!(smt.get(b"alice"), Some(&[1, 2, 3, 4][..]));
    }

    #[test]
    fn test_get_unknown_key_returns_none() {
        let smt = SparseMerkleTree::new();
        assert_eq!(smt.get(b"unknown"), None);
    }

    #[test]
    fn test_insert_overwrites_value() {
        let mut smt = SparseMerkleTree::new();
        smt.insert(b"key", vec![1, 2, 3]);
        smt.insert(b"key", vec![4, 5, 6]);
        assert_eq!(smt.get(b"key"), Some(&[4, 5, 6][..]));
    }

    #[test]
    fn test_insert_changes_root() {
        let mut smt = SparseMerkleTree::new();
        let empty_root = smt.root();
        smt.insert(b"alice", vec![1, 2, 3, 4]);
        let after_root = smt.root();
        assert_ne!(after_root, empty_root);
    }

    #[test]
    fn test_deterministic_root_same_keys() {
        let mut a = SparseMerkleTree::new();
        let mut b = SparseMerkleTree::new();
        a.insert(b"x", vec![1]);
        b.insert(b"x", vec![1]);
        assert_eq!(a.root(), b.root());
    }

    #[test]
    fn test_different_values_different_roots() {
        let mut a = SparseMerkleTree::new();
        let mut b = SparseMerkleTree::new();
        a.insert(b"x", vec![1]);
        b.insert(b"x", vec![2]);
        assert_ne!(a.root(), b.root());
    }

    #[test]
    fn test_multiple_keys() {
        let mut smt = SparseMerkleTree::new();
        smt.insert(b"alice", vec![1]);
        smt.insert(b"bob", vec![2]);
        smt.insert(b"carol", vec![3]);
        assert_eq!(smt.get(b"alice"), Some(&[1][..]));
        assert_eq!(smt.get(b"bob"), Some(&[2][..]));
        assert_eq!(smt.get(b"carol"), Some(&[3][..]));
        assert_eq!(smt.get(b"dave"), None);
    }

    #[test]
    fn test_root_caching_repeated_calls_same_value() {
        let mut smt = SparseMerkleTree::new();
        smt.insert(b"key", vec![42]);
        let root_a = smt.root();
        let root_b = smt.root();
        assert_eq!(root_a, root_b);
    }

    #[test]
    fn test_large_value() {
        let mut smt = SparseMerkleTree::new();
        let large = vec![0xABu8; 10_000];
        smt.insert(b"big", large.clone());
        assert_eq!(smt.get(b"big"), Some(large.as_slice()));
    }

    // -----------------------------------------------------------------------
    // Trie trait tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_trie_trait_insert_get() {
        let mut trie: Box<dyn Trie> = Box::new(SparseMerkleTree::new());
        trie.insert(b"trait", vec![9, 9, 9]);
        let val = trie.get(b"trait");
        assert_eq!(val, Some(vec![9, 9, 9]));
    }

    #[test]
    fn test_trie_trait_root() {
        let mut trie: Box<dyn Trie> = Box::new(SparseMerkleTree::new());
        let empty = trie.root();
        trie.insert(b"a", vec![1]);
        let after = trie.root();
        assert_ne!(after, empty);
    }
}
