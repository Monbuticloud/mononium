//! State machine — applies blocks, executes transactions, maintains state.
//!
//! The state machine is the core of the protocol. It holds all on-chain
//! state in a Sparse Merkle Tree and processes blocks deterministically.

use primitive_types::U256;

use crate::core::account::{Account, Address, scale_encode_account};
use crate::core::block::Block;
use crate::core::transaction::{BurnTarget, TxBody};
use parity_scale_codec::{Decode, Encode};
use crate::core::validator::{ValidatorEntry, ValidatorStatus};
use crate::crypto::trie::{SparseMerkleTree, namespace_key, NS_ACCOUNTS, NS_VALIDATORS};
use crate::error::{HexBytes, LibError, Result};

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

/// Result of applying a slashing (Phase 2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashResult {
    /// Total amount slashed (90% of pre-slash stake).
    pub slashed_amount: U256,
    /// Amount permanently burned (90% of slashed).
    pub burn_amount: U256,
    /// Bounty awarded to the reporter (10% of slashed).
    pub bounty_amount: U256,
    /// Stake remaining after slashing (10% of original).
    pub remaining_stake: U256,
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
    /// Active validator set for fee distribution. Set at era boundary.
    active_set: Vec<Address>,
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
        Self { state, active_set: vec![] }
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

    /// Retrieve a validator entry from state by address, if it exists.
    #[must_use]
    pub fn get_validator(&self, address: &Address) -> Option<ValidatorEntry> {
        let key = namespace_key(NS_VALIDATORS, address.as_bytes());
        let bytes = self.state.get(&key)?;
        Some(ValidatorEntry::decode(&mut &bytes[..]).ok()?)
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
                TxBody::RegisterValidator { public_key } => {
                    self.apply_register_validator(
                        &mut sender_acct,
                        &tx.sender,
                        public_key,
                        tx.fee,
                        tx.nonce,
                    )
                }
                TxBody::Stake { validator, amount } => {
                    self.apply_stake(
                        &mut sender_acct,
                        &tx.sender,
                        validator,
                        *amount,
                        tx.fee,
                        tx.nonce,
                    )
                }
                TxBody::RegisterAndStake { validator, amount } => {
                    self.apply_register_and_stake(
                        &mut sender_acct,
                        &tx.sender,
                        validator,
                        *amount,
                        tx.fee,
                        tx.nonce,
                    )
                }
                TxBody::Unstake { validator, amount } => {
                    self.apply_unstake(
                        &mut sender_acct,
                        &tx.sender,
                        validator,
                        *amount,
                        tx.fee,
                        tx.nonce,
                        0, // current_era = 0 for now
                    )
                }
                TxBody::Propose { proposal_id, title, description, actions } => {
                    self.apply_propose(&mut sender_acct, &tx.sender, proposal_id, title, description, actions, tx.fee, tx.nonce)
                }
                TxBody::Vote { proposal_id, approve } => {
                    self.apply_gov_vote(&mut sender_acct, &tx.sender, proposal_id, *approve, tx.fee, tx.nonce)
                }
                TxBody::CancelProposal { proposal_id } => {
                    self.apply_cancel_proposal(&mut sender_acct, &tx.sender, proposal_id, tx.fee, tx.nonce)
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

        // Distribute fees to active validators
        self.distribute_fees(block_fees);

        // TODO: state root verification

        Ok(BlockReceipt {
            total_fees: block_fees,
            tx_count,
            failed_count,
            state_root: self.state.root(),
        })
    }

    /// Set the active validator set (called at era boundary).
    /// Used by `distribute_fees` to determine who receives block rewards.
    pub fn set_active_set(&mut self, active_set: Vec<Address>) {
        self.active_set = active_set;
    }

    /// Get the current active validator set.
    pub fn active_set(&self) -> &[Address] {
        &self.active_set
    }

    /// Get the stake of a validator, or None if not registered.
    pub fn validator_stake(&self, address: &Address) -> Option<U256> {
        self.get_validator(address).map(|v| v.stake)
    }

    /// Distribute collected block fees to active validators proportionally
    /// by stake. Each validator's account balance is credited.
    fn distribute_fees(&mut self, total_fees: u128) {
        if total_fees == 0 || self.active_set.is_empty() {
            return;
        }
        let total = U256::from(total_fees);

        // Sum stakes and build (stake, address) pairs
        let mut pairs: Vec<(U256, Address)> = Vec::with_capacity(self.active_set.len());
        let mut total_stake = U256::zero();
        for addr in &self.active_set {
            if let Some(v) = self.get_validator(addr) {
                total_stake = total_stake.saturating_add(v.stake);
                pairs.push((v.stake, *addr));
            }
        }
        if total_stake.is_zero() {
            return;
        }

        // Sort by stake desc, then address asc for remainder rule
        pairs.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.as_bytes().cmp(b.1.as_bytes())));

        let mut distributed = U256::zero();
        for (stake, addr) in &pairs {
            let share = total * *stake / total_stake;
            if share.is_zero() {
                continue;
            }
            self.credit_balance(addr, share);
            distributed = distributed.saturating_add(share);
        }

        // Remainder goes to highest-stake validator (first after sort)
        if distributed < total {
            if let Some((_, addr)) = pairs.first() {
                self.credit_balance(addr, total - distributed);
            }
        }
    }

    /// Credit an account's balance (create account if missing).
    fn credit_balance(&mut self, addr: &Address, amount: U256) {
        let mut acct = self.get_account(addr).unwrap_or_else(|| Account::new(U256::zero()));
        acct.balance = acct.balance.saturating_add(amount);
        self.set_account(addr, &acct);
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

    /// Write a validator entry to state.
    pub(crate) fn set_validator(&mut self, addr: &Address, entry: &ValidatorEntry) {
        let key = namespace_key(NS_VALIDATORS, addr.as_bytes());
        let value = entry.encode();
        self.state.insert(&key, value);
    }

    // -- Generic SMT access (used by governance, future modules) ----------

    /// Insert a raw value into the governance namespace (`0x03 ++ sub_key`).
    pub(crate) fn governance_insert(&mut self, sub_key: &[u8], value: Vec<u8>) {
        let key = namespace_key(crate::crypto::trie::NS_GOVERNANCE, sub_key);
        self.state.insert(&key, value);
    }

    /// Retrieve a raw value from the governance namespace.
    pub(crate) fn governance_get(&self, sub_key: &[u8]) -> Option<Vec<u8>> {
        let key = namespace_key(crate::crypto::trie::NS_GOVERNANCE, sub_key);
        self.state.get(&key).map(|s| s.to_vec())
    }

    /// Total active stake: sum of stake for all `Active` validators.
    pub(crate) fn total_active_stake(&self) -> U256 {
        // Iterate all validators via list_keys in NS_VALIDATORS
        // For now, return zero (iteration not yet implemented in SMT)
        // TODO: implement iteration or cache
        U256::zero()
    }

    /// Sum of stake for a set of addresses.
    pub(crate) fn sum_stake(&self, addresses: &[Address]) -> U256 {
        let mut total = U256::zero();
        for addr in addresses {
            if let Some(entry) = self.get_validator(addr) {
                total = total.saturating_add(entry.stake);
            }
        }
        total
    }

    /// Apply a RegisterValidator transaction.
    ///
    /// Creates a `ValidatorEntry` with status `Registered` and deducts the
    /// fee + anti-spam deposit from the sender. Rejects if already registered.
    fn apply_register_validator(
        &mut self,
        sender: &mut Account,
        addr: &Address,
        public_key: &[u8; 897],
        fee: U256,
        nonce: u64,
    ) -> Result<()> {
        // Verify not already registered
        if self.get_validator(addr).is_some() {
            return Err(LibError::Consensus("already registered"));
        }
        // Verify nonce
        if sender.nonce != nonce {
            return Err(LibError::InvalidNonce(sender.nonce, nonce));
        }
        let total_debit = fee + crate::core::constants::ANTI_SPAM_DEPOSIT;
        if sender.balance < total_debit {
            return Err(LibError::InsufficientBalance(sender.balance, total_debit));
        }

        sender.balance -= total_debit;
        sender.nonce += 1;

        let entry = ValidatorEntry {
            address: *addr,
            public_key: *public_key,
            stake: U256::zero(),
            status: ValidatorStatus::Registered,
            registration_era: 0,
        };
        self.set_validator(addr, &entry);
        Ok(())
    }

    /// Apply a Stake transaction.
    ///
    /// Validates the target validator exists, is not frozen or unstaking,
    /// then deducts `amount + fee` from sender and adds `amount` to the
    /// validator's stake. Updates status to `Staked` if currently `Registered`.
    fn apply_stake(
        &mut self,
        sender: &mut Account,
        sender_addr: &Address,
        validator_addr: &Address,
        amount: U256,
        fee: U256,
        nonce: u64,
    ) -> Result<()> {
        // Verify nonce
        if sender.nonce != nonce {
            return Err(LibError::InvalidNonce(sender.nonce, nonce));
        }

        // Look up validator
        let Some(mut entry) = self.get_validator(validator_addr) else {
            return Err(LibError::ValidatorNotFound(
                HexBytes(validator_addr.into_bytes()),
            ));
        };

        // Verify validator is not frozen or unstaking
        match &entry.status {
            ValidatorStatus::Frozen { .. } | ValidatorStatus::Unstaking { .. } => {
                return Err(LibError::Consensus("validator is frozen or unstaking"));
            }
            _ => {}
        }

        // Verify amount > 0
        if amount.is_zero() {
            return Err(LibError::Consensus("stake amount must be > 0"));
        }

        // Verify sender can afford fee + amount
        let total_debit = fee + amount;
        if sender.balance < total_debit {
            return Err(LibError::InsufficientBalance(sender.balance, total_debit));
        }

        // Deduct from sender
        sender.balance -= total_debit;
        sender.nonce += 1;

        // Add to validator stake (defensive overflow check)
        let new_stake = entry.stake.checked_add(amount)
            .ok_or_else(|| LibError::Consensus("stake overflow"))?;
        entry.stake = new_stake;

        // Update status to Staked if currently Registered
        if entry.status == ValidatorStatus::Registered {
            entry.status = ValidatorStatus::Staked { stake: new_stake };
        }

        self.set_validator(validator_addr, &entry);
        Ok(())
    }

    /// Apply a RegisterAndStake transaction.
    ///
    /// Atomic: register self as validator, then stake to self. Single fee,
    /// single nonce increment, single deposit. If either step fails, the
    /// entire tx is rejected (no partial state).
    fn apply_register_and_stake(
        &mut self,
        sender: &mut Account,
        sender_addr: &Address,
        validator_addr: &Address,
        amount: U256,
        fee: U256,
        nonce: u64,
    ) -> Result<()> {
        // The validator must equal the sender (self-register + self-stake)
        if validator_addr != sender_addr {
            return Err(LibError::Consensus(
                "register-and-stake validator must equal sender",
            ));
        }

        // Verify nonce
        if sender.nonce != nonce {
            return Err(LibError::InvalidNonce(sender.nonce, nonce));
        }

        // Verify not already registered
        if self.get_validator(sender_addr).is_some() {
            return Err(LibError::Consensus("already registered"));
        }

        // Verify amount > 0
        if amount.is_zero() {
            return Err(LibError::Consensus("stake amount must be > 0"));
        }

        // Verify sender can afford fee + deposit + amount
        let total_debit = fee + crate::core::constants::ANTI_SPAM_DEPOSIT + amount;
        if sender.balance < total_debit {
            return Err(LibError::InsufficientBalance(sender.balance, total_debit));
        }

        // Deduct
        sender.balance -= total_debit;
        sender.nonce += 1;

        // Create validator entry with stake already applied
        let entry = ValidatorEntry {
            address: *sender_addr,
            public_key: [0u8; 897], // placeholder — real pubkey from tx body
            stake: amount,
            status: ValidatorStatus::Staked { stake: amount },
            registration_era: 0,
        };
        self.set_validator(sender_addr, &entry);
        Ok(())
    }

    /// Apply an Unstake transaction.
    ///
    /// Initiates withdrawal from a validator. Sets `release_era` to
    /// `current_era + UNSTAKING_COOLDOWN_ERAS` (168). Anyone can unstake
    /// from any validator — the unstaked amount goes to the *sender* at
    /// cooldown expiry, not the original staker.
    fn apply_unstake(
        &mut self,
        sender: &mut Account,
        sender_addr: &Address,
        validator_addr: &Address,
        amount: U256,
        fee: U256,
        nonce: u64,
        current_era: u64,
    ) -> Result<()> {
        // Verify nonce
        if sender.nonce != nonce {
            return Err(LibError::InvalidNonce(sender.nonce, nonce));
        }

        // Look up validator
        let Some(mut entry) = self.get_validator(validator_addr) else {
            return Err(LibError::ValidatorNotFound(
                HexBytes(validator_addr.into_bytes()),
            ));
        };

        // Verify validator is not frozen
        if matches!(entry.status, ValidatorStatus::Frozen { .. }) {
            return Err(LibError::Consensus("validator is frozen"));
        }

        // Verify amount > 0
        if amount.is_zero() {
            return Err(LibError::Consensus("unstake amount must be > 0"));
        }

        // Verify amount ≤ validator.stake
        if amount > entry.stake {
            return Err(LibError::Consensus("unstake amount exceeds validator stake"));
        }

        // Verify sender can afford fee
        if sender.balance < fee {
            return Err(LibError::InsufficientBalance(sender.balance, fee));
        }

        // Deduct fee from sender
        sender.balance -= fee;
        sender.nonce += 1;

        // Set validator to Unstaking status
        let release_era = current_era + crate::core::constants::UNSTAKING_COOLDOWN_ERAS;

        // Allow nested unstaking: if already Unstaking, add to existing amount
        match &entry.status {
            ValidatorStatus::Unstaking { release_era: existing_era, amount: existing_amount } => {
                let new_amount = existing_amount.checked_add(amount)
                    .ok_or_else(|| LibError::Consensus("unstake overflow"))?;
                if new_amount > entry.stake {
                    return Err(LibError::Consensus(
                        "cumulative unstake exceeds validator stake",
                    ));
                }
                entry.status = ValidatorStatus::Unstaking {
                    release_era: *existing_era,
                    amount: new_amount,
                };
            }
            _ => {
                entry.status = ValidatorStatus::Unstaking {
                    release_era,
                    amount,
                };
            }
        }

        self.set_validator(validator_addr, &entry);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Era boundary hooks
    // -----------------------------------------------------------------------

    /// Process unstaking cooldown for a single validator.
    ///
    /// If `release_era <= current_era`, the unstaking completes:
    /// - Full unstake (amount == stake) → remove entry (Inactive)
    /// - Partial unstake (amount < stake) → reduce stake, set status to
    ///   `Staked` (if remaining ≥ 1 MONEX) or `Registered` (if < 1 MONEX)
    pub fn process_unstaking_cooldown(
        &mut self,
        validator_addr: &Address,
        current_era: u64,
    ) -> Result<()> {
        let Some(entry) = self.get_validator(validator_addr) else {
            return Ok(()); // silently skip non-existent
        };

        let ValidatorStatus::Unstaking { release_era, amount } = entry.status else {
            return Ok(()); // not unstaking, skip
        };

        if release_era > current_era {
            return Ok(()); // cooldown not yet expired
        }

        let min_stake = crate::core::constants::MIN_STAKE;

        if amount >= entry.stake {
            // Full unstake — set to Registered with 0 stake (effectively Inactive)
            let new_entry = ValidatorEntry {
                stake: U256::zero(),
                status: ValidatorStatus::Registered,
                ..entry
            };
            self.set_validator(validator_addr, &new_entry);
        } else {
            // Partial unstake
            let new_stake = entry.stake - amount;
            let new_entry = ValidatorEntry {
                status: if new_stake >= min_stake {
                    ValidatorStatus::Staked { stake: new_stake }
                } else {
                    ValidatorStatus::Registered
                },
                stake: new_stake,
                ..entry
            };
            self.set_validator(validator_addr, &new_entry);
        }
        Ok(())
    }

    /// Process thaw for a single frozen validator.
    ///
    /// If `frozen_until <= current_era`, the validator thaws:
    /// - If stake ≥ 1 MONEX → status = `Thawed` (re-enters candidate pool)
    /// - If stake < 1 MONEX → status = `Registered` (no stake)
    /// Process a single frozen validator at era boundary.
    ///
    /// Returns `Ok(true)` if the validator was actually thawed,
    /// `Ok(false)` if nothing changed (not frozen, frozen_until > era, or absent).
    pub fn process_thaw(&mut self, validator_addr: &Address, current_era: u64) -> Result<bool> {
        let Some(entry) = self.get_validator(validator_addr) else {
            return Ok(false);
        };

        let ValidatorStatus::Frozen { frozen_until } = entry.status else {
            return Ok(false);
        };

        if frozen_until > current_era {
            return Ok(false);
        }

        let min_stake = crate::core::constants::MIN_STAKE;
        let new_status = if entry.stake >= min_stake {
            ValidatorStatus::Thawed
        } else {
            ValidatorStatus::Registered
        };

        let new_entry = ValidatorEntry {
            status: new_status,
            ..entry
        };
        self.set_validator(validator_addr, &new_entry);
        Ok(true)
    }

    /// Bulk-thaw all frozen validators whose freeze period has expired.
    ///
    /// At era boundary, the caller passes all known validator addresses.
    /// Returns the number of validators that were actually thawed.
    pub fn thaw_all(&mut self, addrs: &[Address], current_era: u64) -> usize {
        let mut count = 0;
        for addr in addrs {
            if self.process_thaw(addr, current_era).unwrap_or(false) {
                count += 1;
            }
        }
        count
    }

    /// Run validator election from a list of candidates.
    ///
    /// `era == 0`: Open election — all non-Frozen candidates become Active,
    /// first `max_validators` by registration order.
    ///
    /// `era >= 1`: Top-N election — candidates with stake ≥ 1 MONEX are
    /// sorted by stake (desc), ties broken by earliest registration_era.
    /// Top `max_validators` become Active. Previously Active but not elected
    /// revert to Staked.
    ///
    /// Returns the addresses of the newly elected active set.
    pub fn run_election(
        &mut self,
        candidates: &[Address],
        max_validators: usize,
        era: u64,
    ) -> Vec<Address> {
        let min_stake = crate::core::constants::MIN_STAKE;
        let mut eligible: Vec<ValidatorEntry> = Vec::new();

        for addr in candidates {
            if let Some(entry) = self.get_validator(addr) {
                // Exclude Frozen and Unstaking
                if matches!(
                    entry.status,
                    ValidatorStatus::Frozen { .. } | ValidatorStatus::Unstaking { .. }
                ) {
                    continue;
                }
                eligible.push(entry);
            }
        }

        let elected = if era == 0 {
            // Era 0: Open — first max_validators by registration order,
            // exclude Frozen/Unstaking (already filtered above)
            eligible.into_iter().take(max_validators).collect::<Vec<_>>()
        } else {
            // Era 1+: Top-N by stake
            eligible.sort_by(|a, b| {
                b.stake.cmp(&a.stake)
                    .then_with(|| a.registration_era.cmp(&b.registration_era))
            });
            eligible.into_iter()
                .filter(|e| e.stake >= min_stake)
                .take(max_validators)
                .collect::<Vec<_>>()
        };

        let active_addrs: Vec<Address> = elected.iter().map(|e| e.address).collect();

        // Set elected validators to Active
        for entry in elected {
            let new_status = match entry.status {
                ValidatorStatus::Registered | ValidatorStatus::Staked { .. }
                | ValidatorStatus::Thawed => ValidatorStatus::Active,
                other => other, // shouldn't happen, but preserve
            };
            let new_entry = ValidatorEntry {
                status: new_status,
                ..entry
            };
            self.set_validator(&new_entry.address, &new_entry);
        }

        active_addrs
    }

    // -- Slashing ---------------------------------------------------------

    /// Apply an equivocation slashing.
    ///
    /// Slashes 90% of the validator's stake: 90% of slashed → burned,
    /// 10% → reporter bounty. Sets validator status to Frozen for 72 eras.
    ///
    /// Returns `LibError::AlreadyFrozen` if the validator is already frozen.
    #[allow(unused_variables)]
    pub fn apply_slash(
        &mut self,
        proposer_addr: &[u8; 32],
        reporter_addr: &Address,
        current_era: u64,
    ) -> Result<SlashResult> {
        let addr = Address::from(*proposer_addr);
        let Some(mut entry) = self.get_validator(&addr) else {
            return Err(LibError::ValidatorNotFound(HexBytes(*proposer_addr)));
        };

        // Already frozen → ignore secondary evidence
        if matches!(entry.status, ValidatorStatus::Frozen { .. }) {
            return Err(LibError::AlreadyFrozen);
        }

        // Compute slash amounts: 90% of total stake
        let slashed = entry.stake * U256::from(90) / U256::from(100);
        let burn_amount = slashed * U256::from(90) / U256::from(100); // 81% of original
        let bounty = slashed * U256::from(10) / U256::from(100);      // 9% of original
        let remaining = entry.stake - slashed;                         // 10% of original

        // Burn to 0x00..00 (permanent destruction)
        let burn_addr = crate::core::account::burn_address();
        if let Some(mut burn_acct) = self.get_account(&burn_addr) {
            burn_acct.balance = burn_acct.balance.saturating_add(burn_amount);
            self.set_account(&burn_addr, &burn_acct);
        } else {
            // Create burn account if it doesn't exist
            let mut acct = Account::new(U256::zero());
            acct.balance = burn_amount;
            self.set_account(&burn_addr, &acct);
        }

        // Credit reporter bounty
        if let Some(mut reporter_acct) = self.get_account(reporter_addr) {
            reporter_acct.balance = reporter_acct.balance.saturating_add(bounty);
            self.set_account(reporter_addr, &reporter_acct);
        } else {
            let mut acct = Account::new(U256::zero());
            acct.balance = bounty;
            self.set_account(reporter_addr, &acct);
        }

        // Update validator: partial stake + frozen status
        entry.stake = remaining;
        entry.status = ValidatorStatus::Frozen {
            frozen_until: current_era + 72,
        };
        self.set_validator(&addr, &entry);

        Ok(SlashResult {
            slashed_amount: slashed,
            burn_amount,
            bounty_amount: bounty,
            remaining_stake: remaining,
        })
    }

    // -- Governance stubs (Phase 2.5 — GovernanceEngine will replace) -----

    /// Submit a governance proposal.
    #[allow(unused_variables)]
    fn apply_propose(
        &mut self,
        sender: &mut Account,
        sender_addr: &Address,
        proposal_id: &[u8; 32],
        title: &[u8],
        description: &[u8],
        actions: &[crate::governance::types::GovernanceAction],
        fee: U256,
        nonce: u64,
    ) -> Result<()> {
        // TODO: GovernanceEngine integration
        if sender.nonce != nonce {
            return Err(LibError::InvalidNonce(sender.nonce, nonce));
        }
        if sender.balance < fee {
            return Err(LibError::InsufficientBalance(sender.balance, fee));
        }
        sender.balance = sender.balance.saturating_sub(fee);
        sender.nonce += 1;
        Ok(())
    }

    /// Cast a vote on a governance proposal.
    #[allow(unused_variables)]
    fn apply_gov_vote(
        &mut self,
        sender: &mut Account,
        sender_addr: &Address,
        proposal_id: &[u8; 32],
        approve: bool,
        fee: U256,
        nonce: u64,
    ) -> Result<()> {
        // TODO: GovernanceEngine integration
        if sender.nonce != nonce {
            return Err(LibError::InvalidNonce(sender.nonce, nonce));
        }
        if sender.balance < fee {
            return Err(LibError::InsufficientBalance(sender.balance, fee));
        }
        sender.balance = sender.balance.saturating_sub(fee);
        sender.nonce += 1;
        Ok(())
    }

    /// Cancel a governance proposal.
    #[allow(unused_variables)]
    fn apply_cancel_proposal(
        &mut self,
        sender: &mut Account,
        sender_addr: &Address,
        proposal_id: &[u8; 32],
        fee: U256,
        nonce: u64,
    ) -> Result<()> {
        // TODO: GovernanceEngine integration
        if sender.nonce != nonce {
            return Err(LibError::InvalidNonce(sender.nonce, nonce));
        }
        if sender.balance < fee {
            return Err(LibError::InsufficientBalance(sender.balance, fee));
        }
        sender.balance = sender.balance.saturating_sub(fee);
        sender.nonce += 1;
        Ok(())
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
    use crate::crypto::constants::{FALCON_SIGNATURE_SIZE, FALCON_PUBLIC_KEY_SIZE};

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
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
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
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
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
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
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
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
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
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
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
    fn test_register_validator_deducts_fee_and_deposit() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        // Give Alice enough balance via set_account
        let mut acct = sm.get_account(&alice()).unwrap();
        acct.balance = U256::from(1000) + deposit;
        let key = crate::crypto::trie::namespace_key(
            crate::crypto::trie::NS_ACCOUNTS,
            alice().as_bytes(),
        );
        let value = crate::core::account::scale_encode_account(&acct);
        sm.state.insert(&key, value);
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
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(950)); // (1000 + deposit) - 50(fee) - deposit = 950
        assert_eq!(alice_post.nonce, 1);
    }

    #[test]
    fn test_stake_deducts_fee_and_amount() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0)), (bob(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(2000) + deposit);
        setup_bob_with_balance(&mut sm, U256::from(1000) + deposit);
        // Register Bob as validator
        create_validator(&mut sm, bob(), [0x42u8; 897], 0, 0);

        // Alice stakes 200 to Bob
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(75),
            body: TxBody::Stake { validator: bob(), amount: U256::from(200) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader {
                height: 2, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_001, proposer: bob(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        // Alice never registered (only Bob did), so no deposit deducted from Alice
        // Alice: (2000 + deposit) - 200(stake) - 75(fee) = 1725 + deposit
        assert_eq!(alice_post.balance, U256::from(1725) + deposit);
        assert_eq!(alice_post.nonce, 1);

        // Bob's validator now has 200 stake
        let entry = sm.get_validator(&bob()).unwrap();
        assert_eq!(entry.stake, U256::from(200));
    }

    // -----------------------------------------------------------------------
    // RegisterValidator error paths
    // -----------------------------------------------------------------------

    fn setup_alice_with_balance(sm: &mut StateMachine, balance: U256) {
        let mut acct = sm.get_account(&alice()).unwrap();
        acct.balance = balance;
        let key = crate::crypto::trie::namespace_key(
            crate::crypto::trie::NS_ACCOUNTS,
            alice().as_bytes(),
        );
        let value = crate::core::account::scale_encode_account(&acct);
        sm.state.insert(&key, value);
    }

    fn make_register_tx(nonce: u64, fee: u64) -> Transaction {
        Transaction {
            chain_id: 0, nonce, sender: alice(),
            fee: U256::from(fee),
            body: TxBody::RegisterValidator { public_key: [0x42u8; 897] },
            signature: dummy_sig(),
        }
    }

    #[test]
    fn test_register_validator_already_registered() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(2000) + deposit);

        // First registration succeeds (nonce 0)
        let block1 = Block {
            header: BlockHeader { height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![make_register_tx(0, 50)] },
        };
        sm.apply_block(&block1).unwrap();

        // Second registration with correct nonce (1) — fails because already registered
        let tx2 = Transaction {
            chain_id: 0, nonce: 1, sender: alice(),
            fee: U256::from(50),
            body: TxBody::RegisterValidator { public_key: [0x43u8; 897] },
            signature: dummy_sig(),
        };
        let block2 = Block {
            header: BlockHeader { height: 2, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_001, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx2] },
        };
        let receipt = sm.apply_block(&block2).unwrap();
        assert_eq!(receipt.failed_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        // After block1: (2000 + deposit) - 50 - deposit = 1950
        // After block2: fee-only 50 deducted → 1900
        assert_eq!(alice_post.balance, U256::from(1900));
        assert_eq!(alice_post.nonce, 1); // nonce NOT incremented on failure
    }

    #[test]
    fn test_register_validator_invalid_nonce() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(1000) + deposit);

        // Nonce 5 but account nonce is 0
        let block = Block {
            header: BlockHeader { height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![make_register_tx(5, 50)] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        // fee-only: balance >= fee, so (1000 + deposit) - 50 = 950 + deposit
        assert_eq!(alice_post.balance, U256::from(950) + deposit);
        assert_eq!(alice_post.nonce, 0);
    }

    #[test]
    fn test_register_validator_insufficient_balance() {
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(100));

        // 100 < 50 + deposit → execution fails, but fee-only path deducts 50
        let block = Block {
            header: BlockHeader { height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![make_register_tx(0, 50)] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(50)); // 100 - 50 (fee-only)
        assert_eq!(alice_post.nonce, 0);
    }

    #[test]
    fn test_register_validator_cannot_cover_fee() {
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        // Only 30 — can't even cover fee of 50
        setup_alice_with_balance(&mut sm, U256::from(30));

        let block = Block {
            header: BlockHeader { height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![make_register_tx(0, 50)] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        // 30 < 50, so even fee-only deduction fails — balance unchanged
        assert_eq!(alice_post.balance, U256::from(30));
        assert_eq!(alice_post.nonce, 0);
    }

    // -----------------------------------------------------------------------
    // Stake happy path
    // -----------------------------------------------------------------------

    fn setup_bob_with_balance(sm: &mut StateMachine, balance: U256) {
        let mut acct = sm.get_account(&bob()).unwrap();
        acct.balance = balance;
        let key = crate::crypto::trie::namespace_key(
            crate::crypto::trie::NS_ACCOUNTS,
            bob().as_bytes(),
        );
        let value = crate::core::account::scale_encode_account(&acct);
        sm.state.insert(&key, value);
    }

    #[test]
    fn test_stake_increases_validator_stake() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0)), (bob(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        // Alice registers, Bob stakes to Alice
        setup_alice_with_balance(&mut sm, U256::from(1000) + deposit);
        setup_bob_with_balance(&mut sm, U256::from(1000));
        create_validator(&mut sm, alice(), [0x42u8; 897], 0, 0);

        // Bob stakes 500 to Alice
        let stake_amount = U256::from(500);
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: bob(),
            fee: U256::from(50),
            body: TxBody::Stake { validator: alice(), amount: stake_amount },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 2, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_001, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);

        let entry = sm.get_validator(&alice()).unwrap();
        assert_eq!(entry.stake, stake_amount);
        assert_eq!(entry.status, ValidatorStatus::Staked { stake: stake_amount });

        // Bob's balance decreased by fee + amount
        let bob_post = sm.get_account(&bob()).unwrap();
        assert_eq!(bob_post.balance, U256::from(1000) - stake_amount - U256::from(50));
        assert_eq!(bob_post.nonce, 1);
    }

    #[test]
    fn test_stake_self_stake_allowed() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(2000) + deposit);
        create_validator(&mut sm, alice(), [0x42u8; 897], 0, 0);

        // Self-stake: Alice stakes to herself
        let stake_amount = U256::from(300);
        let tx = Transaction {
            chain_id: 0, nonce: 1, sender: alice(),
            fee: U256::from(50),
            body: TxBody::Stake { validator: alice(), amount: stake_amount },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 2, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_001, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);

        let entry = sm.get_validator(&alice()).unwrap();
        assert_eq!(entry.stake, stake_amount);
        let alice_post = sm.get_account(&alice()).unwrap();
        // Alice's balance after registration: (2000 + deposit) - 0(fee) - deposit = 2000
        // After self-stake: 2000 - 300(stake) - 50(fee) = 1650
        assert_eq!(alice_post.balance, U256::from(1650));
    }

    // -----------------------------------------------------------------------
    // Stake error paths
    // -----------------------------------------------------------------------

    #[test]
    fn test_stake_nonexistent_validator() {
        let accounts = vec![(alice(), make_account(1000))];
        let mut sm = StateMachine::new(accounts);

        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(50),
            body: TxBody::Stake { validator: bob(), amount: U256::from(100) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(950)); // 1000 - 50 (fee-only)
        assert_eq!(alice_post.nonce, 0);
    }

    #[test]
    fn test_stake_amount_zero() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(1000) + deposit);
        create_validator(&mut sm, alice(), [0x42u8; 897], 0, 0);

        let tx = Transaction {
            chain_id: 0, nonce: 1, sender: alice(),
            fee: U256::from(50),
            body: TxBody::Stake { validator: alice(), amount: U256::zero() },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 2, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_001, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        // Alice after register: (1000 + deposit) - 50 - deposit = 950
        // After failed stake: fee-only 50 → 900
        // After register: (1000 + deposit) - 0(fee) - deposit = 1000
        // After failed stake: fee-only 50 deducted → 950
        assert_eq!(alice_post.balance, U256::from(950));
    }
    #[test]
    fn test_stake_to_frozen_validator() {
        // This tests the status check — we create a frozen validator entry directly
        let accounts = vec![(alice(), make_account(1000))];
        let mut sm = StateMachine::new(accounts);
        // Manually insert a frozen validator
        let entry = ValidatorEntry {
            address: bob(),
            public_key: [0x42u8; 897],
            stake: U256::from(500),
            status: ValidatorStatus::Frozen { frozen_until: 720 },
            registration_era: 0,
        };
        let key = crate::crypto::trie::namespace_key(
            crate::crypto::trie::NS_VALIDATORS,
            bob().as_bytes(),
        );
        let value = entry.encode();
        sm.state.insert(&key, value);

        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(50),
            body: TxBody::Stake { validator: bob(), amount: U256::from(100) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(950)); // 1000 - 50
    }

    /// Helper: register Alice as a validator with the given public key.
    fn create_validator(
        sm: &mut StateMachine,
        addr: Address,
        pk: [u8; 897],
        nonce: u64,
        fee: u64,
    ) {
        let tx = Transaction {
            chain_id: 0, nonce, sender: addr,
            fee: U256::from(fee),
            body: TxBody::RegisterValidator { public_key: pk },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: addr, chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        sm.apply_block(&block).unwrap();
    }

    // -----------------------------------------------------------------------
    // RegisterAndStake tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_and_stake_happy() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(2000) + deposit);

        // Alice registers and stakes to herself in one tx
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(50),
            body: TxBody::RegisterAndStake { validator: alice(), amount: U256::from(500) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);

        let entry = sm.get_validator(&alice()).unwrap();
        assert_eq!(entry.stake, U256::from(500));
        assert_eq!(entry.status, ValidatorStatus::Staked { stake: U256::from(500) });
        assert_eq!(entry.registration_era, 0);

        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(1450)); // 2000 + deposit - 50 - deposit - 500 = 1450
        assert_eq!(alice_post.nonce, 1);
    }

    #[test]
    fn test_register_and_stake_validator_must_equal_sender() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(2000) + deposit);

        // Alice tries to register Bob (different address) — should fail
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(60),
            body: TxBody::RegisterAndStake { validator: bob(), amount: U256::from(300) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        // fee-only: (2000 + deposit) - 60 = 1940 + deposit
        assert_eq!(alice_post.balance, U256::from(1940) + deposit);
        assert_eq!(alice_post.nonce, 0);
    }

    #[test]
    fn test_register_and_stake_already_registered() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(3000) + deposit);
        // First register Alice
        create_validator(&mut sm, alice(), [0x42u8; 897], 0, 0);

        // Now try register-and-stake again — should fail (already registered)
        let tx = Transaction {
            chain_id: 0, nonce: 1, sender: alice(),
            fee: U256::from(50),
            body: TxBody::RegisterAndStake { validator: alice(), amount: U256::from(100) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 2, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_001, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        // After register: (3000 + deposit) - 0 - deposit = 3000
        // After failed: fee-only 50 → 2950
        assert_eq!(alice_post.balance, U256::from(2950));
        assert_eq!(alice_post.nonce, 1);
    }

    // -----------------------------------------------------------------------
    // Unstake tests
    // -----------------------------------------------------------------------

    fn make_staked_validator(
        sm: &mut StateMachine,
        addr: Address,
        _pk: [u8; 897],
        stake_amount: U256,
    ) {
        // Register via RegisterAndStake atomic tx
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: addr,
            fee: U256::from(0),
            body: TxBody::RegisterAndStake { validator: addr, amount: stake_amount },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: addr, chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        sm.apply_block(&block).unwrap();
    }

    fn insert_frozen(sm: &mut StateMachine, addr: Address, stake: U256, frozen_until: u64) {
        let entry = ValidatorEntry {
            address: addr,
            public_key: [0x42u8; 897],
            stake,
            status: ValidatorStatus::Frozen { frozen_until },
            registration_era: 0,
        };
        let key = namespace_key(NS_VALIDATORS, addr.as_bytes());
        sm.state.insert(&key, entry.encode());
    }

    fn insert_test_validator(
        sm: &mut StateMachine,
        addr: Address,
        stake: U256,
        status: ValidatorStatus,
    ) {
        let entry = ValidatorEntry {
            address: addr,
            public_key: [0x42u8; 897],
            stake,
            status,
            registration_era: 0,
        };
        let key = namespace_key(NS_VALIDATORS, addr.as_bytes());
        sm.state.insert(&key, entry.encode());
    }

    #[test]
    fn test_unstake_sets_release_era() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(2000) + deposit);
        make_staked_validator(&mut sm, alice(), [0x42u8; 897], U256::from(500));

        // Alice unstakes 200 from herself
        let tx = Transaction {
            chain_id: 0, nonce: 1, sender: alice(),
            fee: U256::from(40),
            body: TxBody::Unstake { validator: alice(), amount: U256::from(200) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 2, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_001, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);

        let entry = sm.get_validator(&alice()).unwrap();
        assert_eq!(
            entry.status,
            ValidatorStatus::Unstaking {
                release_era: 168,
                amount: U256::from(200),
            }
        );
        // Stake should be unchanged (remains 500 until cooldown)
        assert_eq!(entry.stake, U256::from(500));
    }

    #[test]
    fn test_unstake_by_other_sender() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0)), (bob(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(2000) + deposit);
        setup_bob_with_balance(&mut sm, U256::from(1000));
        make_staked_validator(&mut sm, alice(), [0x42u8; 897], U256::from(500));

        // Bob (not Alice) unstakes 100 from Alice
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: bob(),
            fee: U256::from(30),
            body: TxBody::Unstake { validator: alice(), amount: U256::from(100) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 2, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_001, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);

        let entry = sm.get_validator(&alice()).unwrap();
        assert_eq!(
            entry.status,
            ValidatorStatus::Unstaking {
                release_era: 168,
                amount: U256::from(100),
            }
        );
    }

    #[test]
    fn test_unstake_nonexistent_validator() {
        let accounts = vec![(alice(), make_account(1000))];
        let mut sm = StateMachine::new(accounts);
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: alice(),
            fee: U256::from(40),
            body: TxBody::Unstake { validator: bob(), amount: U256::from(100) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
        let alice_post = sm.get_account(&alice()).unwrap();
        assert_eq!(alice_post.balance, U256::from(960)); // 1000 - 40 (fee-only)
        assert_eq!(alice_post.nonce, 0);
    }

    #[test]
    fn test_unstake_amount_exceeds_stake() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(2000) + deposit);
        make_staked_validator(&mut sm, alice(), [0x42u8; 897], U256::from(300));

        // Try to unstake 500, but stake is only 300
        let tx = Transaction {
            chain_id: 0, nonce: 1, sender: alice(),
            fee: U256::from(40),
            body: TxBody::Unstake { validator: alice(), amount: U256::from(500) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 2, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_001, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
    }

    #[test]
    fn test_unstake_amount_zero() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice(), make_account(0))];
        let mut sm = StateMachine::new(accounts);
        setup_alice_with_balance(&mut sm, U256::from(2000) + deposit);
        make_staked_validator(&mut sm, alice(), [0x42u8; 897], U256::from(300));

        let tx = Transaction {
            chain_id: 0, nonce: 1, sender: alice(),
            fee: U256::from(40),
            body: TxBody::Unstake { validator: alice(), amount: U256::zero() },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 2, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_001, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.failed_count, 1);
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
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
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
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
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
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
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

    // -----------------------------------------------------------------------
    // RegisterValidator
    // -----------------------------------------------------------------------

    #[test]
    fn test_register_validator_creates_entry() {
        let alice_addr = alice();
        let pk = [0x42u8; FALCON_PUBLIC_KEY_SIZE];
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let accounts = vec![(alice_addr, make_account(0))];
        let mut sm = StateMachine::new(accounts);
        // Give Alice enough for fee + deposit (use set_account via raw SMT)
        let mut acct = sm.get_account(&alice_addr).unwrap();
        acct.balance = U256::from(1000) + deposit;
        let key = crate::crypto::trie::namespace_key(
            crate::crypto::trie::NS_ACCOUNTS,
            alice_addr.as_bytes(),
        );
        let value = crate::core::account::scale_encode_account(&acct);
        sm.state.insert(&key, value);

        let tx = Transaction {
            chain_id: 0,
            nonce: 0,
            sender: alice_addr,
            fee: U256::from(50),
            body: TxBody::RegisterValidator { public_key: pk },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader {
                height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: alice_addr, chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };

        let receipt = sm.apply_block(&block).unwrap();
        assert_eq!(receipt.tx_count, 1);

        // ValidatorEntry should exist in state
        let entry = sm.get_validator(&alice_addr).unwrap();
        assert_eq!(entry.address, alice_addr);
        assert_eq!(entry.public_key, pk);
        assert_eq!(entry.status, ValidatorStatus::Registered);
        assert_eq!(entry.registration_era, 0);
        assert_eq!(entry.stake, U256::zero());

        // Alice: balance decreased by fee + deposit
        let alice_post = sm.get_account(&alice_addr).unwrap();
        let expected = (U256::from(1000) + deposit) - U256::from(50) - deposit;
        assert_eq!(alice_post.balance, expected);
        assert_eq!(alice_post.nonce, 1);
    }

    // -----------------------------------------------------------------------
    // Era boundary tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_unstaking_cooldown_full() {
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let mut sm = StateMachine::new(vec![(alice(), make_account(0))]);
        setup_alice_with_balance(&mut sm, U256::from(2000) + deposit);
        make_staked_validator(&mut sm, alice(), [0x42u8; 897], U256::from(500));

        // Unstake full amount (500)
        let tx = Transaction {
            chain_id: 0, nonce: 1, sender: alice(),
            fee: U256::from(40),
            body: TxBody::Unstake { validator: alice(), amount: U256::from(500) },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 2, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_001, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        sm.apply_block(&block).unwrap();

        // Cooldown at era 167 → not yet expired
        sm.process_unstaking_cooldown(&alice(), 167).unwrap();
        assert!(sm.get_validator(&alice()).is_some()); // still exists

        // Cooldown at era 168+ → full unstake, entry becomes Registered with 0 stake
        sm.process_unstaking_cooldown(&alice(), 168).unwrap();
        let entry = sm.get_validator(&alice()).unwrap();
        assert_eq!(entry.stake, U256::zero());
        assert_eq!(entry.status, ValidatorStatus::Registered);
    }

    #[test]
    fn test_unstaking_cooldown_partial() {
        let one_monex = crate::core::constants::ONE_MONEX;
        let deposit = crate::core::constants::ANTI_SPAM_DEPOSIT;
        let initial = one_monex * U256::from(100) + deposit;
        let mut sm = StateMachine::new(vec![(alice(), make_account(0))]);
        setup_alice_with_balance(&mut sm, initial);
        make_staked_validator(&mut sm, alice(), [0x42u8; 897], U256::from(50) * crate::core::constants::ONE_MONEX);

        // Unstake 20 MONEX (partial)
        let unstake_amount = U256::from(20) * one_monex;
        let tx = Transaction {
            chain_id: 0, nonce: 1, sender: alice(),
            fee: U256::from(40),
            body: TxBody::Unstake { validator: alice(), amount: unstake_amount },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 2, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_001, proposer: alice(), chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        sm.apply_block(&block).unwrap();

        // Cooldown at era 168
        sm.process_unstaking_cooldown(&alice(), 168).unwrap();
        let entry = sm.get_validator(&alice()).unwrap();
        // 50 - 20 = 30 MONEX remaining (>= 1 MONEX → Staked)
        assert_eq!(entry.stake, U256::from(30) * one_monex);
        assert_eq!(
            entry.status,
            ValidatorStatus::Staked { stake: U256::from(30) * one_monex }
        );
    }

    #[test]
    fn test_thaw_updates_status() {
        let one_monex = crate::core::constants::ONE_MONEX;
        let mut sm = StateMachine::new(vec![]);
        // Manually insert a frozen validator with >= 1 MONEX stake
        let entry = ValidatorEntry {
            address: alice(),
            public_key: [0x42u8; 897],
            stake: one_monex * U256::from(5),
            status: ValidatorStatus::Frozen { frozen_until: 720 },
            registration_era: 0,
        };
        let key = namespace_key(NS_VALIDATORS, alice().as_bytes());
        sm.state.insert(&key, entry.encode());

        // Thaw at era 719 → not yet
        sm.process_thaw(&alice(), 719).unwrap();
        let entry = sm.get_validator(&alice()).unwrap();
        assert_eq!(
            entry.status,
            ValidatorStatus::Frozen { frozen_until: 720 }
        );

        // Thaw at era 720+ → thawed
        sm.process_thaw(&alice(), 720).unwrap();
        let entry = sm.get_validator(&alice()).unwrap();
        assert_eq!(entry.status, ValidatorStatus::Thawed);
    }

    #[test]
    fn test_thaw_all_bulk() {
        let one_monex = crate::core::constants::ONE_MONEX;
        let mut sm = StateMachine::new(vec![]);
        let al = alice();
        let bob = bob();
        let charlie = Address::from([2u8; 32]);

        // al: Frozen, due at 700, stake >= 1 MONEX → Thawed
        insert_frozen(&mut sm, al, one_monex * 5, 700);
        // bob: Frozen, due at 720, stake < 1 MONEX → Registered
        insert_frozen(&mut sm, bob, one_monex / 2, 700);
        // charlie: Active (not frozen) → unchanged
        insert_test_validator(&mut sm, charlie, one_monex * 10, ValidatorStatus::Active);

        let count = sm.thaw_all(&[al, bob, charlie], 700);
        assert_eq!(count, 2, "alice + bob thawed, charlie unfrozen");

        assert_eq!(sm.get_validator(&al).unwrap().status, ValidatorStatus::Thawed);
        assert_eq!(sm.get_validator(&bob).unwrap().status, ValidatorStatus::Registered);
        assert_eq!(
            sm.get_validator(&charlie).unwrap().status,
            ValidatorStatus::Active,
        );
    }

    #[test]
    fn test_thaw_all_empty() {
        let mut sm = StateMachine::new(vec![]);
        assert_eq!(sm.thaw_all(&[], 999), 0);
    }

    #[test]
    fn test_thaw_all_not_due() {
        let mut sm = StateMachine::new(vec![]);
        let al = alice();
        insert_frozen(&mut sm, al, crate::core::constants::ONE_MONEX * 5, 720);

        let count = sm.thaw_all(&[al], 700);
        assert_eq!(count, 0, "not due yet");
        // Still frozen
        assert!(matches!(
            sm.get_validator(&al).unwrap().status,
            ValidatorStatus::Frozen { .. },
        ));
    }

    // -----------------------------------------------------------------------
    // Fee distribution tests (Phase 2.3)
    // -----------------------------------------------------------------------

    #[test]
    fn test_fee_distribution_empty_set_noop() {
        let mut sm = StateMachine::new(vec![]);
        sm.set_active_set(vec![]);
        sm.distribute_fees(1000); // should not panic
    }

    #[test]
    fn test_fee_distribution_zero_fees_noop() {
        let mut sm = StateMachine::new(vec![]);
        let al = alice();
        insert_test_validator(&mut sm, al, U256::from(1000), ValidatorStatus::Active);
        sm.set_active_set(vec![al]);
        sm.distribute_fees(0); // should not panic
        assert!(sm.get_account(&al).is_none());
    }

    #[test]
    fn test_fee_distribution_proportional() {
        let one_monex = crate::core::constants::ONE_MONEX;
        let mut sm = StateMachine::new(vec![]);
        let al = alice();
        let bob = bob();
        // al has 2x stake of bob → gets 2x fees
        insert_test_validator(&mut sm, al, one_monex * 200, ValidatorStatus::Active);
        insert_test_validator(&mut sm, bob, one_monex * 100, ValidatorStatus::Active);
        sm.set_active_set(vec![al, bob]);

        let fees: u128 = 300; // 300 MOXX
        sm.distribute_fees(fees);

        let al_acct = sm.get_account(&al).unwrap();
        let bob_acct = sm.get_account(&bob).unwrap();
        // total_stake = 300 MONEX, al = 200/300*300 = 200, bob = 100/300*300 = 100
        assert_eq!(al_acct.balance, U256::from(200));
        assert_eq!(bob_acct.balance, U256::from(100));
    }

    #[test]
    fn test_fee_distribution_remainder_goes_to_top() {
        let one_monex = crate::core::constants::ONE_MONEX;
        let mut sm = StateMachine::new(vec![]);
        let al = alice();
        let bob = bob();
        // 3:1 ratio, total_stake = 4, fees = 10
        // al = 10*3/4 = 7, bob = 10*1/4 = 2, remainder = 1
        insert_test_validator(&mut sm, al, one_monex * 3, ValidatorStatus::Active);
        insert_test_validator(&mut sm, bob, one_monex * 1, ValidatorStatus::Active);
        sm.set_active_set(vec![al, bob]);

        sm.distribute_fees(10);

        let al_acct = sm.get_account(&al).unwrap();
        let bob_acct = sm.get_account(&bob).unwrap();
        assert_eq!(al_acct.balance, U256::from(8), "7 + 1 remainder");
        assert_eq!(bob_acct.balance, U256::from(2));
    }

    #[test]
    fn test_fee_distribution_integration_via_apply_block() {
        let one_monex = crate::core::constants::ONE_MONEX;
        let mut sm = StateMachine::new(vec![(alice(), Account::new(one_monex * 1000))]);
        let al = alice();
        let bob = bob();
        insert_test_validator(&mut sm, al, one_monex * 100, ValidatorStatus::Active);
        insert_test_validator(&mut sm, bob, one_monex * 100, ValidatorStatus::Active);
        sm.set_active_set(vec![al, bob]);

        // Alice sends 10 MONEX to bob with a 5 MONEX fee
        let tx = Transaction {
            chain_id: 0, nonce: 0, sender: al,
            fee: one_monex * 5, // 5 MONEX
            body: TxBody::Transfer { recipient: bob, amount: one_monex * 10 },
            signature: dummy_sig(),
        };
        let block = Block {
            header: BlockHeader { height: 1, parent_hash: [0u8; 32],
                global_state_root: [0u8; 32], tx_root: [0u8; 32],
                timestamp: 1_700_000_000, proposer: al, chain_id: 0,
                proposer_signature: crate::crypto::falcon::Falcon512Signature::from_bytes(&[0xCD; crate::crypto::constants::FALCON_SIGNATURE_SIZE]).unwrap(),
            },
            body: BlockBody { transactions: vec![tx] },
        };
        let receipt = sm.apply_block(&block).unwrap();
        assert!(receipt.total_fees > 0, "fees collected");

        // Both validators should have been credited
        let al_acct = sm.get_account(&al).unwrap();
        let bob_acct = sm.get_account(&bob).unwrap();
        // alice sent 10 + 5 fee, but gets half back from fee distribution
        // alice's balance start: 1000, send: -15, receive fee share: 2.5 MONEX
        // bob's balance start: none, receive: +10, receive fee share: 2.5 MONEX
        assert!(al_acct.balance < one_monex * 1000, "alice paid tx + fee");
        // Both validators get fee share (accounts created if missing)
        assert!(bob_acct.balance >= one_monex * 10, "bob received transfer");
    }

    #[test]
    fn test_election_era_0_open() {
        let mut sm = StateMachine::new(vec![]);
        // Insert 5 validators in registration order
        for i in 0..5u8 {
            let addr = Address::from([i; 32]);
            let entry = ValidatorEntry {
                address: addr,
                public_key: [0x42u8; 897],
                stake: U256::zero(),
                status: ValidatorStatus::Registered,
                registration_era: u64::from(i),
            };
            let key = namespace_key(NS_VALIDATORS, addr.as_bytes());
            sm.state.insert(&key, entry.encode());
        }

        let addrs: Vec<Address> = (0..5u8).map(|i| Address::from([i; 32])).collect();
        let active = sm.run_election(&addrs, 3, 0);

        assert_eq!(active.len(), 3);
        assert_eq!(active[0], Address::from([0u8; 32]));
        assert_eq!(active[1], Address::from([1u8; 32]));
        assert_eq!(active[2], Address::from([2u8; 32]));

        // Check status changed to Active
        for addr in &active {
            let entry = sm.get_validator(addr).unwrap();
            assert_eq!(entry.status, ValidatorStatus::Active);
        }
        // 4th validator stays Registered
        let entry = sm.get_validator(&Address::from([3u8; 32])).unwrap();
        assert_eq!(entry.status, ValidatorStatus::Registered);
    }

    #[test]
    fn test_election_era_1_top_n() {
        let one_monex = crate::core::constants::ONE_MONEX;
        let mut sm = StateMachine::new(vec![]);
        // Insert 5 validators with different stakes (all >= 1 MONEX)
        let stakes = [
            U256::from(100) * one_monex,
            U256::from(50) * one_monex,
            U256::from(200) * one_monex,
            U256::from(30) * one_monex,
            U256::from(75) * one_monex,
        ];
        for (i, &stake) in stakes.iter().enumerate() {
            let addr = Address::from([i as u8; 32]);
            let entry = ValidatorEntry {
                address: addr,
                public_key: [0x42u8; 897],
                stake,
                status: ValidatorStatus::Staked { stake },
                registration_era: u64::from(i as u8),
            };
            let key = namespace_key(NS_VALIDATORS, addr.as_bytes());
            sm.state.insert(&key, entry.encode());
        }

        let addrs: Vec<Address> = (0..5u8).map(|i| Address::from([i; 32])).collect();
        let active = sm.run_election(&addrs, 3, 1);

        // Top 3 by stake: 200 (idx 2), 100 (idx 0), 75 (idx 4)
        assert_eq!(active.len(), 3);
        assert_eq!(active[0], Address::from([2u8; 32])); // 200 * ONE_MONEX
        assert_eq!(active[1], Address::from([0u8; 32])); // 100 * ONE_MONEX
        assert_eq!(active[2], Address::from([4u8; 32])); // 75 * ONE_MONEX

        // Non-elected validators still have their original status
        let entry = sm.get_validator(&Address::from([1u8; 32])).unwrap();
        assert_eq!(
            entry.status,
            ValidatorStatus::Staked { stake: U256::from(50) * one_monex }
        );
    }

    #[test]
    fn test_election_frozen_excluded() {
        let mut sm = StateMachine::new(vec![]);
        for i in 0..3u8 {
            let status = if i == 1 {
                ValidatorStatus::Frozen { frozen_until: 999 }
            } else {
                ValidatorStatus::Staked { stake: U256::from(100) }
            };
            let addr = Address::from([i; 32]);
            let entry = ValidatorEntry {
                address: addr,
                public_key: [0x42u8; 897],
                stake: U256::from(100),
                status,
                registration_era: u64::from(i),
            };
            let key = namespace_key(NS_VALIDATORS, addr.as_bytes());
            sm.state.insert(&key, entry.encode());
        }

        let addrs: Vec<Address> = (0..3u8).map(|i| Address::from([i; 32])).collect();
        let active = sm.run_election(&addrs, 3, 0);
        // Frozen validator (idx 1) excluded, so only 2 elected
        assert_eq!(active.len(), 2);
        assert_eq!(active[0], Address::from([0u8; 32]));
        assert_eq!(active[1], Address::from([2u8; 32]));
    }

    // -----------------------------------------------------------------------
    // apply_slash tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_apply_slash_1000_monex() {
        let one_monex = crate::core::constants::ONE_MONEX;
        let mut sm = StateMachine::new(vec![]);
        let val_addr = alice();
        // Manually insert a staked validator with 1000 MONEX
        let entry = ValidatorEntry {
            address: val_addr,
            public_key: [0x42u8; 897],
            stake: U256::from(1000) * one_monex,
            status: ValidatorStatus::Active,
            registration_era: 0,
        };
        let key = namespace_key(NS_VALIDATORS, val_addr.as_bytes());
        sm.state.insert(&key, entry.encode());

        let proposer_hash = *val_addr.as_bytes();
        let result = sm.apply_slash(&proposer_hash, &bob(), 0).unwrap();

        // 1000 slashed → 900 slashed, 810 burned, 90 bounty, 100 remaining
        assert_eq!(result.slashed_amount, U256::from(900) * one_monex);
        assert_eq!(result.burn_amount, U256::from(810) * one_monex);
        assert_eq!(result.bounty_amount, U256::from(90) * one_monex);
        assert_eq!(result.remaining_stake, U256::from(100) * one_monex);

        // Validator entry updated
        let entry = sm.get_validator(&val_addr).unwrap();
        assert_eq!(entry.stake, U256::from(100) * one_monex);
        assert_eq!(entry.status, ValidatorStatus::Frozen { frozen_until: 72 });
    }

    #[test]
    #[test]
    fn test_apply_slash_10_monex() {
        let one_monex = crate::core::constants::ONE_MONEX;
        let mut sm = StateMachine::new(vec![]);
        let val_addr = alice();
        let entry = ValidatorEntry {
            address: val_addr,
            public_key: [0x42u8; 897],
            stake: U256::from(10) * one_monex,
            status: ValidatorStatus::Active,
            registration_era: 0,
        };
        let key = namespace_key(NS_VALIDATORS, val_addr.as_bytes());
        sm.state.insert(&key, entry.encode());

        let proposer_hash = *val_addr.as_bytes();
        let result = sm.apply_slash(&proposer_hash, &bob(), 5).unwrap();

        // 10 MONEX → 9 slashed, 8.1 burned, 0.9 bounty, 1 remaining
        assert_eq!(result.slashed_amount, U256::from(9) * one_monex);
        assert_eq!(result.burn_amount, U256::from(9) * one_monex * U256::from(90) / U256::from(100));
        assert_eq!(result.bounty_amount, U256::from(9) * one_monex * U256::from(10) / U256::from(100));
        assert_eq!(result.remaining_stake, one_monex);

        // frozen_until = current_era(5) + 72 = 77
        let entry = sm.get_validator(&val_addr).unwrap();
        assert_eq!(entry.status, ValidatorStatus::Frozen { frozen_until: 77 });
    }

    #[test]
    fn test_apply_slash_already_frozen_rejected() {
        let one_monex = crate::core::constants::ONE_MONEX;
        let mut sm = StateMachine::new(vec![]);
        let val_addr = alice();
        let entry = ValidatorEntry {
            address: val_addr,
            public_key: [0x42u8; 897],
            stake: U256::from(500) * one_monex,
            status: ValidatorStatus::Frozen { frozen_until: 72 },
            registration_era: 0,
        };
        let key = namespace_key(NS_VALIDATORS, val_addr.as_bytes());
        sm.state.insert(&key, entry.encode());

        let proposer_hash = *val_addr.as_bytes();
        let err = sm.apply_slash(&proposer_hash, &bob(), 0).unwrap_err();
        assert_eq!(err, LibError::AlreadyFrozen);
    }

    #[test]
    fn test_apply_slash_nonexistent_validator() {
        let mut sm = StateMachine::new(vec![]);
        let proposer_hash = [0xFFu8; 32];
        let err = sm.apply_slash(&proposer_hash, &bob(), 0).unwrap_err();
        assert!(matches!(err, LibError::ValidatorNotFound(_)));
    }

    #[test]
    fn test_apply_slash_credits_reporter_account() {
        let one_monex = crate::core::constants::ONE_MONEX;
        let mut sm = StateMachine::new(vec![]);
        let val_addr = alice();
        let reporter_addr = bob();
        // Give reporter some initial balance to verify it's credited correctly
        let mut reporter_acct = Account::new(U256::from(100) * one_monex);
        let reporter_key = namespace_key(NS_ACCOUNTS, reporter_addr.as_bytes());
        sm.state.insert(&reporter_key, scale_encode_account(&reporter_acct));

        let entry = ValidatorEntry {
            address: val_addr,
            public_key: [0x42u8; 897],
            stake: U256::from(1000) * one_monex,
            status: ValidatorStatus::Active,
            registration_era: 0,
        };
        let key = namespace_key(NS_VALIDATORS, val_addr.as_bytes());
        sm.state.insert(&key, entry.encode());

        let proposer_hash = *val_addr.as_bytes();
        sm.apply_slash(&proposer_hash, &reporter_addr, 0).unwrap();

        // Reporter should have 100 + 90 = 190 MONEX
        let reporter_post = sm.get_account(&reporter_addr).unwrap();
        assert_eq!(reporter_post.balance, U256::from(190) * one_monex);
    }

    #[test]
    fn test_apply_slash_burns_to_permanent() {
        let one_monex = crate::core::constants::ONE_MONEX;
        let mut sm = StateMachine::new(vec![]);
        let val_addr = alice();
        let entry = ValidatorEntry {
            address: val_addr,
            public_key: [0x42u8; 897],
            stake: U256::from(1000) * one_monex,
            status: ValidatorStatus::Active,
            registration_era: 0,
        };
        let key = namespace_key(NS_VALIDATORS, val_addr.as_bytes());
        sm.state.insert(&key, entry.encode());

        let proposer_hash = *val_addr.as_bytes();
        sm.apply_slash(&proposer_hash, &bob(), 0).unwrap();

        // Burn address should have 810 MONEX
        let burn_addr = crate::core::account::burn_address();
        let burn_acct = sm.get_account(&burn_addr).unwrap();
        assert_eq!(burn_acct.balance, U256::from(810) * one_monex);
    }
}
