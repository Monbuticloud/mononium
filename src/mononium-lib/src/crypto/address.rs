//! Address derivation from Falcon-512 public keys.
//!
//! Per the protocol spec and ADR-016:
//! ```text
//! Address = BLAKE3-256(Falcon-512 public key)[..32]
//! ```
//!
//! The [`Address`] type itself is defined in [`crate::core::account`].
//! This module provides the derivation function and helpers.

use crate::core::account::Address;
use crate::crypto::constants::FALCON_PUBLIC_KEY_SIZE;

/// Derive an on-chain [`Address`] from a Falcon-512 public key.
///
/// The address is the first 32 bytes of the BLAKE3 hash of the
/// 897-byte Falcon-512 public key:
///
/// ```text
/// address = BLAKE3-256(public_key_bytes)[..32]
/// ```
#[must_use]
pub fn derive_address(pubkey: &[u8; FALCON_PUBLIC_KEY_SIZE]) -> Address {
    let hash = blake3::hash(pubkey);
    let mut addr_bytes = [0u8; 32];
    addr_bytes.copy_from_slice(&hash.as_bytes()[..32]);
    Address::from(addr_bytes)
}

/// Derive an [`Address`] from a public key reference that implements `AsRef<[u8]>`.
///
/// This is a convenience wrapper for use with [`Falcon512PublicKey`]
/// and similar types.
///
/// # Panics
///
/// Panics if `pubkey.as_ref()` does not have exactly 897 bytes.
#[must_use]
pub fn derive_address_from<P: AsRef<[u8]>>(pubkey: &P) -> Address {
    let bytes = pubkey.as_ref();
    assert_eq!(
        bytes.len(),
        FALCON_PUBLIC_KEY_SIZE,
        "public key must be {FALCON_PUBLIC_KEY_SIZE} bytes",
    );
    let hash = blake3::hash(bytes);
    let mut addr_bytes = [0u8; 32];
    addr_bytes.copy_from_slice(&hash.as_bytes()[..32]);
    Address::from(addr_bytes)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::constants::FALCON_PUBLIC_KEY_SIZE;
    use crate::crypto::falcon::Falcon512;
    use crate::crypto::signature::SignatureScheme;
    use crate::crypto::hash::blake3_hash;

    /// A fixed 48-byte seed for deterministic tests.
    fn test_seed() -> [u8; 48] {
        let mut seed = [0u8; 48];
        seed[..4].copy_from_slice(b"test");
        seed
    }

    // -----------------------------------------------------------------------
    // derive_address
    // -----------------------------------------------------------------------

    #[test]
    fn test_derive_address_length() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let pk_bytes = kp.public_key_bytes();
        let addr = derive_address(&pk_bytes);
        assert_eq!(addr.as_bytes().len(), 32);
    }

    #[test]
    fn test_derive_address_deterministic() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let pk_bytes = kp.public_key_bytes();
        let a = derive_address(&pk_bytes);
        let b = derive_address(&pk_bytes);
        assert_eq!(a, b);
    }

    #[test]
    fn test_derive_address_is_blake3_of_pubkey() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let pk_bytes = kp.public_key_bytes();
        let addr = derive_address(&pk_bytes);
        let expected_hash = blake3_hash(&pk_bytes);
        assert_eq!(addr.as_bytes(), &expected_hash[..32]);
    }

    #[test]
    fn test_different_key_different_address() {
        let mut seed_a = test_seed();
        seed_a[0] = 0x01;
        let mut seed_b = test_seed();
        seed_b[0] = 0x02;

        let kp_a = Falcon512::generate(&seed_a).unwrap();
        let kp_b = Falcon512::generate(&seed_b).unwrap();

        let addr_a = derive_address(&kp_a.public_key_bytes());
        let addr_b = derive_address(&kp_b.public_key_bytes());

        assert_ne!(addr_a, addr_b);
    }

    // -----------------------------------------------------------------------
    // derive_address_from (convenience wrapper)
    // -----------------------------------------------------------------------

    #[test]
    fn test_derive_address_from_pubkey_type() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let pk = Falcon512::public_key(&kp);
        let addr = derive_address_from(&pk);
        let expected = derive_address(&kp.public_key_bytes());
        assert_eq!(addr, expected);
    }

    #[test]
    fn test_derive_address_from_ref() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let pk_bytes = kp.public_key_bytes();
        let pk_ref: &[u8; FALCON_PUBLIC_KEY_SIZE] = &pk_bytes;
        let addr = derive_address_from(pk_ref);
        let expected = derive_address(&pk_bytes);
        assert_eq!(addr, expected);
    }

    // -----------------------------------------------------------------------
    // Integration: full pipeline (keygen → address → format → parse)
    // -----------------------------------------------------------------------

    #[test]
    fn test_key_to_address_full_roundtrip() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let pk_bytes = kp.public_key_bytes();
        let addr = derive_address(&pk_bytes);

        // The address should format and parse correctly
        let formatted = crate::core::account::format_address(&addr);
        let parsed = crate::core::account::parse_address(&formatted).unwrap();
        assert_eq!(addr, parsed);
    }

    #[test]
    fn test_address_from_reconstructed_keypair() {
        // Generate, serialize private key, reconstruct, derive address
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let original_addr = derive_address(&kp.public_key_bytes());

        let sk_bytes = kp.private_key_bytes();
        let restored = Falcon512::from_private_key(&sk_bytes).unwrap();
        let restored_addr = derive_address(&restored.public_key_bytes());

        assert_eq!(original_addr, restored_addr);
    }

    #[test]
    fn test_multiple_keys_unique_addresses() {
        let count = 5;
        let mut addresses = std::collections::HashSet::new();

        for i in 0..count {
            let mut seed = test_seed();
            seed[0] = i as u8;
            seed[1] = (i >> 8) as u8;
            let kp = Falcon512::generate(&seed).unwrap();
            let addr = derive_address(&kp.public_key_bytes());
            addresses.insert(addr);
        }

        assert_eq!(addresses.len(), count);
    }
}
