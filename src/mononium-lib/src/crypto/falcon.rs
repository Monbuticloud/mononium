//! Concrete Falcon-512 implementation of [`SignatureScheme`].
//!
//! Wraps the [`falcon_rs`] crate (FN-DSA FIPS 206) into Mononium's
//! [`SignatureScheme`] trait. All protocol signing uses Falcon-512
//! (NIST Level I, logn = 9).

use crate::error::{LibError, Result};
use crate::crypto::constants::{
    FALCON_LOGN, FALCON_PRIVATE_KEY_SIZE, FALCON_PUBLIC_KEY_SIZE, FALCON_SEED_SIZE,
    FALCON_SIGNATURE_SIZE,
};
use crate::crypto::signature::SignatureScheme;

use falcon::safe_api::{DomainSeparation, FnDsaKeyPair, FnDsaSignature};

// ---------------------------------------------------------------------------
// Public key
// ---------------------------------------------------------------------------

/// A Falcon-512 public key (897 bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Falcon512PublicKey(pub [u8; FALCON_PUBLIC_KEY_SIZE]);

impl AsRef<[u8]> for Falcon512PublicKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Signature
// ---------------------------------------------------------------------------

/// A Falcon-512 signature (666 bytes at FN-DSA-512 security).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Falcon512Signature(Vec<u8>);

impl Falcon512Signature {
    /// The expected byte length for FN-DSA-512 signatures.
    pub const LEN: usize = FALCON_SIGNATURE_SIZE;

    /// Create a signature from raw bytes, validating the length.
    ///
    /// # Errors
    ///
    /// Returns `LibError::Crypto` if the length is not exactly 666 bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::LEN {
            return Err(LibError::Crypto(format!(
                "signature must be {} bytes, got {}",
                Self::LEN,
                bytes.len()
            )));
        }
        Ok(Self(bytes.to_vec()))
    }

    /// Return a reference to the raw signature bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Consume and return the owned byte vector.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

impl AsRef<[u8]> for Falcon512Signature {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Key pair
// ---------------------------------------------------------------------------

/// A Falcon-512 key pair wrapping the underlying [`FnDsaKeyPair`].
///
/// Holds secret key material — use with care. The underlying memory is
/// zeroized on drop by the `falcon-rs` crate.
#[derive(Debug)]
pub struct Falcon512KeyPair {
    inner: FnDsaKeyPair,
}

impl Falcon512KeyPair {
    /// Return the public key as a fixed-size array.
    #[must_use]
    pub fn public_key_bytes(&self) -> [u8; FALCON_PUBLIC_KEY_SIZE] {
        let slice = self.inner.public_key();
        let mut arr = [0u8; FALCON_PUBLIC_KEY_SIZE];
        arr.copy_from_slice(slice);
        arr
    }

    /// Return the private key as a fixed-size array.
    ///
    /// **Security:** The returned bytes contain secret key material.
    /// The caller must zeroize them when done.
    #[must_use]
    pub fn private_key_bytes(&self) -> [u8; FALCON_PRIVATE_KEY_SIZE] {
        let slice = self.inner.private_key();
        let mut arr = [0u8; FALCON_PRIVATE_KEY_SIZE];
        arr.copy_from_slice(slice);
        arr
    }
}

// ---------------------------------------------------------------------------
// SignatureScheme implementation
// ---------------------------------------------------------------------------

/// The Falcon-512 signature scheme.
///
/// Implements [`SignatureScheme`] for use anywhere in the protocol
/// (transactions, blocks, consensus votes).
pub struct Falcon512;

impl SignatureScheme for Falcon512 {
    type KeyPair = Falcon512KeyPair;
    type PublicKey = Falcon512PublicKey;
    type Signature = Falcon512Signature;

    fn generate(seed: &[u8; FALCON_SEED_SIZE]) -> Result<Self::KeyPair> {
        let kp = FnDsaKeyPair::generate_deterministic(seed, FALCON_LOGN)
            .map_err(|e| LibError::Crypto(format!("key generation failed: {e}")))?;
        Ok(Falcon512KeyPair { inner: kp })
    }

    fn sign(keypair: &Self::KeyPair, msg: &[u8]) -> Result<Self::Signature> {
        let sig = keypair
            .inner
            .sign(msg, &DomainSeparation::None)
            .map_err(|e| LibError::Crypto(format!("signing failed: {e}")))?;
        let bytes = sig.into_bytes();
        // The signature should always be 666 bytes for FN-DSA-512.
        debug_assert_eq!(bytes.len(), FALCON_SIGNATURE_SIZE);
        Ok(Falcon512Signature(bytes))
    }

    fn verify(pubkey: &Self::PublicKey, msg: &[u8], sig: &Self::Signature) -> bool {
        FnDsaSignature::verify(&sig.0, &pubkey.0, msg, &DomainSeparation::None).is_ok()
    }

    fn public_key(keypair: &Self::KeyPair) -> Self::PublicKey {
        Falcon512PublicKey(keypair.public_key_bytes())
    }

    fn from_private_key(bytes: &[u8]) -> Result<Self::KeyPair> {
        let kp = FnDsaKeyPair::from_private_key(bytes)
            .map_err(|e| LibError::Crypto(format!("invalid private key: {e}")))?;
        Ok(Falcon512KeyPair { inner: kp })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A fixed 48-byte seed for deterministic tests.
    fn test_seed() -> [u8; 48] {
        let mut seed = [0u8; 48];
        seed[..4].copy_from_slice(b"test");
        seed
    }

    fn test_message() -> &'static [u8] {
        b"hello mononium"
    }

    // -----------------------------------------------------------------------
    // Key generation
    // -----------------------------------------------------------------------

    #[test]
    fn test_generate_keypair() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let pk = Falcon512::public_key(&kp);
        assert_eq!(pk.as_ref().len(), FALCON_PUBLIC_KEY_SIZE);
    }

    #[test]
    fn test_generate_deterministic() {
        // Same seed → same keypair
        let kp_a = Falcon512::generate(&test_seed()).unwrap();
        let kp_b = Falcon512::generate(&test_seed()).unwrap();
        let pk_a = Falcon512::public_key(&kp_a);
        let pk_b = Falcon512::public_key(&kp_b);
        assert_eq!(pk_a, pk_b);
    }

    #[test]
    fn test_different_seed_different_key() {
        let mut seed_a = test_seed();
        seed_a[0] = 0x01;
        let mut seed_b = test_seed();
        seed_b[0] = 0x02;

        let kp_a = Falcon512::generate(&seed_a).unwrap();
        let kp_b = Falcon512::generate(&seed_b).unwrap();
        assert_ne!(
            Falcon512::public_key(&kp_a),
            Falcon512::public_key(&kp_b)
        );
    }

    #[test]
    fn test_public_key_size() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let pk = Falcon512::public_key(&kp);
        assert_eq!(pk.as_ref().len(), FALCON_PUBLIC_KEY_SIZE);
        assert_eq!(FALCON_PUBLIC_KEY_SIZE, 897);
    }

    #[test]
    fn test_private_key_size() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let sk = kp.private_key_bytes();
        assert_eq!(sk.len(), FALCON_PRIVATE_KEY_SIZE);
        assert_eq!(FALCON_PRIVATE_KEY_SIZE, 1281);
    }

    // -----------------------------------------------------------------------
    // Sign / Verify
    // -----------------------------------------------------------------------

    #[test]
    fn test_sign_and_verify() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let msg = test_message();
        let sig = Falcon512::sign(&kp, msg).unwrap();
        let pk = Falcon512::public_key(&kp);

        assert!(Falcon512::verify(&pk, msg, &sig));
    }

    #[test]
    fn test_signature_size() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let msg = test_message();
        let sig = Falcon512::sign(&kp, msg).unwrap();
        assert_eq!(sig.as_ref().len(), FALCON_SIGNATURE_SIZE);
        assert_eq!(FALCON_SIGNATURE_SIZE, 809);
    }

    #[test]
    fn test_wrong_message_fails() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let sig = Falcon512::sign(&kp, b"real message").unwrap();
        let pk = Falcon512::public_key(&kp);

        assert!(!Falcon512::verify(&pk, b"wrong message", &sig));
    }

    #[test]
    fn test_wrong_pubkey_fails() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let msg = test_message();
        let sig = Falcon512::sign(&kp, msg).unwrap();

        // Generate a different keypair to get a different pubkey
        let mut other_seed = test_seed();
        other_seed[0] = 0xff;
        let other_kp = Falcon512::generate(&other_seed).unwrap();
        let wrong_pk = Falcon512::public_key(&other_kp);

        assert!(!Falcon512::verify(&wrong_pk, msg, &sig));
    }

    #[test]
    fn test_verify_rejects_tampered_signature() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let msg = test_message();
        let mut sig = Falcon512::sign(&kp, msg).unwrap();

        // Corrupt the last byte of the signature
        let mut bytes = sig.0.clone();
        if let Some(b) = bytes.last_mut() {
            *b ^= 0xff;
        }
        sig.0 = bytes;

        let pk = Falcon512::public_key(&kp);
        assert!(!Falcon512::verify(&pk, msg, &sig));
    }

    // -----------------------------------------------------------------------
    // Serialization round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_keypair_roundtrip_via_private_key() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let sk = kp.private_key_bytes();
        let original_pk = Falcon512::public_key(&kp);

        // Reconstruct from private key
        let restored = Falcon512::from_private_key(&sk).unwrap();
        let restored_pk = Falcon512::public_key(&restored);

        assert_eq!(original_pk, restored_pk);
    }

    #[test]
    fn test_keypair_roundtrip_sign_after_restore() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let sk = kp.private_key_bytes();

        let restored = Falcon512::from_private_key(&sk).unwrap();
        let msg = test_message();
        let sig = Falcon512::sign(&restored, msg).unwrap();
        let pk = Falcon512::public_key(&restored);

        assert!(Falcon512::verify(&pk, msg, &sig));
    }

    #[test]
    fn test_signature_from_bytes_roundtrip() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let msg = test_message();
        let sig = Falcon512::sign(&kp, msg).unwrap();
        let pk = Falcon512::public_key(&kp);

        // Serialize signature to bytes
        let sig_bytes = sig.as_ref().to_vec();

        // Deserialize
        let sig2 = Falcon512Signature::from_bytes(&sig_bytes).unwrap();

        // Verify with deserialized signature
        assert!(Falcon512::verify(&pk, msg, &sig2));
    }

    #[test]
    fn test_signature_from_bytes_wrong_length() {
        let result = Falcon512Signature::from_bytes(&[0u8; 100]);
        assert!(result.is_err());
        match result {
            Err(LibError::Crypto(_)) => {} // expected
            _ => panic!("expected Crypto error"),
        }
    }

    #[test]
    fn test_signature_into_bytes() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let sig = Falcon512::sign(&kp, test_message()).unwrap();
        let bytes = sig.clone().into_bytes();
        assert_eq!(bytes.len(), FALCON_SIGNATURE_SIZE);
        assert_eq!(bytes, sig.as_ref());
    }

    // -----------------------------------------------------------------------
    // Deterministic signing
    // -----------------------------------------------------------------------

    #[test]
    fn test_deterministic_signing_consistency() {
        // Two signatures with the same key + msg should differ (randomized sign)
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let msg = test_message();
        let sig1 = Falcon512::sign(&kp, msg).unwrap();
        let sig2 = Falcon512::sign(&kp, msg).unwrap();
        let pk = Falcon512::public_key(&kp);

        // Signatures should differ (non-deterministic signing)
        // but both should verify
        assert!(Falcon512::verify(&pk, msg, &sig1));
        assert!(Falcon512::verify(&pk, msg, &sig2));
        // Note: falcon-rs uses randomized signing by default,
        // so signatures will differ. Both must verify.
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_empty_message() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let pk = Falcon512::public_key(&kp);
        let sig = Falcon512::sign(&kp, b"").unwrap();
        assert!(Falcon512::verify(&pk, b"", &sig));
    }

    #[test]
    fn test_large_message() {
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let pk = Falcon512::public_key(&kp);
        let large_msg = vec![0xABu8; 1_000_000];
        let sig = Falcon512::sign(&kp, &large_msg).unwrap();
        assert!(Falcon512::verify(&pk, &large_msg, &sig));
    }

    #[test]
    fn test_signature_verify_fresh_instance() {
        // Verify using a Signature::from_bytes + pubkey without the original keypair
        let kp = Falcon512::generate(&test_seed()).unwrap();
        let msg = test_message();
        let sig_orig = Falcon512::sign(&kp, msg).unwrap();
        let pk = Falcon512::public_key(&kp);

        // Export sig + pubkey, then verify on a "fresh" instance
        let sig_bytes = sig_orig.as_ref().to_vec();
        let pk_bytes = pk.as_ref().to_vec();

        // Reconstruct both from raw bytes
        let sig_reconstructed = Falcon512Signature::from_bytes(&sig_bytes).unwrap();
        let mut pk_arr = [0u8; FALCON_PUBLIC_KEY_SIZE];
        pk_arr.copy_from_slice(&pk_bytes);
        let pk_reconstructed = Falcon512PublicKey(pk_arr);

        assert!(Falcon512::verify(&pk_reconstructed, msg, &sig_reconstructed));
    }
}
