//! Core-specific protocol constants.
//!
//! These govern the U256 precision, fee defaults, and other
//! core-module parameters. Chain-wide constants are in
//! `crate::constants`.

#![allow(clippy::unreadable_literal)]

use primitive_types::U256;

// ---------------------------------------------------------------------------
// Denomination
// ---------------------------------------------------------------------------

/// One MONEX expressed in MOXX (the smallest unit).
///
/// 1 MONEX = 10^32 MOXX. All on-chain values are stored in MOXX.
pub const ONE_MONEX: U256 = U256([9632337040368467968, 5421010862427, 0, 0]);

/// One MOXX (the smallest unit, equivalent to 10^-32 MONEX).
pub const ONE_MOXX: U256 = U256([1, 0, 0, 0]);

// ---------------------------------------------------------------------------
// Fee defaults
// ---------------------------------------------------------------------------

/// Default flat fee per transaction (~0.00667 MONEX in MOXX).
pub const DEFAULT_FLAT_FEE: U256 = U256([7223210852348854272, 36158142452, 0, 0]);

/// Default per-byte fee rate (~0.000467 MONEX/byte in MOXX).
pub const DEFAULT_PER_BYTE_RATE: U256 = U256([13902444923925299200, 2531612072, 0, 0]);

/// Anti-spam deposit per transaction (~0.33 MONEX in MOXX).
pub const ANTI_SPAM_DEPOSIT: U256 = U256([1522216674051227648, 1788933584601, 0, 0]);

/// Default minimum mempool fee (~0.0667 MONEX in MOXX) — local node policy.
pub const DEFAULT_MIN_MEMPOOL_FEE: U256 = U256([16891876302359887872, 361581424523, 0, 0]);

/// Burn transaction flat fee (10 MOXX).
pub const BURN_FLAT_FEE: U256 = U256([10, 0, 0, 0]);

/// Missed slot penalty (~0.08 MONEX in MOXX).
pub const MISSED_SLOT_PENALTY: U256 = U256([3722225092021714944, 433680868994, 0, 0]);

// ---------------------------------------------------------------------------
// Governance deposit
// ---------------------------------------------------------------------------

/// Proposal deposit (100 MONEX in MOXX).
pub const PROPOSAL_DEPOSIT: U256 = U256([4003012203950112768, 542101086242752, 0, 0]);

// ---------------------------------------------------------------------------
// Supply (mainnet)
// ---------------------------------------------------------------------------

/// Base maximum supply (10,000,000,000 MONEX in MOXX).
pub const BASE_MAX_SUPPLY: U256 = U256([11806718586779598848, 13574535716559052564, 2938, 0]);

/// Annual ceiling rate (3.5%) expressed as parts-per-10^4 for integer math.
pub const SUPPLY_CEILING_RATE: u32 = 350; // 3.5% = 350 / 10000

/// Annual headroom rate (5.0%) expressed as parts-per-10^4.
pub const SUPPLY_HEADROOM_RATE: u32 = 500; // 5.0% = 500 / 10000

// ---------------------------------------------------------------------------
// Staking
// ---------------------------------------------------------------------------

/// Minimum stake to enter candidate pool (1 MONEX in MOXX).
pub const MIN_STAKE: U256 = ONE_MONEX;

/// Unstaking cooldown in eras (168 eras ≈ 7 days).
pub const UNSTAKING_COOLDOWN_ERAS: u64 = 168;

/// Freeze duration in eras for slashed validators (72 eras ≈ 3 days).
pub const FREEZE_DURATION_ERAS: u16 = 72;

/// Maximum validator set size (mainnet).
pub const MAX_VALIDATORS: usize = 101;

/// Maximum validator set size (devnet).
pub const MAX_VALIDATORS_DEVNET: usize = 21;
