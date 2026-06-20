//! Account types and `Address` format.
//!
//! Address format (per ADR-016):
//!   `0x` + 32 raw bytes as hex (64 chars) + 8-byte checksum (16 chars)
//!
//! Address derivation: `BLAKE3-256(Falcon-512 public key)[..32]`
//! Checksum: first 8 bytes of `BLAKE3(address_bytes)`

use primitive_types::U256;
use serde::{Deserialize, Serialize};

/// Maximum number of accounts expected per shard (used for capacity hints).
pub const MAX_ACCOUNTS_PER_SHARD: usize = 10_000_000;

// ---------------------------------------------------------------------------
// Account
// ---------------------------------------------------------------------------

/// An on-chain account.
///
/// All balances are stored as MOXX (10^-32 MONEX).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    /// Current spendable balance in MOXX.
    pub balance: U256,
    /// Next valid nonce for this account.
    pub nonce: u64,
    /// Optional code hash for future smart contract support.
    pub code_hash: Option<[u8; 32]>,
}

impl Account {
    /// Create a new account with zero nonce and no code hash.
    #[must_use]
    pub const fn new(balance: U256) -> Self {
        Self {
            balance,
            nonce: 0,
            code_hash: None,
        }
    }

    /// Create a new account with a specific nonce.
    #[must_use]
    pub const fn with_nonce(balance: U256, nonce: u64) -> Self {
        Self {
            balance,
            nonce,
            code_hash: None,
        }
    }

    /// Increment the nonce by one (called after each successful tx).
    pub const fn increment_nonce(&mut self) {
        self.nonce += 1;
    }
}

// ---------------------------------------------------------------------------
// Address
// ---------------------------------------------------------------------------

/// A 32-byte on-chain address.
///
/// Internally stored as raw bytes. Display and serialization use the
/// hex+checksum format (`0x` + 64 hex chars + 16 hex checksum chars).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address(
    #[serde(with = "hex_serde")]
    [u8; 32]
);

impl Address {
    /// The number of raw address bytes.
    pub const LEN: usize = 32;

    /// Return a reference to the underlying bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Consume the address, returning the raw bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl From<[u8; 32]> for Address {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Address formatting (hex + BLAKE3 checksum)
// ---------------------------------------------------------------------------

/// Format an Address into its display form.
///
/// Returns `0x` + 64 hex chars (32 bytes) + 16 hex chars (8-byte checksum).
/// The checksum is the first 8 bytes of `BLAKE3(address_bytes)`.
#[must_use]
pub fn format_address(addr: &Address) -> String {
    let raw = addr.as_bytes();
    let hash = blake3::hash(raw);
    let checksum = &hash.as_bytes()[..8];
    let hex_body = hex::encode(raw);
    let hex_checksum = hex::encode(checksum);
    format!("0x{hex_body}{hex_checksum}")
}

/// Error returned when parsing an address string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseAddressError {
    /// String does not start with `0x`.
    MissingPrefix,
    /// String is shorter than expected (should be 82 chars: 0x + 64 + 16).
    TooShort,
    /// Invalid hex characters in the string.
    InvalidHex,
    /// The embedded checksum does not match the computed checksum.
    ChecksumMismatch,
}

impl std::fmt::Display for ParseAddressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingPrefix => write!(f, "address missing 0x prefix"),
            Self::TooShort => write!(f, "address too short (expected 82 chars)"),
            Self::InvalidHex => write!(f, "address contains invalid hex characters"),
            Self::ChecksumMismatch => write!(f, "address checksum mismatch"),
        }
    }
}

/// Parse a formatted address string back into an `Address`.
///
/// Expects format: `0x` + 64 hex chars + 16 hex chars.
/// Validates the 8-byte BLAKE3 checksum.
///
/// # Errors
///
/// Returns `ParseAddressError` if the string is missing the `0x` prefix,
/// too short, contains invalid hex, or has a checksum mismatch.
pub fn parse_address(s: &str) -> std::result::Result<Address, ParseAddressError> {
    if !s.starts_with("0x") {
        return Err(ParseAddressError::MissingPrefix);
    }
    // 0x(2) + body(64) + checksum(16) = 82
    if s.len() < 82 {
        return Err(ParseAddressError::TooShort);
    }

    let body = &s[2..66];
    let checksum_hex = &s[66..82];

    let body_bytes = hex::decode(body).map_err(|_| ParseAddressError::InvalidHex)?;
    let checksum_bytes = hex::decode(checksum_hex).map_err(|_| ParseAddressError::InvalidHex)?;

    let mut raw = [0u8; 32];
    raw.copy_from_slice(&body_bytes);

    let hash = blake3::hash(&raw);
    let computed = &hash.as_bytes()[..8];
    if checksum_bytes.as_slice() != computed {
        return Err(ParseAddressError::ChecksumMismatch);
    }

    Ok(Address(raw))
}

// ---------------------------------------------------------------------------
// Serialization helpers for hex encoding the address
// ---------------------------------------------------------------------------

mod hex_serde {
    use serde::de::Error;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex_str = hex::encode(bytes);
        serializer.serialize_str(&format!("0x{hex_str}"))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let s = s.strip_prefix("0x").unwrap_or(&s);
        let bytes = hex::decode(s).map_err(D::Error::custom)?;
        if bytes.len() != 32 {
            return Err(D::Error::custom("address must be 32 bytes"));
        }
        let mut raw = [0u8; 32];
        raw.copy_from_slice(&bytes);
        Ok(raw)
    }
}

// ---------------------------------------------------------------------------
// Burn and Cap-Refill special addresses
// ---------------------------------------------------------------------------

/// The Burn address (`0x00..00`) — slashed and voluntarily burned MONEX.
#[must_use]
pub fn burn_address() -> Address {
    Address::from([0u8; 32])
}

/// The Cap-Refill address (`0x00..01`) — expands mainnet inflation cap.
#[must_use]
pub fn cap_refill_address() -> Address {
    let mut bytes = [0u8; 32];
    bytes[31] = 1;
    Address::from(bytes)
}

// ---------------------------------------------------------------------------
// SCALE codec support (parity-scale-codec)
// ---------------------------------------------------------------------------

impl parity_scale_codec::Encode for Address {
    fn encode_to<W: parity_scale_codec::Output + ?Sized>(&self, dest: &mut W) {
        dest.write(&self.0);
    }
}

impl parity_scale_codec::Decode for Address {
    fn decode<I: parity_scale_codec::Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
        let mut bytes = [0u8; 32];
        input.read(&mut bytes)?;
        Ok(Self(bytes))
    }
}

impl parity_scale_codec::Encode for Account {
    fn encode_to<W: parity_scale_codec::Output + ?Sized>(&self, dest: &mut W) {
        self.balance.encode_to(dest);
        self.nonce.encode_to(dest);
        self.code_hash.encode_to(dest);
    }
}

impl parity_scale_codec::Decode for Account {
    fn decode<I: parity_scale_codec::Input>(input: &mut I) -> Result<Self, parity_scale_codec::Error> {
        Ok(Self {
            balance: U256::decode(input)?,
            nonce: u64::decode(input)?,
            code_hash: Option::<[u8; 32]>::decode(input)?,
        })
    }
}

/// SCALE-encode an Account to a byte vector.
#[must_use]
pub fn scale_encode_account(acct: &Account) -> Vec<u8> {
    use parity_scale_codec::Encode;
    acct.encode()
}

/// SCALE-decode an Account from a byte slice.
///
/// # Panics
///
/// Panics if the bytes are not a valid SCALE-encoded Account.
#[must_use]
pub fn scale_decode_account(bytes: &[u8]) -> Account {
    use parity_scale_codec::Decode;
    Account::decode(&mut &bytes[..]).expect("valid SCALE-encoded Account")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_new() {
        let acc = Account::new(U256::from(1000));
        assert_eq!(acc.balance, U256::from(1000));
        assert_eq!(acc.nonce, 0);
        assert_eq!(acc.code_hash, None);
    }

    #[test]
    fn test_account_with_nonce() {
        let acc = Account::with_nonce(U256::from(500), 7);
        assert_eq!(acc.nonce, 7);
    }

    #[test]
    fn test_account_increment_nonce() {
        let mut acc = Account::new(U256::zero());
        acc.increment_nonce();
        assert_eq!(acc.nonce, 1);
        acc.increment_nonce();
        assert_eq!(acc.nonce, 2);
    }

    #[test]
    fn test_address_from_bytes() {
        let bytes = [0x3a; 32];
        let addr = Address::from(bytes);
        assert_eq!(addr.as_bytes(), &bytes);
    }

    #[test]
    fn test_address_display_and_parse_roundtrip() {
        let bytes = [0x3a, 0x1b, 0x2c, 0x3d, 0x4e, 0x5f, 0x6a, 0x7b,
                     0x8c, 0x9d, 0x0e, 0x1f, 0x2a, 0x3b, 0x4c, 0x5d,
                     0x6e, 0x7f, 0x8a, 0x9b, 0x0c, 0x1d, 0x2e, 0x3f,
                     0x4a, 0x5b, 0x6c, 0x7d, 0x8e, 0x9f, 0x0a, 0x1b];
        let addr = Address::from(bytes);
        let formatted = format_address(&addr);
        let parsed = parse_address(&formatted).unwrap();
        assert_eq!(addr, parsed);
        assert!(formatted.starts_with("0x"));
        assert_eq!(formatted.len(), 82);
    }

    #[test]
    fn test_format_address_length() {
        let addr = Address::from([0u8; 32]);
        let s = format_address(&addr);
        // "0x" + 64 hex chars (body) + 16 hex chars (checksum) = 82
        assert_eq!(s.len(), 82);
    }

    #[test]
    fn test_parse_address_valid() {
        let addr = Address::from([0xab; 32]);
        let formatted = format_address(&addr);
        let parsed = parse_address(&formatted).unwrap();
        assert_eq!(addr, parsed);
    }

    #[test]
    fn test_parse_address_missing_prefix() {
        let result = parse_address("abcd1234");
        assert_eq!(result, Err(ParseAddressError::MissingPrefix));
    }

    #[test]
    fn test_parse_address_too_short() {
        let result = parse_address("0xabcd");
        assert_eq!(result, Err(ParseAddressError::TooShort));
    }

    #[test]
    fn test_parse_address_invalid_hex() {
        let result = parse_address(&format!("0x{}", "z".repeat(80)));
        assert_eq!(result, Err(ParseAddressError::InvalidHex));
    }

    #[test]
    fn test_parse_address_checksum_mismatch() {
        // Build a valid 82-char address string but with wrong checksum
        let addr = Address::from([0x42; 32]);
        let formatted = format_address(&addr);
        // Corrupt the last checksum char
        let mut corrupted = formatted.clone();
        corrupted.replace_range(81..82, "f");
        let result = parse_address(&corrupted);
        assert_eq!(result, Err(ParseAddressError::ChecksumMismatch));
    }

    #[test]
    fn test_burn_address() {
        let addr = burn_address();
        assert_eq!(addr.as_bytes(), &[0u8; 32]);
    }

    #[test]
    fn test_cap_refill_address() {
        let addr = cap_refill_address();
        assert_eq!(addr.as_bytes()[31], 1);
        assert_eq!(addr.as_bytes()[..31], [0u8; 31]);
    }

    #[test]
    fn test_address_scale_roundtrip() {
        use parity_scale_codec::{Decode, Encode};
        let addr = Address::from([0x55; 32]);
        let encoded = addr.encode();
        let decoded = Address::decode(&mut &encoded[..]).unwrap();
        assert_eq!(addr, decoded);
    }

    #[test]
    fn test_account_scale_roundtrip() {
        use parity_scale_codec::{Decode, Encode};
        let acc = Account {
            balance: U256::from(12345),
            nonce: 42,
            code_hash: Some([0x99; 32]),
        };
        let encoded = acc.encode();
        let decoded = Account::decode(&mut &encoded[..]).unwrap();
        assert_eq!(acc, decoded);
    }

    #[test]
    fn test_account_scale_roundtrip_no_code() {
        use parity_scale_codec::{Decode, Encode};
        let acc = Account::new(U256::from(999));
        let encoded = acc.encode();
        let decoded = Account::decode(&mut &encoded[..]).unwrap();
        assert_eq!(acc, decoded);
    }

    #[test]
    fn test_account_serde_roundtrip() {
        let acc = Account::new(U256::from(42));
        let json = serde_json::to_string(&acc).unwrap();
        let decoded: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(acc, decoded);
    }

    #[test]
    fn test_address_serde_roundtrip() {
        let addr = Address::from([0x77; 32]);
        let json = serde_json::to_string(&addr).unwrap();
        let decoded: Address = serde_json::from_str(&json).unwrap();
        assert_eq!(addr, decoded);
    }
}
