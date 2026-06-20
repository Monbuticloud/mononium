//! Unified error types for the mononium library.
//!
//! Uses `thiserror` for concise derive-based error impls.

use primitive_types::U256;
use thiserror::Error;

/// Convenience result alias for fallible library operations.
pub type Result<T> = std::result::Result<T, LibError>;

/// A helper wrapper for displaying byte arrays as hex in error messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexBytes(pub [u8; 32]);

impl std::fmt::Display for HexBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{}", hex::encode(self.0))
    }
}

impl From<[u8; 32]> for HexBytes {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Unified error enum for all internal error paths.
///
/// Each variant carries contextual data to aid debugging. The CLI wraps
/// these into `anyhow::Error` for user-facing display; the GUI reads
/// them directly.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LibError {
    #[error("invalid signature")]
    InvalidSignature,

    #[error("insufficient balance: have {0}, need {1}")]
    InsufficientBalance(U256, U256),

    #[error("invalid nonce: expected {0}, got {1}")]
    InvalidNonce(u64, u64),

    #[error("account not found: {0}")]
    AccountNotFound(HexBytes),

    #[error("validator not found: {0}")]
    ValidatorNotFound(HexBytes),

    #[error("block not found: {0}")]
    BlockNotFound(u64),

    #[error("tx not found")]
    TxNotFound,

    #[error("proposal not found")]
    ProposalNotFound,

    #[error("governance action rejected: {0}")]
    GovernanceRejected(&'static str),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("serialization error: {0}")]
    Codec(String),

    #[error("consensus error: {0}")]
    Consensus(&'static str),

    #[error("network error: {0}")]
    Network(String),

    #[error("invalid address: {0}")]
    InvalidAddress(String),

    #[error("address checksum mismatch: expected {0}, computed {1}")]
    ChecksumMismatch(String, String),
}
