//! `SignatureScheme` trait — pluggable digital signature abstraction.
//!
//! Falcon-512 is the V1 implementation (see [`super::falcon::Falcon512`]).
//! If Falcon is ever broken, implement this trait for the replacement and
//! swap at the consensus layer via DI — no protocol changes required.

use crate::error::Result;

/// Pluggable signature scheme.
///
/// Each associated type is algorithm-specific (key sizes differ per scheme).
/// Implementations must be constant-time where applicable.
pub trait SignatureScheme: Sized {
    /// Algorithm-specific key pair.
    type KeyPair;

    /// Algorithm-specific public key.
    type PublicKey: AsRef<[u8]>;

    /// Algorithm-specific signature.
    type Signature: AsRef<[u8]>;

    /// Generate a new key pair from 48 bytes of seed entropy.
    ///
    /// # Errors
    ///
    /// Returns `LibError::Crypto` if key generation fails (e.g., bad entropy).
    fn generate(seed: &[u8; 48]) -> Result<Self::KeyPair>;

    /// Sign a message with the given key pair.
    ///
    /// # Errors
    ///
    /// Returns `LibError::Crypto` if signing fails.
    fn sign(keypair: &Self::KeyPair, msg: &[u8]) -> Result<Self::Signature>;

    /// Verify a signature against a public key and message.
    ///
    /// Returns `true` if the signature is valid, `false` otherwise.
    /// This function is constant-time where the implementation supports it.
    #[must_use]
    fn verify(pubkey: &Self::PublicKey, msg: &[u8], sig: &Self::Signature) -> bool;

    /// Extract the public key from a key pair.
    #[must_use]
    fn public_key(keypair: &Self::KeyPair) -> Self::PublicKey;

    /// Reconstruct a key pair from private key bytes.
    ///
    /// # Errors
    ///
    /// Returns `LibError::Crypto` if the private key bytes are invalid.
    fn from_private_key(bytes: &[u8]) -> Result<Self::KeyPair>;
}
