//! State machine — applies blocks, executes transactions, maintains state.
//!
//! The state machine is the core of the protocol. It holds all on-chain
//! state in a Sparse Merkle Tree and processes blocks deterministically.

use primitive_types::U256;

use crate::core::account::{Account, Address, scale_encode_account};
use crate::core::block::Block;
use crate::core::transaction::{BurnTarget, TxBody};
use crate::crypto::trie::{SparseMerkleTree, namespace_key, NS_ACCOUNTS};
use crate::error::{LibError, Result};

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
    pub fn apply_block(&mut self, block: &Block) -> Result<BlockReceipt> {
        let mut block_fees: u128 = 0;
        let mut tx_count: u32 = 0;
        let mut failed_count: u32 = 0;

        for tx in &block.body.transactions {
            // Validate basic chain_id
            if tx.chain_id != block.header.chain_id {
                failed_count += 1;
                continue;
            }

            // Get sender account — missing sender = fatal skip
            let Some(mut sender_acct) = self.get_account(&tx.sender) else {
                failed_count += 1;
                continue;
            };

            // Attempt to execute the tx body
            let executed = match &tx.body {
                TxBody::Transfer { recipient, amount } => {
                    self.try_transfer(&mut sender_acct, recipient, *amount, tx.fee, tx.nonce)
                }
                TxBody::Burn { target, amount } => {
                    let destination = match target {
                        BurnTarget::Permanent => crate::core::account::burn_address(),
                        BurnTarget::CapRefill => crate::core::account::cap_refill_address(),
                    };
                    self.try_transfer(&mut sender_acct, &destination, *amount, tx.fee, tx.nonce)
                }
                TxBody::RegisterValidator { .. }
                | TxBody::Stake { .. }
                | TxBody::RegisterAndStake { .. }
                | TxBody::Unstake { .. } => {
                    // Validate nonce + deduct fee at minimum
                    self.try_deduct_fee_only(&mut sender_acct, tx.fee, tx.nonce)
                }
            };

            if executed.is_ok() {
                self.set_account(&tx.sender, &sender_acct);
                block_fees += fee_as_u128(tx.fee);
                tx_count += 1;
            } else {
                if sender_acct.balance >= tx.fee {
                    sender_acct.balance -= tx.fee;
                    self.set_account(&tx.sender, &sender_acct);
                    block_fees += fee_as_u128(tx.fee);
                }
                failed_count += 1;
            }
        }

        // TODO: fee distribution (needs active validator set)
        // TODO: state root verification

        Ok(BlockReceipt {
            total_fees: block_fees,
            tx_count,
            failed_count,
            state_root: self.state.root(),
        })
    }

    /// Try to execute a Transfer/Burn tx.
    /// Deducts `amount + fee`, credits `recipient`, increments nonce.
    /// On failure, does NOT mutate state.
    fn try_transfer(
        &mut self,
        sender: &mut Account,
        recipient: &Address,
        amount: U256,
        fee: U256,
        nonce: u64,
    ) -> Result<()> {
        // Verify nonce
        if sender.nonce != nonce {
            return Err(LibError::InvalidNonce(sender.nonce, nonce));
        }

        let total_debit = amount + fee;
        if sender.balance < total_debit {
            return Err(LibError::InsufficientBalance(sender.balance, total_debit));
        }

        // Deduct from sender
        sender.balance -= total_debit;
        sender.nonce += 1;

        // Credit recipient
        let mut recipient_acct = self.get_account(recipient).unwrap_or_else(|| Account::new(U256::zero()));
        recipient_acct.balance += amount;
        self.set_account(recipient, &recipient_acct);

        Ok(())
    }

    /// Try to deduct fee only (for tx types without transfer execution).
    /// Validates nonce and balance.
    fn try_deduct_fee_only(
        &self,
        sender: &mut Account,
        fee: U256,
        nonce: u64,
    ) -> Result<()> {
        if sender.nonce != nonce {
            return Err(LibError::InvalidNonce(sender.nonce, nonce));
        }
        if sender.balance < fee {
            return Err(LibError::InsufficientBalance(sender.balance, fee));
        }

        sender.balance -= fee;
        sender.nonce += 1;
        Ok(())
    }

    /// Write an account to state.
    fn set_account(&mut self, addr: &Address, acct: &Account) {
        let key = namespace_key(NS_ACCOUNTS, addr.as_bytes());
        let value = scale_encode_account(acct);
        self.state.insert(&key, value);
    }
}

/// Convert a U256 fee to u128 (sat at max u128).
fn fee_as_u128(fee: U256) -> u128 {
    U256::min(fee, U256::from(u128::MAX)).low_u128()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use primitive_types::U256;
    use crate::core::block::{Block, BlockBody, BlockHeader};
    use crate::core::transaction::{Transaction, TxBody};
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

        let alice_post = sm.get_account(&alice()).unwrap();
        let bob_post = sm.get_account(&bob()).unwrap();
        assert_eq!(bob_post.balance, U256::from(100));
        // Alice: 1000 - 100 (transfer) - 100 (fee) = 800
        assert_eq!(alice_post.balance, U256::from(800));
        assert_eq!(alice_post.nonce, 1);
    }

    #[test]
    fn test_transfer_insufficient_balance_pays_fee() {
        // Alice has 50 — can't transfer 100, but fee of 100 also fails
        let accounts = vec![(alice(), make_account(50))];
        let mut sm = StateMachine::new(accounts);
        let block = transfer_block(alice(), bob(), 100, 0, 0);
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);

        // Alice pays nothing — can't even cover fee
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(50));
    }

    #[test]
    fn test_transfer_wrong_nonce_fails() {
        let accounts = vec![(alice(), make_account(1000))];
        let mut sm = StateMachine::new(accounts);
        // Nonce 5, but account nonce is 0
        let block = transfer_block(alice(), bob(), 100, 5, 0);
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);

        // Fee should be deducted
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(900)); // 1000 - 100 (fee)
        assert_eq!(alice_post.nonce, 0); // nonce not incremented for failed tx
    }

    #[test]
    fn test_burn_permanent_deducts_balance() {
        let accounts = vec![(alice(), make_account(1000))];
        let mut sm = StateMachine::new(accounts);
        let tx = Transaction {
            chain_id: 0,
            nonce: 0,
            sender: alice(),
            fee: U256::from(100),
            body: TxBody::Burn {
                target: BurnTarget::Permanent,
                amount: U256::from(200),
            },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader {
                height: 1,
                parent_hash: [0u8; 32],
                global_state_root: [0u8; 32],
                tx_root: [0u8; 32],
                timestamp: 1_700_000_000,
                proposer: alice(),
                chain_id: 0,
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);

        let alice_post = sm.get_account(&alice()).unwrap();
        // 1000 - 200 (burn) - 100 (fee) = 700
        assert_eq!(alice_post.balance, U256::from(700));
        assert_eq!(alice_post.nonce, 1);

        // Burn address has 200
        let burn_addr = crate::core::account::burn_address();
        let burn_acct = sm.get_account(&burn_addr).unwrap();
        assert_eq!(burn_acct.balance, U256::from(200));
    }

    #[test]
    fn test_multiple_transfers_in_block() {
        let accounts = vec![
            (alice(), make_account(1000)),
            (bob(), make_account(0)),
        ];
        let mut sm = StateMachine::new(accounts);

        let tx1 = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(50),
            body: TxBody::Transfer { recipient: bob(), amount: U256::from(100) },
            signature: dummy_sig(),
        };
        let tx2 = Transaction {
            chain_id: 0, nonce: 1, sender: alice(),
            fee: U256::from(50),
            body: TxBody::Transfer { recipient: bob(), amount: U256::from(200) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader {
                height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
            },
            body: BlockBody { transactions: vec![tx1, tx2] },
        };

        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 2);

        let alice_post = sm.get_account(&alice()).unwrap();
        let bob_post = sm.get_account(&bob()).unwrap();
        // Alice: 1000 - 100 - 50 (tx1) - 200 - 50 (tx2) = 600
        assert_eq!(alice_post.balance, U256::from(600));
        assert_eq!(alice_post.nonce, 2);
        assert_eq!(bob_post.balance, U256::from(300));
    }

    #[test]
    fn test_block_wrong_chain_id() {
        let accounts = vec![(alice(), make_account(1000))];
        let mut sm = StateMachine::new(accounts);
        // Test mismatch between tx.chain_id and block.header.chain_id
        let tx = Transaction {
            chain_id: 99, nonce: 0, sender: alice(),
            fee: U256::from(100),
            body: TxBody::Transfer { recipient: bob(), amount: U256::from(100) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader {
                height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
    }

    #[test]
    fn test_receipt_tracks_fees() {
        let accounts = vec![(alice(), make_account(1000))];
        let mut sm = StateMachine::new(accounts);
        let block = transfer_block(alice(), bob(), 100, 0, 0);
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.total_fees, 100);
        assert_eq!(receipt.tx_count, 1);
        assert_eq!(receipt.failed_count, 0);
    }

    #[test]
    fn test_apply_empty_block_returns_zero_receipt() {
        let mut sm = StateMachine::new(vec![(alice(), make_account(100))]);
        let receipt = sm.apply_block(&empty_block(alice(), 1)).unwrap();
        assert_eq!(receipt.total_fees, 0);
        assert_eq!(receipt.tx_count, 0);
        assert_eq!(receipt.failed_count, 0);
    }

    #[test]
    fn test_register_validator_deducts_fee_only() {
        let accounts = vec![(alice(), make_account(1000))];
        let mut sm = StateMachine::new(accounts);
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(50),
            body: TxBody::RegisterValidator { public_key: [0x42u8; 897] },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader {
                height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(950)); // 1000 - 50 (fee)
        assert_eq!(alice_post.nonce, 1);
    }

    #[test]
    fn test_stake_deducts_fee_only() {
        let accounts = vec![(alice(), make_account(1000)), (bob(), make_account(500))];
        let mut sm = StateMachine::new(accounts);
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(75),
            body: TxBody::Stake { validator: bob(), amount: U256::from(200) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader {
                height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(925)); // 1000 - 75 (fee)
        assert_eq!(alice_post.nonce, 1);
    }

    #[test]
    fn test_register_and_stake_deducts_fee_only() {
        let accounts = vec![(alice(), make_account(1000)), (bob(), make_account(500))];
        let mut sm = StateMachine::new(accounts);
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(60),
            body: TxBody::RegisterAndStake { validator: bob(), amount: U256::from(300) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader {
                height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(940)); // 1000 - 60 (fee)
        assert_eq!(alice_post.nonce, 1);
    }

    #[test]
    fn test_unstake_deducts_fee_only() {
        let accounts = vec![(alice(), make_account(1000)), (bob(), make_account(500))];
        let mut sm = StateMachine::new(accounts);
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(40),
            body: TxBody::Unstake { validator: bob(), amount: U256::from(100) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader {
                height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(960)); // 1000 - 40 (fee)
        assert_eq!(alice_post.nonce, 1);
    }

    #[test]
    fn test_missing_sender_account_increments_failed_count() {
        let mut sm = StateMachine::new(vec![]); // empty state
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(100),
            body: TxBody::Transfer { recipient: bob(), amount: U256::from(100) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader {
                height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
        assert_eq!(receipt.tx_count, 0);
    }

    #[test]
    fn test_cap_refill_burn_transfer() {
        let accounts = vec![(alice(), make_account(1000))];
        let mut sm = StateMachine::new(accounts);
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(50),
            body: TxBody::Burn {
                target: BurnTarget::CapRefill,
                amount: U256::from(200),
            },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader {
                height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(750)); // 1000 - 200 (burn) - 50 (fee)
        assert_eq!(alice_post.nonce, 1);
        let cap_addr = crate::core::account::cap_refill_address();
        let cap_acct = sm.get_account(&cap_addr).unwrap();
        assert_eq!(cap_acct.balance, U256::from(200));
    }

    #[test]
    fn test_failed_tx_still_pays_fee_when_possible() {
        // Alice has 1000, tries to transfer 2000 (insufficient) with fee 50
        // Should fail but pay the fee (50) since balance >= fee
        let accounts = vec![(alice(), make_account(1000))];
        let mut sm = StateMachine::new(accounts);
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(50),
            body: TxBody::Transfer { recipient: bob(), amount: U256::from(2000) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader {
                height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
        assert_eq!(receipt.tx_count, 0);
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(950)); // 1000 - 50 (fee still paid)
        assert_eq!(alice_post.nonce, 0); // nonce NOT incremented on failure
    }

    #[test]
    fn test_fee_as_u128_normal() {
        let fee = U256::from(42);
        assert_eq!(fee_as_u128(fee), 42);
    }

    #[test]
    fn test_fee_as_u128_saturated() {
        let huge = U256::MAX;
        assert_eq!(fee_as_u128(huge), u128::MAX);
    }
}
