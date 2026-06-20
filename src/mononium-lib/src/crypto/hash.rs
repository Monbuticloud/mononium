//! BLAKE3 hashing utilities.
//!
//! Wraps the [`blake3`] crate with Mononium-specific hashing patterns.
//! BLAKE3 is used for all protocol hashing: block hashes, state roots,
//! Merkle trees, address derivation, and Merkle proofs.

use crate::crypto::constants::HASH_SIZE;

/// Compute a BLAKE3-256 hash of the input data.
///
/// Returns a 32-byte digest. This is the primary hash function for all
/// protocol operations.
#[must_use]
pub fn blake3_hash(data: &[u8]) -> [u8; HASH_SIZE] {
    let hash = blake3::hash(data);
    *hash.as_bytes()
}

/// Compute the BLAKE3 hash of two concatenated byte slices.
///
/// Equivalent to `blake3_hash(&[a, b].concat())` but avoids the
/// intermediate allocation.
#[must_use]
pub fn blake3_hash_pair(a: &[u8], b: &[u8]) -> [u8; HASH_SIZE] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(a);
    hasher.update(b);
    *hasher.finalize().as_bytes()
}

/// Compute a keyed BLAKE3 hash.
///
/// The `key` must be exactly 32 bytes. Use for domain-separated hashing
/// where different protocol components use different keys.
///
/// # Panics
///
/// Panics if `key` is not exactly 32 bytes.
#[must_use]
pub fn blake3_keyed_hash(key: &[u8; HASH_SIZE], data: &[u8]) -> [u8; HASH_SIZE] {
    let mut hasher = blake3::Hasher::new_keyed(key);
    let hash = hasher.update(data).finalize();
    *hash.as_bytes()
}

/// Derive a sub-key using BLAKE3's key derivation mode.
///
/// `context` should be a unique, hard-coded string identifying the
/// protocol component (e.g., `"mononium.consensus"`).
#[must_use]
pub fn blake3_derive_key(context: &str, key_material: &[u8]) -> [u8; HASH_SIZE] {
    let mut hasher = blake3::Hasher::new_derive_key(context);
    let hash = hasher.update(key_material).finalize();
    *hash.as_bytes()
}

/// Compute a rolling BLAKE3 batch hash (per ADR-018).
///
/// Hashes a sequence of items where each item is a `&[u8]`.
/// Used for batch verification of block data.
#[must_use]
pub fn blake3_batch_hash<'a>(items: impl IntoIterator<Item = &'a [u8]>) -> [u8; HASH_SIZE] {
    let mut hasher = blake3::Hasher::new();
    for item in items {
        hasher.update(item);
    }
    *hasher.finalize().as_bytes()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blake3_hash_length() {
        let hash = blake3_hash(b"hello");
        assert_eq!(hash.len(), HASH_SIZE);
        assert_eq!(HASH_SIZE, 32);
    }

    #[test]
    fn test_blake3_hash_deterministic() {
        let a = blake3_hash(b"hello mononium");
        let b = blake3_hash(b"hello mononium");
        assert_eq!(a, b);
    }

    #[test]
    fn test_blake3_hash_different_inputs() {
        let a = blake3_hash(b"hello");
        let b = blake3_hash(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn test_blake3_hash_empty() {
        let hash = blake3_hash(b"");
        assert_eq!(hash.len(), HASH_SIZE);
    }

    #[test]
    fn test_blake3_hash_large_input() {
        let data = vec![0xABu8; 1_000_000];
        let hash = blake3_hash(&data);
        assert_eq!(hash.len(), HASH_SIZE);
    }

    // -----------------------------------------------------------------------
    // blake3_hash_pair
    // -----------------------------------------------------------------------

    #[test]
    fn test_hash_pair_equals_concat() {
        let a = b"hello ";
        let b = b"world";
        let pair_hash = blake3_hash_pair(a, b);
        let concat = [a.as_slice(), b.as_slice()].concat();
        let concat_hash = blake3_hash(&concat);
        assert_eq!(pair_hash, concat_hash);
    }

    #[test]
    fn test_hash_pair_deterministic() {
        let a = blake3_hash_pair(b"abc", b"def");
        let b = blake3_hash_pair(b"abc", b"def");
        assert_eq!(a, b);
    }

    #[test]
    fn test_hash_pair_order_matters() {
        let a = blake3_hash_pair(b"abc", b"def");
        let b = blake3_hash_pair(b"def", b"abc");
        assert_ne!(a, b);
    }

    // -----------------------------------------------------------------------
    // blake3_keyed_hash
    // -----------------------------------------------------------------------

    #[test]
    fn test_keyed_hash_deterministic() {
        let key = [0x42u8; 32];
        let a = blake3_keyed_hash(&key, b"mononium");
        let b = blake3_keyed_hash(&key, b"mononium");
        assert_eq!(a, b);
    }

    #[test]
    fn test_keyed_hash_different_key() {
        let key_a = [0x01u8; 32];
        let key_b = [0x02u8; 32];
        let a = blake3_keyed_hash(&key_a, b"mononium");
        let b = blake3_keyed_hash(&key_b, b"mononium");
        assert_ne!(a, b);
    }

    // -----------------------------------------------------------------------
    // blake3_derive_key
    // -----------------------------------------------------------------------

    #[test]
    fn test_derive_key_deterministic() {
        let a = blake3_derive_key("mononium.test", b"key material");
        let b = blake3_derive_key("mononium.test", b"key material");
        assert_eq!(a, b);
    }

    #[test]
    fn test_derive_key_different_context() {
        let a = blake3_derive_key("mononium.context-a", b"key material");
        let b = blake3_derive_key("mononium.context-b", b"key material");
        assert_ne!(a, b);
    }

    // -----------------------------------------------------------------------
    // blake3_batch_hash
    // -----------------------------------------------------------------------

    #[test]
    fn test_batch_hash_empty() {
        let hash = blake3_batch_hash(std::iter::empty::<&[u8]>());
        assert_eq!(hash, blake3_hash(b""));
    }

    #[test]
    fn test_batch_hash_single_item() {
        let items: [&[u8]; 1] = [b"single"];
        let hash = blake3_batch_hash(items.iter().copied());
        assert_eq!(hash, blake3_hash(b"single"));
    }

    #[test]
    fn test_batch_hash_multiple_items() {
        let items: [&[u8]; 3] = [b"a", b"b", b"c"];
        let hash = blake3_batch_hash(items.iter().copied());
        let expected = blake3_hash(b"abc");
        assert_eq!(hash, expected);
    }

    #[test]
    fn test_batch_hash_order_matters() {
        let items_abc: [&[u8]; 3] = [b"a", b"b", b"c"];
        let items_cba: [&[u8]; 3] = [b"c", b"b", b"a"];
        let hash_abc = blake3_batch_hash(items_abc.iter().copied());
        let hash_cba = blake3_batch_hash(items_cba.iter().copied());
        assert_ne!(hash_abc, hash_cba);
    }
}
