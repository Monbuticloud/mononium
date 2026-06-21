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

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("invalid address: {0}")]
    InvalidAddress(String),

    #[error("equivocation: header height mismatch (a={0}, b={1})")]
    EquivocationHeightMismatch(u64, u64),

    #[error("equivocation: parent hash mismatch")]
    EquivocationParentMismatch,

    #[error("equivocation: headers are identical")]
    EquivocationIdenticalBlocks,

    #[error("equivocation: signature A is invalid")]
    EquivocationSigAInvalid,

    #[error("equivocation: signature B is invalid")]
    EquivocationSigBInvalid,

    #[error("address checksum mismatch: expected {0}, computed {1}")]
    ChecksumMismatch(String, String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_bytes_display() {
        let hb = HexBytes([0xABu8; 32]);
        let s = hb.to_string();
        assert!(s.starts_with("0x"));
        assert_eq!(s.len(), 66); // 0x + 64 hex chars
    }

    #[test]
    fn test_hex_bytes_from() {
        let arr = [0x42u8; 32];
        let hb: HexBytes = arr.into();
        assert_eq!(hb.0, arr);
    }

    #[test]
    fn test_hex_bytes_eq() {
        let a = HexBytes([0x01u8; 32]);
        let b = HexBytes([0x01u8; 32]);
        let c = HexBytes([0x02u8; 32]);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_error_display_variants() {
        assert_eq!(LibError::InvalidSignature.to_string(), "invalid signature");
        assert_eq!(
            LibError::InsufficientBalance(U256::from(10), U256::from(100)).to_string(),
            "insufficient balance: have 10, need 100"
        );
        assert_eq!(
            LibError::InvalidNonce(0, 5).to_string(),
            "invalid nonce: expected 0, got 5"
        );
        assert!(LibError::Storage("disk full".to_string()).to_string().contains("disk full"));
        assert!(LibError::Crypto("bad key".to_string()).to_string().contains("bad key"));
        assert!(LibError::Codec("bad encode".to_string()).to_string().contains("bad encode"));
        assert_eq!(
            LibError::BlockNotFound(42).to_string(),
            "block not found: 42"
        );
        assert_eq!(LibError::TxNotFound.to_string(), "tx not found");
        assert_eq!(LibError::ProposalNotFound.to_string(), "proposal not found");
        assert_eq!(
            LibError::Consensus("bad proposal").to_string(),
            "consensus error: bad proposal"
        );
        assert!(LibError::Network("timeout".to_string()).to_string().contains("timeout"));
        assert!(LibError::GovernanceRejected("no").to_string().contains("no"));
        assert_eq!(
            LibError::InvalidAddress("bad".to_string()).to_string(),
            "invalid address: bad"
        );
        assert_eq!(
            LibError::AccountNotFound(HexBytes([0xAAu8; 32])).to_string(),
            "account not found: 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(
            LibError::ValidatorNotFound(HexBytes([0xBBu8; 32])).to_string(),
            "validator not found: 0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        );
        assert_eq!(
            LibError::ChecksumMismatch("abc".to_string(), "def".to_string()).to_string(),
            "address checksum mismatch: expected abc, computed def"
        );
        assert_eq!(
            LibError::EquivocationHeightMismatch(1, 2).to_string(),
            "equivocation: header height mismatch (a=1, b=2)"
        );
        assert_eq!(
            LibError::EquivocationParentMismatch.to_string(),
            "equivocation: parent hash mismatch"
        );
        assert_eq!(
            LibError::EquivocationIdenticalBlocks.to_string(),
            "equivocation: headers are identical"
        );
        assert_eq!(
            LibError::EquivocationSigAInvalid.to_string(),
            "equivocation: signature A is invalid"
        );
        assert_eq!(
            LibError::EquivocationSigBInvalid.to_string(),
            "equivocation: signature B is invalid"
        );
    }
}
