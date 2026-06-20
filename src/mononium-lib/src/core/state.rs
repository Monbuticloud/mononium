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
    use crate::core::block::{Block, BlockBody, BlockHeader};
    use crate::core::transaction::{Transaction, TxBody, BurnTarget};
    use crate::crypto::falcon::Falcon512Signature;
    use crate::crypto::constants::FALCON_SIGNATURE_SIZE;

    fn alice() -> Address { Address::from([0xAAu8; 32]) }
    fn bob() -> Address { Address::from([0xBBu8; 32]) }

    fn dummy_sig() -> Falcon512Signature {
        Falcon512Signature::from_bytes(&[0xEEu8; FALCON_SIGNATURE_SIZE]).unwrap()
    }

    fn make_account(balance: u64) -> Account {
        Account::new(U256::from(balance))
    }

    fn empty_block(proposer: Address, height: u64) -> Block {
        Block {
            header: BlockHeader {
                height,
                parent_hash: [0u8; 32],
                global_state_root: [0u8; 32],
                tx_root: [0u8; 32],
                timestamp: 1_700_000_000,
                proposer,
                chain_id: 0,
            },
            body: BlockBody { transactions: vec![] },
        }
    }

    fn transfer_block(sender: Address, recipient: Address, amount: u64,
                       nonce: u64, chain_id: u64) -> Block {
        let tx = Transaction {
            chain_id,
            nonce,
            sender,
            fee: U256::from(100),
            body: TxBody::Transfer {
                recipient,
                amount: U256::from(amount),
            },
            signature: dummy_sig(),
        };
        Block {
            header: BlockHeader {
                height: 1,
                parent_hash: [0u8; 32],
                global_state_root: [0u8; 32],
                tx_root: [0u8; 32],
                timestamp: 1_700_000_000,
                proposer: Address::from([0u8; 32]),
                chain_id,
            },
            body: BlockBody { transactions: vec![tx] },
        }
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

    #[test]
    fn test_empty_block_no_state_change() {
        let accounts = vec![(alice(), make_account(100))];
        let mut sm = StateMachine::new(accounts);
        let pre_root = sm.state_root();
        let receipt = sm.apply_block(&empty_block(alice(), 1)).unwrap();
        assert_eq!(receipt.tx_count, 0);
        assert_eq!(receipt.failed_count, 0);
        // Root should not change for an empty block
        // (fee distribution with 0 fees is a no-op)
        assert_eq!(sm.state_root(), pre_root);
    }

    #[test]
    fn test_transfer_moves_balance() {
        let accounts = vec![
            (alice(), make_account(1000)),
            (bob(), make_account(0)),
        ];
        let mut sm = StateMachine::new(accounts);
        let block = transfer_block(alice(), bob(), 100, 0, 0);
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);

        let bob_post = sm.get_account(&bob()).unwrap();
        assert_eq!(bob_post.balance, U256::from(100));
    }
}
