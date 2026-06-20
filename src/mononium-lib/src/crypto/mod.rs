//! Cryptography module.
//!
//! Digital signatures (Falcon-512), hashing (BLAKE3), and address derivation.
//!
//! # Modules
//!
//! | Module | Description |
//! |--------|-------------|
//! | [`constants`] | Key/signature size constants and crypto-domain values |
//! | [`signature`] | [`SignatureScheme`] trait — pluggable signature abstraction |
//! | [`falcon`] | Falcon-512 concrete implementation |
//! | [`hash`] | BLAKE3 hashing utilities |
//! | [`address`] | Address derivation from Falcon-512 public keys |

pub mod address;
pub mod constants;
pub mod falcon;
pub mod hash;
pub mod signature;
