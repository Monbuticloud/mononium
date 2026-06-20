//! Cryptography constant values.
//!
//! Key sizes, signature sizes, and other crypto-domain constants.

// ---------------------------------------------------------------------------
// Falcon-512 sizes (FN-DSA-512, logn = 9)
// ---------------------------------------------------------------------------

/// Falcon-512 public key length in bytes.
pub const FALCON_PUBLIC_KEY_SIZE: usize = 897;

/// Falcon-512 private key length in bytes.
pub const FALCON_PRIVATE_KEY_SIZE: usize = 1281;

/// Falcon-512 (FN-DSA-512) constant-time signature length in bytes.
///
/// Note: this differs from the original Falcon compressed signature size
/// (666 bytes). The FN-DSA constant-time variant (used by `falcon-rs`)
/// produces 809-byte signatures for logn=9.
pub const FALCON_SIGNATURE_SIZE: usize = 809;

/// Falcon-512 seed entropy size in bytes.
pub const FALCON_SEED_SIZE: usize = 48;

/// Falcon-512 logn parameter (9 = FN-DSA-512).
pub const FALCON_LOGN: u32 = 9;

// ---------------------------------------------------------------------------
// Hashing
// ---------------------------------------------------------------------------

/// BLAKE3 output hash length in bytes.
pub const HASH_SIZE: usize = 32;

/// BLAKE3 checksum length for addresses (first 8 bytes of hash).
pub const CHECKSUM_SIZE: usize = 8;

// ---------------------------------------------------------------------------
// Address
// ---------------------------------------------------------------------------

/// Address length in bytes.
pub const ADDRESS_SIZE: usize = 32;
