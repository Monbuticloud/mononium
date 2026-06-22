//! GovernanceEngine — on-chain governance proposal lifecycle.
//!
//! Manages proposal submission, vote casting, tallying at era boundaries,
//! and execution of approved actions. All state is stored in the
//! `NS_GOVERNANCE` SMT namespace.

use primitive_types::U256;

use crate::core::account::Address;
use crate::core::state::StateMachine;
use crate::crypto::hash::blake3_hash;
use parity_scale_codec::Encode;

use crate::error::{LibError, Result};
use crate::governance::constants::{
    MAX_ACTIVE_PROPOSALS_PER_PROPOSER, MAX_PROPOSALS_PER_ERA, PROPOSAL_DEPOSIT,
    VOTING_WINDOW_ERAS,
};
use crate::governance::types::{
    GovernanceAction, GovernanceParam, Proposal, ProposalStatus, Vote,
};

// ---------------------------------------------------------------------------
// Sub-key helpers
// ---------------------------------------------------------------------------

/// SMT sub-key for a proposal: `prop_{proposal_id_hex}`.
fn prop_key(proposal_id: &[u8; 32]) -> Vec<u8> {
    let mut k = b"prop_".to_vec();
    k.extend_from_slice(&hex::encode(proposal_id).as_bytes());
    k
}

/// SMT sub-key for a vote: `vote_{proposal_id_hex}_{voter_hex}`.
fn vote_key(proposal_id: &[u8; 32], voter: &Address) -> Vec<u8> {
    let mut k = b"vote_".to_vec();
    k.extend_from_slice(&hex::encode(proposal_id).as_bytes());
    k.push(b'_');
    k.extend_from_slice(&hex::encode(voter.as_bytes()).as_bytes());
    k
}

/// SMT sub-key for a governance param value: `gov_param_{name}`.
fn param_key(param: &GovernanceParam) -> Vec<u8> {
    let mut k = b"gov_param_".to_vec();
    k.extend_from_slice(&format!("{param:?}").as_bytes());
    k
}

/// SMT sub-key for active proposal count (per-proposer + global).
const GOV_ACTIVE_COUNT: &[u8] = b"gov_active_count";
const GOV_PROPOSER_COUNT_PREFIX: &[u8] = b"gov_proposer_count_";

// ---------------------------------------------------------------------------
// GovernanceEngine
// ---------------------------------------------------------------------------

/// On-chain governance engine.
///
/// Operates on a [`StateMachine`] — reads/writes proposals, votes, and param
/// values in the `NS_GOVERNANCE` SMT namespace.
pub struct GovernanceEngine<'a> {
    state: &'a mut StateMachine,
}

impl<'a> GovernanceEngine<'a> {
    /// Create a new engine bound to the given state machine.
    pub fn new(state: &'a mut StateMachine) -> Self {
        Self { state }
    }

    // -------------------------------------------------------------------
    // Proposal submission
    // -------------------------------------------------------------------

    /// Submit a new governance proposal.
    ///
    /// Validates the proposer's stake balance, deposit amount, title/desc
    /// size, action parameter bounds, and rate limits. Deducts deposit,
    /// stores the proposal in SMT.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_proposal(
        &mut self,
        proposer: &Address,
        proposal_id: &[u8; 32],
        title: &[u8],
        description: &[u8],
        actions: &[GovernanceAction],
        current_era: u64,
    ) -> Result<()> {
        // -- Validation ------------------------------------------------

        // Proposer must have sufficient balance for deposit
        let Some(acct) = self.state.get_account(proposer) else {
            return Err(LibError::AccountNotFound((*proposer.as_bytes()).into()));
        };
        if acct.balance < PROPOSAL_DEPOSIT {
            return Err(LibError::InsufficientBalance(acct.balance, PROPOSAL_DEPOSIT));
        }

        // Title max 256 bytes
        if title.len() > 256 {
            return Err(LibError::GovernanceRejected("title exceeds 256 bytes"));
        }

        // Description max 4096 bytes
        if description.len() > 4096 {
            return Err(LibError::GovernanceRejected("description exceeds 4096 bytes"));
        }

        // Actions must be non-empty and respect param bounds
        if actions.is_empty() {
            return Err(LibError::GovernanceRejected("proposal must have at least one action"));
        }
        for action in actions {
            self.validate_action(action)?;
        }

        // Rate limits
        let proposer_count = self.proposer_active_count(proposer);
        if proposer_count >= MAX_ACTIVE_PROPOSALS_PER_PROPOSER as u64 {
            return Err(LibError::GovernanceRejected("max 5 active proposals per proposer"));
        }

        let global_count = self.global_active_count();
        if global_count >= MAX_PROPOSALS_PER_ERA as u64 {
            return Err(LibError::GovernanceRejected("max 50 proposals per era reached"));
        }

        // Param lock: no active proposal for same param
        for action in actions {
            if let GovernanceAction::UpdateParam { param, .. } = action {
                if self.has_active_proposal_for_param(param) {
                    return Err(LibError::GovernanceRejected(
                        "an active proposal already targets this parameter",
                    ));
                }
            }
        }

        // -- Apply -----------------------------------------------------

        // Deduct deposit from proposer
        let mut acct = self.state.get_account(proposer).unwrap();
        acct.balance = acct.balance.saturating_sub(PROPOSAL_DEPOSIT);
        acct.nonce += 1;

        // Store proposal
        let proposal = Proposal {
            proposal_id: *proposal_id,
            proposer: *proposer,
            title: title.to_vec(),
            description: description.to_vec(),
            actions: actions.to_vec(),
            deposit: PROPOSAL_DEPOSIT,
            submission_era: current_era,
            status: ProposalStatus::Active,
        };
        let key = prop_key(proposal_id);
        self.state.governance_insert(&key, proposal.encode());

        // Update counters
        self.increment_proposer_count(proposer);
        self.increment_global_count();

        Ok(())
    }

    // -------------------------------------------------------------------
    // Vote casting
    // -------------------------------------------------------------------

    /// Cast (or overwrite) a vote on an active proposal.
    ///
    /// The voter must have non-zero stake. The vote window is
    /// `submission_era ≤ current_era < submission_era + VOTING_WINDOW_ERAS`.
    /// A second vote overwrites the first (allows changing position).
    pub fn cast_vote(
        &mut self,
        proposal_id: &[u8; 32],
        voter: &Address,
        approve: bool,
        current_era: u64,
        block_height: u64,
    ) -> Result<()> {
        // Load proposal
        let Some(proposal) = self.get_proposal(proposal_id) else {
            return Err(LibError::ProposalNotFound);
        };

        // Must be active
        if proposal.status != ProposalStatus::Active {
            return Err(LibError::GovernanceRejected("proposal is not active"));
        }

        // Must be within voting window
        let window_end = proposal.submission_era + VOTING_WINDOW_ERAS;
        if current_era < proposal.submission_era || current_era >= window_end {
            return Err(LibError::GovernanceRejected("voting window is closed"));
        }

        // Voter must have non-zero stake
        let stake = self
            .state
            .get_validator(voter)
            .map(|e| e.stake)
            .unwrap_or(U256::zero());
        if stake.is_zero() {
            return Err(LibError::GovernanceRejected("voter has no stake"));
        }

        // Snapshotted weight = current stake at time of vote
        let vote = Vote {
            proposal_id: *proposal_id,
            voter: *voter,
            approve,
            weight: stake,
            block_height,
        };
        let vk = vote_key(proposal_id, voter);
        self.state.governance_insert(&vk, vote.encode());

        Ok(())
    }

    // -------------------------------------------------------------------
    // Cancellation
    // -------------------------------------------------------------------

    /// Cancel a proposal (before any votes are cast).
    ///
    /// Only the original proposer may cancel. Deposit is returned.
    pub fn cancel_proposal(
        &mut self,
        proposal_id: &[u8; 32],
        caller: &Address,
    ) -> Result<()> {
        let Some(mut proposal) = self.get_proposal(proposal_id) else {
            return Err(LibError::ProposalNotFound);
        };

        // Only proposer
        if &proposal.proposer != caller {
            return Err(LibError::GovernanceRejected("only the proposer may cancel"));
        }

        // Must be active
        if proposal.status != ProposalStatus::Active {
            return Err(LibError::GovernanceRejected("proposal is not active"));
        }

        // No votes yet
        if self.has_any_votes(proposal_id) {
            return Err(LibError::GovernanceRejected("cannot cancel after votes are cast"));
        }

        // Return deposit
        let Some(mut acct) = self.state.get_account(caller) else {
            return Err(LibError::AccountNotFound((*caller.as_bytes()).into()));
        };
        acct.balance = acct.balance.saturating_add(proposal.deposit);
        // Note: nonce not incremented for cancellation

        // Update status
        proposal.status = ProposalStatus::Cancelled;
        let pk = prop_key(proposal_id);
        self.state.governance_insert(&pk, proposal.encode());

        // Decrement counters
        self.decrement_proposer_count(caller);
        self.decrement_global_count();

        Ok(())
    }

    // -------------------------------------------------------------------
    // Tally (era boundary hook)
    // -------------------------------------------------------------------

    /// Tally all proposals whose voting window has closed at `current_era`.
    ///
    /// Returns the list of proposals that were approved (ready for execution
    /// at the next era boundary).
    pub fn tally_proposals(&mut self, current_era: u64) -> Result<Vec<[u8; 32]>> {
        let mut approved = Vec::new();

        // Collect proposal IDs (we can't iterate SMT, so we check known IDs)
        // In practice, the caller (ConsensusEngine) tracks active proposal IDs.
        // For now, this is a no-op stub — full iteration needs SMT list_keys.
        // TODO: integrate with ConsensusEngine for proposal ID tracking.

        let _ = current_era; // placeholder
        Ok(approved)
    }

    // -------------------------------------------------------------------
    // Param execution (next era boundary after approval)
    // -------------------------------------------------------------------

    /// Execute approved proposals by applying their actions to state.
    pub fn execute_approved(&mut self, proposal_ids: &[[u8; 32]]) -> Result<()> {
        let mut sorted: Vec<[u8; 32]> = proposal_ids.to_vec();
        sorted.sort_by(|a, b| a.cmp(b));

        for pid in &sorted {
            let Some(proposal) = self.get_proposal(pid) else {
                continue;
            };
            if proposal.status != ProposalStatus::Approved {
                continue;
            }

            for action in &proposal.actions {
                self.apply_action(action)?;
            }
        }
        Ok(())
    }

    // -------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------

    /// Validate a governance action against param bounds.
    fn validate_action(&self, action: &GovernanceAction) -> Result<()> {
        match action {
            GovernanceAction::UpdateParam { param, new_value } => {
                let bounds = param_bounds_for(param).ok_or_else(|| {
                    LibError::GovernanceRejected("unknown governance parameter")
                })?;
                if *new_value < bounds.0 || *new_value > bounds.1 {
                    return Err(LibError::GovernanceRejected(
                        "new value is out of bounds for this parameter",
                    ));
                }
                Ok(())
            }
            GovernanceAction::IncreaseShards { new_count, .. } => {
                // Shards must increase (not decrease)
                if *new_count == 0 {
                    return Err(LibError::GovernanceRejected(
                        "shard count must be at least 1",
                    ));
                }
                // TODO: check new_count > current shard count (Phase 3)
                Ok(())
            }
        }
    }

    /// Apply a single governance action to state.
    fn apply_action(&mut self, action: &GovernanceAction) -> Result<()> {
        match action {
            GovernanceAction::UpdateParam { param, new_value } => {
                let key = param_key(param);
                let value = new_value.encode();
                self.state.governance_insert(&key, value);
                Ok(())
            }
            GovernanceAction::IncreaseShards { .. } => {
                // Phase 3: sharded state
                Ok(())
            }
        }
    }

    /// Return a proposal from SMT storage.
    fn get_proposal(&self, proposal_id: &[u8; 32]) -> Option<Proposal> {
        let key = prop_key(proposal_id);
        let bytes = self.state.governance_get(&key)?;
        Proposal::decode(&mut &bytes[..]).ok()
    }

    /// Check if a specific param has an active proposal targeting it.
    fn has_active_proposal_for_param(&self, param: &GovernanceParam) -> bool {
        // TODO: iterate active proposals (needs SMT list_keys or tracker)
        let _ = param;
        false
    }

    /// Count active proposals for a proposer.
    fn proposer_active_count(&self, proposer: &Address) -> u64 {
        let key = [GOV_PROPOSER_COUNT_PREFIX, &hex::encode(proposer.as_bytes()).as_bytes()].concat();
        self.state
            .governance_get(&key)
            .and_then(|v| u64::decode(&mut &v[..]).ok())
            .unwrap_or(0)
    }

    /// Global active proposal count.
    fn global_active_count(&self) -> u64 {
        self.state
            .governance_get(GOV_ACTIVE_COUNT)
            .and_then(|v| u64::decode(&mut &v[..]).ok())
            .unwrap_or(0)
    }

    fn increment_proposer_count(&mut self, proposer: &Address) {
        let key = [GOV_PROPOSER_COUNT_PREFIX, &hex::encode(proposer.as_bytes()).as_bytes()].concat();
        let current = self.proposer_active_count(proposer);
        self.state.governance_insert(&key, (current + 1).encode());
    }

    fn decrement_proposer_count(&mut self, proposer: &Address) {
        let key = [GOV_PROPOSER_COUNT_PREFIX, &hex::encode(proposer.as_bytes()).as_bytes()].concat();
        let current = self.proposer_active_count(proposer);
        if current > 0 {
            self.state.governance_insert(&key, (current - 1).encode());
        }
    }

    fn increment_global_count(&mut self) {
        let current = self.global_active_count();
        self.state.governance_insert(GOV_ACTIVE_COUNT, (current + 1).encode());
    }

    fn decrement_global_count(&mut self) {
        let current = self.global_active_count();
        if current > 0 {
            self.state.governance_insert(GOV_ACTIVE_COUNT, (current - 1).encode());
        }
    }

    /// Update a proposal's status in SMT (pub(crate) for tests).
    pub(crate) fn set_proposal_status(&mut self, proposal_id: &[u8; 32], status: ProposalStatus) {
        if let Some(mut p) = self.get_proposal(proposal_id) {
            p.status = status;
            let pk = prop_key(proposal_id);
            self.state.governance_insert(&pk, p.encode());
        }
    }

    /// Check if a proposal has any votes.
    fn has_any_votes(&self, _proposal_id: &[u8; 32]) -> bool {
        // TODO: needs SMT iteration (list_keys for vote_ prefix)
        false
    }
}

// ---------------------------------------------------------------------------
// Helper: decode Vec<u8> via parity-scale-codec
// ---------------------------------------------------------------------------

use parity_scale_codec::Decode;

/// Decode a Proposal from governance SMT bytes.
fn decode_proposal(bytes: &[u8]) -> Option<Proposal> {
    Proposal::decode(&mut &bytes[..]).ok()
}

/// Return the (min, max) bounds for a governance parameter.
fn param_bounds_for(param: &GovernanceParam) -> Option<(U256, U256)> {
    crate::governance::constants::param_bounds().remove(param)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::account::Account;
    use crate::core::constants::ONE_MONEX;
    use crate::core::validator::{ValidatorEntry, ValidatorStatus};
    use crate::crypto::trie::{NS_VALIDATORS, namespace_key};
    use parity_scale_codec::Encode;

    fn addr(b: u8) -> Address {
        Address::from([b; 32])
    }

    fn proposal_id(n: u8) -> [u8; 32] {
        [n; 32]
    }

    fn setup_state() -> StateMachine {
        let mut sm = StateMachine::new(vec![(addr(1), Account::new(PROPOSAL_DEPOSIT * U256::from(5)))]);
        // Add a staked validator for voting
        let val = ValidatorEntry {
            address: addr(2),
            public_key: [0x42u8; 897],
            stake: ONE_MONEX * U256::from(1000),
            status: ValidatorStatus::Active,
            registration_era: 0,
        };
        sm.set_validator(&addr(2), &val);
        sm
    }

    // -- submit_proposal tests ------------------------------------------

    #[test]
    fn test_submit_proposal_success() {
        let mut sm = setup_state();
        let pid = proposal_id(1);
        let proposal = {
            let mut engine = GovernanceEngine::new(&mut sm);
            engine
                .submit_proposal(
                    &addr(1),
                    &pid,
                    b"Test Proposal",
                    b"Description",
                    &[GovernanceAction::UpdateParam {
                        param: GovernanceParam::MaxValidators,
                        new_value: U256::from(50),
                    }],
                    0,
                )
                .unwrap();
            engine.get_proposal(&pid).unwrap()
        };

        assert_eq!(proposal.proposer, addr(1));
        assert_eq!(proposal.status, ProposalStatus::Active);
        assert_eq!(proposal.submission_era, 0);
        assert_eq!(proposal.deposit, PROPOSAL_DEPOSIT);
    }

    #[test]
    fn test_submit_proposal_insufficient_balance() {
        let mut sm = StateMachine::new(vec![(addr(1), Account::new(U256::from(10)))]);
        let err = {
            let mut engine = GovernanceEngine::new(&mut sm);
            engine
                .submit_proposal(
                    &addr(1),
                    &proposal_id(1),
                    b"Title",
                    b"Desc",
                    &[GovernanceAction::UpdateParam {
                        param: GovernanceParam::MaxValidators,
                        new_value: U256::from(50),
                    }],
                    0,
                )
                .unwrap_err()
        };
        assert!(matches!(err, LibError::InsufficientBalance(_, _)));
    }

    #[test]
    fn test_submit_proposal_title_too_long() {
        let mut sm = setup_state();
        let title = [0u8; 257];
        let err = {
            let mut engine = GovernanceEngine::new(&mut sm);
            engine
                .submit_proposal(&addr(1), &proposal_id(1), &title, b"Desc", &[], 0)
                .unwrap_err()
        };
        assert!(matches!(err, LibError::GovernanceRejected(_)));
    }

    #[test]
    fn test_submit_proposal_empty_actions_rejected() {
        let mut sm = setup_state();
        let err = {
            let mut engine = GovernanceEngine::new(&mut sm);
            engine
                .submit_proposal(&addr(1), &proposal_id(1), b"Title", b"Desc", &[], 0)
                .unwrap_err()
        };
        assert!(matches!(err, LibError::GovernanceRejected(_)));
    }

    #[test]
    fn test_submit_proposal_param_out_of_bounds() {
        let mut sm = setup_state();
        let err = {
            let mut engine = GovernanceEngine::new(&mut sm);
            engine
                .submit_proposal(
                    &addr(1),
                    &proposal_id(1),
                    b"Title",
                    b"Desc",
                    &[GovernanceAction::UpdateParam {
                        param: GovernanceParam::MaxValidators,
                        new_value: U256::zero(),
                    }],
                    0,
                )
                .unwrap_err()
        };
        assert!(matches!(err, LibError::GovernanceRejected(_)));
    }

    // -- cast_vote tests ------------------------------------------------

    #[test]
    fn test_cast_vote_success() {
        let mut sm = setup_state();
        {
            let mut engine = GovernanceEngine::new(&mut sm);
            engine
                .submit_proposal(
                    &addr(1),
                    &proposal_id(1),
                    b"Test",
                    b"Desc",
                    &[GovernanceAction::UpdateParam {
                        param: GovernanceParam::MaxValidators,
                        new_value: U256::from(50),
                    }],
                    0,
                )
                .unwrap();
            engine
                .cast_vote(&proposal_id(1), &addr(2), true, 0, 100)
                .unwrap();
        }
        // sm no longer borrowed — can access directly
        let vk = vote_key(&proposal_id(1), &addr(2));
        let bytes = sm.governance_get(&vk).unwrap();
        let vote: Vote = Vote::decode(&mut &bytes[..]).unwrap();
        assert!(vote.approve);
        assert_eq!(vote.voter, addr(2));
        assert_eq!(vote.weight, ONE_MONEX * U256::from(1000));
    }

    #[test]
    fn test_cast_vote_no_stake_rejected() {
        let mut sm = setup_state();
        let err = {
            let mut engine = GovernanceEngine::new(&mut sm);
            engine
                .submit_proposal(
                    &addr(1),
                    &proposal_id(1),
                    b"Test",
                    b"Desc",
                    &[GovernanceAction::UpdateParam {
                        param: GovernanceParam::MaxValidators,
                        new_value: U256::from(50),
                    }],
                    0,
                )
                .unwrap();
            engine
                .cast_vote(&proposal_id(1), &addr(3), true, 0, 100)
                .unwrap_err()
        };
        assert!(matches!(err, LibError::GovernanceRejected(_)));
    }

    // -- cancel_proposal tests ------------------------------------------

    #[test]
    fn test_cancel_proposal_success() {
        let mut sm = setup_state();
        let pid = proposal_id(1);
        let proposal = {
            let mut engine = GovernanceEngine::new(&mut sm);
            engine
                .submit_proposal(
                    &addr(1),
                    &pid,
                    b"Test",
                    b"Desc",
                    &[GovernanceAction::UpdateParam {
                        param: GovernanceParam::MaxValidators,
                        new_value: U256::from(50),
                    }],
                    0,
                )
                .unwrap();
            engine.cancel_proposal(&pid, &addr(1)).unwrap();
            engine.get_proposal(&pid).unwrap()
        };
        assert_eq!(proposal.status, ProposalStatus::Cancelled);
    }

    #[test]
    fn test_cancel_proposal_not_proposer() {
        let mut sm = setup_state();
        let err = {
            let mut engine = GovernanceEngine::new(&mut sm);
            engine
                .submit_proposal(
                    &addr(1),
                    &proposal_id(1),
                    b"Test",
                    b"Desc",
                    &[GovernanceAction::UpdateParam {
                        param: GovernanceParam::MaxValidators,
                        new_value: U256::from(50),
                    }],
                    0,
                )
                .unwrap();
            engine
                .cancel_proposal(&proposal_id(1), &addr(3))
                .unwrap_err()
        };
        assert!(matches!(err, LibError::GovernanceRejected(_)));
    }

    // -- execute_approved tests ------------------------------------------

    #[test]
    fn test_execute_update_param() {
        let mut sm = setup_state();
        let pid = proposal_id(1);
        {
            let mut engine = GovernanceEngine::new(&mut sm);
            engine
                .submit_proposal(
                    &addr(1),
                    &pid,
                    b"Test",
                    b"Desc",
                    &[GovernanceAction::UpdateParam {
                        param: GovernanceParam::MaxValidators,
                        new_value: U256::from(100),
                    }],
                    0,
                )
                .unwrap();

            engine.set_proposal_status(&pid, ProposalStatus::Approved);
            engine.execute_approved(&[pid]).unwrap();
        }

        // sm no longer borrowed
        let pk = param_key(&GovernanceParam::MaxValidators);
        let bytes = sm.governance_get(&pk).unwrap();
        let stored: U256 = U256::decode(&mut &bytes[..]).unwrap();
        assert_eq!(stored, U256::from(100));
    }

    #[test]
    fn test_execute_multiple_params_last_wins() {
        let mut sm = setup_state();
        {
            let mut engine = GovernanceEngine::new(&mut sm);

            for (i, val) in [(1u8, U256::from(50)), (2, U256::from(200))] {
                let pid = proposal_id(i);
                engine
                    .submit_proposal(
                        &addr(1),
                        &pid,
                        b"Test",
                        b"Desc",
                        &[GovernanceAction::UpdateParam {
                            param: GovernanceParam::MaxValidators,
                            new_value: val,
                        }],
                        i as u64,
                    )
                    .unwrap();

                engine.set_proposal_status(&pid, ProposalStatus::Approved);
            }

            engine.execute_approved(&[proposal_id(1), proposal_id(2)]).unwrap();
        }

        let pk = param_key(&GovernanceParam::MaxValidators);
        let bytes = sm.governance_get(&pk).unwrap();
        let stored: U256 = U256::decode(&mut &bytes[..]).unwrap();
        assert_eq!(stored, U256::from(200));
    }
}
