//! Dynamic validator set management.
//!
//! Supports adding and removing validators through governance proposals,
//! with vote tracking, epoch-based transitions, and smooth rotation.

use alloy_primitives::Address;
use std::collections::HashMap;
use tracing::{info, warn};

/// Unique proposal identifier.
pub type ProposalId = u64;

/// Type of validator change proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalType {
    /// Add a new validator.
    Add,
    /// Remove an existing validator.
    Remove,
}

/// State of a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalState {
    /// Voting is in progress.
    Pending,
    /// Proposal was approved.
    Approved,
    /// Proposal was rejected.
    Rejected,
    /// Proposal was executed (validator set changed).
    Executed,
    /// Proposal expired without reaching quorum.
    Expired,
}

/// A proposal to change the validator set.
#[derive(Debug, Clone)]
pub struct ValidatorProposal {
    /// Proposal ID.
    pub id: ProposalId,
    /// Type of change.
    pub proposal_type: ProposalType,
    /// The validator address to add or remove.
    pub target: Address,
    /// Who proposed the change.
    pub proposer: Address,
    /// Votes in favor.
    pub votes_for: Vec<Address>,
    /// Votes against.
    pub votes_against: Vec<Address>,
    /// Block number when proposed.
    pub proposed_at: u64,
    /// Block number when the proposal expires.
    pub expires_at: u64,
    /// Current state.
    pub state: ProposalState,
}

/// Result of a validator management operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatorResult {
    /// Proposal created.
    ProposalCreated(ProposalId),
    /// Vote recorded.
    VoteRecorded { proposal_id: ProposalId, votes_for: usize, votes_against: usize },
    /// Proposal approved and ready for execution.
    Approved(ProposalId),
    /// Proposal rejected.
    Rejected(ProposalId),
    /// Validator added to the set.
    ValidatorAdded(Address),
    /// Validator removed from the set.
    ValidatorRemoved(Address),
    /// Proposal expired.
    Expired(ProposalId),
    /// Error occurred.
    Error(ValidatorError),
}

/// Validator management errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatorError {
    /// Proposer is not a current validator.
    NotValidator,
    /// Target is already a validator (for Add proposals).
    AlreadyValidator,
    /// Target is not a validator (for Remove proposals).
    NotInSet,
    /// Cannot remove the last validator.
    CannotRemoveLast,
    /// Voter already voted on this proposal.
    AlreadyVoted,
    /// Proposal not found.
    ProposalNotFound,
    /// Proposal is not in pending state.
    ProposalNotPending,
    /// Voter is not a validator.
    VoterNotValidator,
}

/// Configuration for validator management.
#[derive(Debug, Clone)]
pub struct ValidatorManagerConfig {
    /// Number of blocks a proposal is valid for.
    pub proposal_ttl_blocks: u64,
    /// Fraction of validators required for quorum (numerator).
    pub quorum_numerator: u64,
    /// Fraction of validators required for quorum (denominator).
    pub quorum_denominator: u64,
    /// Minimum number of validators allowed.
    pub min_validators: usize,
    /// Maximum number of validators allowed.
    pub max_validators: usize,
}

impl Default for ValidatorManagerConfig {
    fn default() -> Self {
        Self {
            proposal_ttl_blocks: 1000,
            quorum_numerator: 2,
            quorum_denominator: 3,
            min_validators: 1,
            max_validators: 21,
        }
    }
}

/// Manages the validator set with governance-based rotation.
pub struct ValidatorManager {
    /// Current validator set.
    validators: Vec<Address>,
    /// Active and historical proposals.
    proposals: HashMap<ProposalId, ValidatorProposal>,
    /// Next proposal ID.
    next_proposal_id: ProposalId,
    /// Configuration.
    config: ValidatorManagerConfig,
    /// History of validator set changes: (block_number, added/removed, address).
    history: Vec<(u64, ProposalType, Address)>,
}

impl ValidatorManager {
    /// Create a new validator manager with an initial validator set.
    pub fn new(initial_validators: Vec<Address>, config: ValidatorManagerConfig) -> Self {
        Self {
            validators: initial_validators,
            proposals: HashMap::new(),
            next_proposal_id: 1,
            config,
            history: Vec::new(),
        }
    }

    /// Get the current validator set.
    pub fn validators(&self) -> &[Address] {
        &self.validators
    }

    /// Get the number of validators.
    pub fn validator_count(&self) -> usize {
        self.validators.len()
    }

    /// Check if an address is a current validator.
    pub fn is_validator(&self, address: &Address) -> bool {
        self.validators.contains(address)
    }

    /// Get a proposal by ID.
    pub fn get_proposal(&self, id: ProposalId) -> Option<&ValidatorProposal> {
        self.proposals.get(&id)
    }

    /// Get all pending proposals.
    pub fn pending_proposals(&self) -> Vec<&ValidatorProposal> {
        self.proposals
            .values()
            .filter(|p| p.state == ProposalState::Pending)
            .collect()
    }

    /// Get the validator change history.
    pub fn history(&self) -> &[(u64, ProposalType, Address)] {
        &self.history
    }

    /// Calculate the required number of votes for quorum.
    pub fn quorum_threshold(&self) -> usize {
        let total = self.validators.len() as u64;
        let required = (total * self.config.quorum_numerator)
            .div_ceil(self.config.quorum_denominator);
        required as usize
    }

    /// Create a proposal to add or remove a validator.
    pub fn propose(
        &mut self,
        proposer: Address,
        proposal_type: ProposalType,
        target: Address,
        current_block: u64,
    ) -> ValidatorResult {
        // Check proposer is a validator.
        if !self.is_validator(&proposer) {
            return ValidatorResult::Error(ValidatorError::NotValidator);
        }

        // Validate the proposal.
        match proposal_type {
            ProposalType::Add => {
                if self.is_validator(&target) {
                    return ValidatorResult::Error(ValidatorError::AlreadyValidator);
                }
                if self.validators.len() >= self.config.max_validators {
                    return ValidatorResult::Error(ValidatorError::CannotRemoveLast);
                }
            }
            ProposalType::Remove => {
                if !self.is_validator(&target) {
                    return ValidatorResult::Error(ValidatorError::NotInSet);
                }
                if self.validators.len() <= self.config.min_validators {
                    return ValidatorResult::Error(ValidatorError::CannotRemoveLast);
                }
            }
        }

        let id = self.next_proposal_id;
        self.next_proposal_id += 1;

        let proposal = ValidatorProposal {
            id,
            proposal_type,
            target,
            proposer,
            votes_for: vec![proposer], // proposer auto-votes in favor
            votes_against: Vec::new(),
            proposed_at: current_block,
            expires_at: current_block + self.config.proposal_ttl_blocks,
            state: ProposalState::Pending,
        };

        info!(
            "Validator proposal #{}: {:?} {:?} by {:?}",
            id, proposal_type, target, proposer
        );

        self.proposals.insert(id, proposal);

        // Check if single-validator quorum is already met.
        if self.quorum_threshold() <= 1 {
            return self.check_and_execute(id, current_block);
        }

        ValidatorResult::ProposalCreated(id)
    }

    /// Vote on a proposal.
    pub fn vote(
        &mut self,
        proposal_id: ProposalId,
        voter: Address,
        approve: bool,
        current_block: u64,
    ) -> ValidatorResult {
        // Check voter is a validator.
        if !self.is_validator(&voter) {
            return ValidatorResult::Error(ValidatorError::VoterNotValidator);
        }

        let proposal = match self.proposals.get_mut(&proposal_id) {
            Some(p) => p,
            None => return ValidatorResult::Error(ValidatorError::ProposalNotFound),
        };

        if proposal.state != ProposalState::Pending {
            return ValidatorResult::Error(ValidatorError::ProposalNotPending);
        }

        // Check for duplicate vote.
        if proposal.votes_for.contains(&voter) || proposal.votes_against.contains(&voter) {
            return ValidatorResult::Error(ValidatorError::AlreadyVoted);
        }

        if approve {
            proposal.votes_for.push(voter);
        } else {
            proposal.votes_against.push(voter);
        }

        let votes_for = proposal.votes_for.len();
        let votes_against = proposal.votes_against.len();

        info!(
            "Vote on proposal #{}: voter={:?}, approve={}, for={}, against={}",
            proposal_id, voter, approve, votes_for, votes_against
        );

        // Check if quorum is reached.
        self.check_and_execute(proposal_id, current_block)
    }

    /// Check if a proposal has reached quorum and execute if so.
    fn check_and_execute(&mut self, proposal_id: ProposalId, current_block: u64) -> ValidatorResult {
        let quorum = self.quorum_threshold();
        let proposal = self.proposals.get(&proposal_id).unwrap();
        let votes_for = proposal.votes_for.len();
        let votes_against = proposal.votes_against.len();

        if votes_for >= quorum {
            // Approved - execute the change.
            let proposal = self.proposals.get_mut(&proposal_id).unwrap();
            proposal.state = ProposalState::Approved;
            let target = proposal.target;
            let proposal_type = proposal.proposal_type;

            match proposal_type {
                ProposalType::Add => {
                    self.validators.push(target);
                    self.history.push((current_block, ProposalType::Add, target));
                    info!("Validator added: {:?}", target);
                    let proposal = self.proposals.get_mut(&proposal_id).unwrap();
                    proposal.state = ProposalState::Executed;
                    ValidatorResult::ValidatorAdded(target)
                }
                ProposalType::Remove => {
                    self.validators.retain(|v| *v != target);
                    self.history
                        .push((current_block, ProposalType::Remove, target));
                    info!("Validator removed: {:?}", target);
                    let proposal = self.proposals.get_mut(&proposal_id).unwrap();
                    proposal.state = ProposalState::Executed;
                    ValidatorResult::ValidatorRemoved(target)
                }
            }
        } else if votes_against >= quorum {
            let proposal = self.proposals.get_mut(&proposal_id).unwrap();
            proposal.state = ProposalState::Rejected;
            ValidatorResult::Rejected(proposal_id)
        } else {
            ValidatorResult::VoteRecorded {
                proposal_id,
                votes_for,
                votes_against,
            }
        }
    }

    /// Expire old proposals that have passed their TTL.
    pub fn expire_proposals(&mut self, current_block: u64) -> Vec<ValidatorResult> {
        let mut results = Vec::new();
        let expired_ids: Vec<ProposalId> = self
            .proposals
            .values()
            .filter(|p| p.state == ProposalState::Pending && current_block >= p.expires_at)
            .map(|p| p.id)
            .collect();

        for id in expired_ids {
            if let Some(proposal) = self.proposals.get_mut(&id) {
                proposal.state = ProposalState::Expired;
                warn!("Proposal #{} expired", id);
                results.push(ValidatorResult::Expired(id));
            }
        }

        results
    }

    /// Get the proposer for a given block height (round-robin).
    pub fn proposer_for_height(&self, height: u64) -> Option<Address> {
        if self.validators.is_empty() {
            return None;
        }
        let index = (height as usize) % self.validators.len();
        Some(self.validators[index])
    }
}

impl Default for ValidatorManager {
    fn default() -> Self {
        Self::new(Vec::new(), ValidatorManagerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_address(n: u8) -> Address {
        let mut bytes = [0u8; 20];
        bytes[19] = n;
        Address::from(bytes)
    }

    fn setup_3_validators() -> (ValidatorManager, Address, Address, Address) {
        let v1 = make_address(1);
        let v2 = make_address(2);
        let v3 = make_address(3);
        let vm = ValidatorManager::new(
            vec![v1, v2, v3],
            ValidatorManagerConfig::default(),
        );
        (vm, v1, v2, v3)
    }

    #[test]
    fn test_new_validator_manager() {
        let (vm, v1, v2, v3) = setup_3_validators();
        assert_eq!(vm.validator_count(), 3);
        assert!(vm.is_validator(&v1));
        assert!(vm.is_validator(&v2));
        assert!(vm.is_validator(&v3));
    }

    #[test]
    fn test_quorum_threshold() {
        let (vm, _, _, _) = setup_3_validators();
        // 2/3 of 3 = 2
        assert_eq!(vm.quorum_threshold(), 2);
    }

    #[test]
    fn test_propose_add_validator() {
        let (mut vm, v1, _, _) = setup_3_validators();
        let new_validator = make_address(4);
        let result = vm.propose(v1, ProposalType::Add, new_validator, 1);
        assert!(matches!(result, ValidatorResult::ProposalCreated(1)));
        assert_eq!(vm.pending_proposals().len(), 1);
    }

    #[test]
    fn test_propose_not_validator() {
        let (mut vm, _, _, _) = setup_3_validators();
        let outsider = make_address(99);
        let result = vm.propose(outsider, ProposalType::Add, make_address(4), 1);
        assert_eq!(result, ValidatorResult::Error(ValidatorError::NotValidator));
    }

    #[test]
    fn test_propose_add_already_validator() {
        let (mut vm, v1, v2, _) = setup_3_validators();
        let result = vm.propose(v1, ProposalType::Add, v2, 1);
        assert_eq!(
            result,
            ValidatorResult::Error(ValidatorError::AlreadyValidator)
        );
    }

    #[test]
    fn test_propose_remove_not_in_set() {
        let (mut vm, v1, _, _) = setup_3_validators();
        let outsider = make_address(99);
        let result = vm.propose(v1, ProposalType::Remove, outsider, 1);
        assert_eq!(result, ValidatorResult::Error(ValidatorError::NotInSet));
    }

    #[test]
    fn test_vote_and_approve() {
        let (mut vm, v1, v2, _) = setup_3_validators();
        let new_v = make_address(4);
        vm.propose(v1, ProposalType::Add, new_v, 1);

        let result = vm.vote(1, v2, true, 2);
        assert_eq!(result, ValidatorResult::ValidatorAdded(new_v));
        assert_eq!(vm.validator_count(), 4);
        assert!(vm.is_validator(&new_v));
    }

    #[test]
    fn test_vote_and_reject() {
        let (mut vm, v1, v2, v3) = setup_3_validators();
        let new_v = make_address(4);
        vm.propose(v1, ProposalType::Add, new_v, 1);

        vm.vote(1, v2, false, 2);
        let result = vm.vote(1, v3, false, 3);
        // With 1 for and 2 against, rejected since quorum of against is met.
        assert_eq!(result, ValidatorResult::Rejected(1));
        assert_eq!(vm.validator_count(), 3);
    }

    #[test]
    fn test_vote_already_voted() {
        let (mut vm, v1, _, _) = setup_3_validators();
        vm.propose(v1, ProposalType::Add, make_address(4), 1);
        // v1 auto-voted in propose, so voting again should fail.
        let result = vm.vote(1, v1, true, 2);
        assert_eq!(result, ValidatorResult::Error(ValidatorError::AlreadyVoted));
    }

    #[test]
    fn test_remove_validator() {
        let (mut vm, v1, v2, v3) = setup_3_validators();
        vm.propose(v1, ProposalType::Remove, v3, 1);
        let result = vm.vote(1, v2, true, 2);
        assert_eq!(result, ValidatorResult::ValidatorRemoved(v3));
        assert_eq!(vm.validator_count(), 2);
        assert!(!vm.is_validator(&v3));
    }

    #[test]
    fn test_cannot_remove_last_validator() {
        let v1 = make_address(1);
        let mut vm = ValidatorManager::new(vec![v1], ValidatorManagerConfig::default());
        let result = vm.propose(v1, ProposalType::Remove, v1, 1);
        assert_eq!(
            result,
            ValidatorResult::Error(ValidatorError::CannotRemoveLast)
        );
    }

    #[test]
    fn test_expire_proposals() {
        let (mut vm, v1, _, _) = setup_3_validators();
        vm.propose(v1, ProposalType::Add, make_address(4), 1);

        // Expire at block 1001 (TTL is 1000).
        let results = vm.expire_proposals(1002);
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], ValidatorResult::Expired(1)));
        assert!(vm.pending_proposals().is_empty());
    }

    #[test]
    fn test_proposer_for_height() {
        let (vm, v1, v2, v3) = setup_3_validators();
        assert_eq!(vm.proposer_for_height(0), Some(v1));
        assert_eq!(vm.proposer_for_height(1), Some(v2));
        assert_eq!(vm.proposer_for_height(2), Some(v3));
        assert_eq!(vm.proposer_for_height(3), Some(v1));
    }

    #[test]
    fn test_history_tracking() {
        let (mut vm, v1, v2, _) = setup_3_validators();
        let new_v = make_address(4);
        vm.propose(v1, ProposalType::Add, new_v, 1);
        vm.vote(1, v2, true, 2);

        let history = vm.history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], (2, ProposalType::Add, new_v));
    }

    #[test]
    fn test_single_validator_instant_quorum() {
        let v1 = make_address(1);
        let mut vm = ValidatorManager::new(vec![v1], ValidatorManagerConfig::default());
        let new_v = make_address(2);
        // Single validator = quorum of 1, proposal auto-executes.
        let result = vm.propose(v1, ProposalType::Add, new_v, 1);
        assert_eq!(result, ValidatorResult::ValidatorAdded(new_v));
        assert_eq!(vm.validator_count(), 2);
    }

    #[test]
    fn test_vote_on_nonexistent_proposal() {
        let (mut vm, v1, _, _) = setup_3_validators();
        let result = vm.vote(999, v1, true, 1);
        assert_eq!(
            result,
            ValidatorResult::Error(ValidatorError::ProposalNotFound)
        );
    }

    #[test]
    fn test_voter_not_validator() {
        let (mut vm, v1, _, _) = setup_3_validators();
        vm.propose(v1, ProposalType::Add, make_address(4), 1);
        let outsider = make_address(99);
        let result = vm.vote(1, outsider, true, 2);
        assert_eq!(
            result,
            ValidatorResult::Error(ValidatorError::VoterNotValidator)
        );
    }

    #[test]
    fn test_default_validator_manager() {
        let vm = ValidatorManager::default();
        assert_eq!(vm.validator_count(), 0);
    }
}
