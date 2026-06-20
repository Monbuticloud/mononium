//! State machine — applies blocks, executes transactions, maintains state.
//!
//! The state machine is the core of the protocol. It holds all on-chain
//! state in a Sparse Merkle Tree and processes blocks deterministically.

use crate::core::account::{Account, Address};
use crate::core::block::Block;
use crate::crypto::trie::SparseMerkleTree;
use crate::error::Result;

/// Receipt returned after successfully applying a block.
#[derive(Debug, Clone)]
pub struct BlockReceipt {
    /// Total fees collected from all transactions in the block.
    pub total_fees: u128,
    /// Number of transactions that were applied successfully.
    pub tx_count: u32,
    /// Number of transactions that failed validation and were skipped.
    pub failed_count: u32,
    /// Post-state root hash.
    pub state_root: [u8; 32],
}

/// The Mononium state machine.
///
/// Owns a [`SparseMerkleTree`] that stores all on-chain state:
///  - `0x00 ++ address` → SCALE(Account)
///  - `0x01 ++ pubkey`   → SCALE(ValidatorInfo)  *(future)*
///  - `0x02 ++ key`      → SCALE(meta value)      *(future)*
#[derive(Debug, Clone)]
pub struct StateMachine {
    state: SparseMerkleTree,
}

impl StateMachine {
    /// Create a new state machine with initial accounts.
    ///
    /// Each account is inserted into the state SMT under the
    /// `0x00 ++ address` namespace key.
    #[must_use]
    pub fn new(initial_accounts: impl IntoIterator<Item = (Address, Account)>) -> Self {
        let mut state = SparseMerkleTree::new();
        for (addr, acct) in initial_accounts {
            let key = crate::crypto::trie::namespace_key(
                crate::crypto::trie::NS_ACCOUNTS,
                addr.as_bytes(),
            );
            let value = crate::core::account::scale_encode_account(&acct);
            state.insert(&key, value);
        }
        Self { state }
    }

    /// Return the current state root hash.
    #[must_use]
    pub fn state_root(&mut self) -> [u8; 32] {
        self.state.root()
    }

    /// Retrieve an account from state by address, if it exists.
    #[must_use]
    pub fn get_account(&self, address: &Address) -> Option<Account> {
        let key = crate::crypto::trie::namespace_key(
            crate::crypto::trie::NS_ACCOUNTS,
            address.as_bytes(),
        );
        let bytes = self.state.get(&key)?;
        Some(crate::core::account::scale_decode_account(bytes))
    }

    /// Apply a block to the state machine.
    ///
    /// Validates the block, executes all transactions, distributes fees,
    /// and commits the new state root.
    ///
    /// # Errors
    ///
    /// Returns `LibError::Consensus` if the block is invalid at the
    /// header level (wrong chain_id, height mismatch, etc.).
    pub fn apply_block(&mut self, _block: &Block) -> Result<BlockReceipt> {
        // TODO: full block application
        // For now, return an empty receipt
        Ok(BlockReceipt {
            total_fees: 0,
            tx_count: 0,
            failed_count: 0,
            state_root: self.state.root(),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use primitive_types::U256;

    fn alice() -> Address {
        Address::from([0xAAu8; 32])
    }

    fn make_account(balance: u64) -> Account {
        Account::new(U256::from(balance))
    }

    #[test]
    fn test_new_state_machine_has_root() {
        let accounts = vec![(alice(), make_account(100))];
        let mut sm = StateMachine::new(accounts);
        let root = sm.state_root();
        assert_ne!(root, [0u8; 32]);
    }

    #[test]
    fn test_get_account_exists() {
        let accounts = vec![(alice(), make_account(100))];
        let sm = StateMachine::new(accounts);
        let acct = sm.get_account(&alice());
        assert_eq!(acct.unwrap().balance, U256::from(100));
    }

    #[test]
    fn test_get_account_missing_returns_none() {
        let sm = StateMachine::new(vec![]);
        assert!(sm.get_account(&alice()).is_none());
    }
}
