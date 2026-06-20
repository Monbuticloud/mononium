//! Database table name constants.
//!
//! Each constant corresponds to a redb `TableDefinition<&[u8], &[u8]>` used
//! by the storage engine. The same names are exposed through the
//! `StorageEngine` trait so callers can reference tables by name without
//! depending on the concrete engine.

/// On-chain account state: `{Address bytes} → SCALE-encoded Account`.
pub const ACCOUNTS: &str = "accounts";

/// Canonical blocks indexed by height (big-endian u64): `{height_be} → SCALE-encoded Block`.
pub const BLOCKS: &str = "blocks";

/// Block hash lookup: `{block_hash} → {height_be}`.
pub const BLOCK_HASHES: &str = "block_hashes";

/// Individual transactions (for RPC lookup): `{tx_hash} → {height_be, tx_index}`.
pub const TXS: &str = "transactions";

/// Commit votes: `{height_be}{validator_addr} → SCALE-encoded CommitVote`.
pub const VOTES: &str = "votes";

/// Validator set state: `{validator_addr} → SCALE-encoded ValidatorEntry`.
pub const VALIDATORS: &str = "validators";

/// Metadata key-value pairs: `{key} → {value}`.
///
/// Used for genesis marker, chain_id, current era, etc.
pub const META: &str = "meta";

/// All table names in a single slice (for iteration / validation).
pub const ALL_TABLES: &[&str] = &[
    ACCOUNTS,
    BLOCKS,
    BLOCK_HASHES,
    TXS,
    VOTES,
    VALIDATORS,
    META,
];

/// Meta key marking that genesis has been loaded.
pub const GENESIS_LOADED_KEY: &[u8] = b"genesis_loaded";

/// Meta key for the chain identifier.
pub const CHAIN_ID_KEY: &[u8] = b"chain_id";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_tables_are_unique() {
        let mut sorted: Vec<&str> = ALL_TABLES.to_vec();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ALL_TABLES.len());
    }
}
