//! Tests for the Account and Address types.
//!
//! INVARIANT: Account balance, nonce, and code_hash are all independently accessible.
//! INVARIANT: Address checksum is deterministic and validated on parse.
//! INVARIANT: U256 constants have correct decimal values.

use crate::core::account::{Account, Address, format_address, parse_address};
use crate::core::constants::{ONE_MONEX, ONE_MOXX};
use primitive_types::U256;

// ---------------------------------------------------------------------------
// Account tests
// ---------------------------------------------------------------------------

#[test]
fn test_account_creation_with_defaults() {
    // Arrange
    let addr = Address::from([0u8; 32]);

    // Act
    let account = Account::new(addr, U256::zero());

    // Assert
    assert_eq!(account.balance, U256::zero());
    assert_eq!(account.nonce, 0);
    assert!(account.code_hash.is_none());
}

#[test]
fn test_account_creation_with_balance() {
    // Arrange
    let addr = Address::from([0u8; 32]);

    // Act
    let account = Account::new(addr, ONE_MONEX);

    // Assert
    assert_eq!(account.balance, ONE_MONEX);
    assert_eq!(account.nonce, 0);
}

#[test]
fn test_account_balance_updated() {
    // Arrange
    let addr = Address::from([0u8; 32]);
    let mut account = Account::new(addr, ONE_MONEX);

    // Act
    account.balance = account.balance + U256::from(500_000u64);

    // Assert
    assert_eq!(account.balance, ONE_MONEX + U256::from(500_000u64));
}

// ---------------------------------------------------------------------------
// Address tests
// ---------------------------------------------------------------------------

#[test]
fn test_address_from_bytes() {
    // Arrange
    let bytes = [0x3a, 0x1b, 0x2c, 0x3d, 0x4e, 0x5f, 0x6a, 0x7b,
                 0x8c, 0x9d, 0x0e, 0x1f, 0x2a, 0x3b, 0x4c, 0x5d,
                 0x6e, 0x7f, 0x8a, 0x9b, 0x0c, 0x1d, 0x2e, 0x3f,
                 0x4a, 0x5b, 0x6c, 0x7d, 0x8e, 0x9f, 0x0a, 0x1b];

    // Act
    let addr = Address::from(bytes);

    // Assert
    assert_eq!(addr.as_bytes(), &bytes);
}

#[test]
fn test_address_display_and_parse_roundtrip() {
    // Arrange
    let bytes = [0x3a, 0x1b, 0x2c, 0x3d, 0x4e, 0x5f, 0x6a, 0x7b,
                 0x8c, 0x9d, 0x0e, 0x1f, 0x2a, 0x3b, 0x4c, 0x5d,
                 0x6e, 0x7f, 0x8a, 0x9b, 0x0c, 0x1d, 0x2e, 0x3f,
                 0x4a, 0x5b, 0x6c, 0x7d, 0x8e, 0x9f, 0x0a, 0x1b];
    let addr = Address::from(bytes);

    // Act
    let formatted = format_address(&addr);
    let parsed = parse_address(&formatted).unwrap();

    // Assert
    assert_eq!(addr, parsed);
    assert!(formatted.starts_with("0x"));
    // 0x + 64 hex chars (32 bytes) + 16 hex chars (8 byte checksum) = 82 chars
    assert_eq!(formatted.len(), 82);
}

#[test]
fn test_address_parse_invalid_checksum() {
    // Arrange
    let bad_address = "0x3a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1deadbeefdeadbeef";

    // Act
    let result = parse_address(bad_address);

    // Assert
    assert!(result.is_err());
}

#[test]
fn test_address_parse_missing_prefix() {
    // Arrange
    let no_prefix = "3a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1a7f2b1c9d3e4f5a6";

    // Act
    let result = parse_address(no_prefix);

    // Assert
    assert!(result.is_err());
}

#[test]
fn test_address_parse_too_short() {
    // Arrange
    let short = "0xabcd";

    // Act
    let result = parse_address(short);

    // Assert
    assert!(result.is_err());
}

#[test]
fn test_address_zero_is_burn_address() {
    // Arrange
    let burn = Address::from([0u8; 32]);

    // Act
    let formatted = format_address(&burn);

    // Assert
    assert!(formatted.starts_with("0x0000000000000000000000000000000000000000000000000000000000000000"));
}

// ---------------------------------------------------------------------------
// U256 constant tests
// ---------------------------------------------------------------------------

#[test]
fn test_one_monex_is_10_pow_32() {
    // 1 MONEX = 10^32 MOXX
    let expected = U256::from(10u64).pow(U256::from(32u64));
    assert_eq!(ONE_MONEX, expected);
}

#[test]
fn test_one_moxx_is_one() {
    assert_eq!(ONE_MOXX, U256::from(1u64));
}
