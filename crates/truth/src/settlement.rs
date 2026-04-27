use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::state::StateError;

/// Economic Settlement Layer - NOT a blockchain
/// This layer handles computational job economics and dispute resolution,
/// not general purpose accounting or smart contracts.

/// JobEscrow - Holds budget, releases on receipt verification
/// This is the core economic primitive for job execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEscrow {
    pub job_id: [u8; 32],
    pub client_id: [u8; 32],
    pub amount: u64,
    pub reserved_for_verification: u64,
    pub status: EscrowStatus,
    pub created_at_block: u64,
    pub deadline_block: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EscrowStatus {
    Active,
    Frozen,   // During dispute
    Released, // Paid to operator
    Refunded, // Returned to client
    PartiallyReleased,
}

impl JobEscrow {
    pub fn new(
        job_id: [u8; 32],
        client_id: [u8; 32],
        amount: u64,
        verifier_reserve_percent: u64,
        current_block: u64,
        deadline_blocks: u64,
    ) -> Self {
        // Use saturating_mul to prevent overflow on arithmetic
        let verifier_reserve = amount.saturating_mul(verifier_reserve_percent) / 10000;
        Self {
            job_id,
            client_id,
            amount,
            reserved_for_verification: verifier_reserve,
            status: EscrowStatus::Active,
            created_at_block: current_block,
            deadline_block: current_block.saturating_add(deadline_blocks),
        }
    }

    pub fn releaseable_amount(&self) -> u64 {
        self.amount.saturating_sub(self.reserved_for_verification)
    }

    pub fn release(&mut self, _to: [u8; 32]) -> u64 {
        if self.status != EscrowStatus::Active {
            return 0;
        }
        self.status = EscrowStatus::Released;
        self.amount
    }

    pub fn refund(&mut self) -> u64 {
        if self.status != EscrowStatus::Active {
            return 0;
        }
        self.status = EscrowStatus::Refunded;
        self.amount
    }

    pub fn freeze(&mut self) {
        if self.status == EscrowStatus::Active {
            self.status = EscrowStatus::Frozen;
        }
    }

    pub fn unfreeze(&mut self) {
        if self.status == EscrowStatus::Frozen {
            self.status = EscrowStatus::Active;
        }
    }

    pub fn is_expired(&self, current_block: u64) -> bool {
        current_block > self.deadline_block && self.status == EscrowStatus::Active
    }
}

/// OperatorRegistry - Stake/reputation tracking for compute providers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorRegistry {
    pub operators: HashMap<[u8; 32], OperatorState>,
    pub stake_index: HashMap<u64, Vec<[u8; 32]>>, // stake -> operator ids
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorState {
    pub id: [u8; 32],
    pub pubkey: [u8; 32],
    pub stake: u64,
    pub reputation: u64,
    pub status: OperatorRegistrationStatus,
    pub region: String,
    pub capabilities: Vec<[u8; 32]>,
    pub slashed_count: u32,
    pub total_jobs_completed: u64,
    pub total_compute_units: u64,
    pub last_update_block: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatorRegistrationStatus {
    Registered,   // Deposit made, not yet active
    Active,       // Approved to take jobs
    Suspended,    // Temporarily barred
    Slashed,      // Penalized for fraud
    Deregistered, // Voluntarily or permanently removed
}

impl OperatorRegistry {
    pub fn new() -> Self {
        Self {
            operators: HashMap::new(),
            stake_index: HashMap::new(),
        }
    }

    pub fn register(&mut self, operator: OperatorState) {
        let stake_bucket = operator.stake / 1000;
        self.stake_index
            .entry(stake_bucket)
            .or_insert_with(Vec::new)
            .push(operator.id);
        self.operators.insert(operator.id, operator);
    }

    pub fn get(&self, id: &[u8; 32]) -> Option<&OperatorState> {
        self.operators.get(id)
    }

    pub fn get_mut(&mut self, id: &[u8; 32]) -> Option<&mut OperatorState> {
        self.operators.get_mut(id)
    }

    pub fn is_active(&self, id: &[u8; 32]) -> bool {
        self.operators
            .get(id)
            .map(|o| o.status == OperatorRegistrationStatus::Active)
            .unwrap_or(false)
    }

    pub fn update_stake(&mut self, id: &[u8; 32], new_stake: u64) -> Result<(), StateError> {
        let operator = self
            .operators
            .get_mut(id)
            .ok_or(StateError::OperatorNotFound)?;

        // Remove from old stake bucket
        let old_bucket = operator.stake / 1000;
        if let Some(ids) = self.stake_index.get_mut(&old_bucket) {
            ids.retain(|i| *i != operator.id);
        }

        // Update stake
        operator.stake = new_stake;

        // Add to new stake bucket
        let new_bucket = new_stake / 1000;
        self.stake_index
            .entry(new_bucket)
            .or_insert_with(Vec::new)
            .push(operator.id);

        Ok(())
    }

    pub fn deactivate(&mut self, id: &[u8; 32]) {
        if let Some(operator) = self.operators.get_mut(id) {
            operator.status = OperatorRegistrationStatus::Deregistered;
        }
    }

    pub fn slash(&mut self, id: &[u8; 32], amount: u64) -> Result<u64, StateError> {
        let operator = self
            .operators
            .get_mut(id)
            .ok_or(StateError::OperatorNotFound)?;

        if operator.stake < amount {
            return Err(StateError::InsufficientStake);
        }

        operator.stake = operator.stake.saturating_sub(amount);
        operator.slashed_count += 1;
        operator.status = OperatorRegistrationStatus::Slashed;

        // Update stake index
        let old_bucket = operator.stake / 1000;
        if let Some(ids) = self.stake_index.get_mut(&old_bucket) {
            ids.retain(|i| *i != operator.id);
        }

        Ok(amount)
    }

    pub fn increase_reputation(&mut self, id: &[u8; 32], delta: u64) {
        if let Some(operator) = self.operators.get_mut(id) {
            operator.reputation = (operator.reputation + delta).min(10000);
            if operator.status == OperatorRegistrationStatus::Suspended {
                if operator.reputation >= 1000 {
                    operator.status = OperatorRegistrationStatus::Active;
                }
            }
        }
    }

    pub fn decrease_reputation(&mut self, id: &[u8; 32], delta: u64) {
        if let Some(operator) = self.operators.get_mut(id) {
            operator.reputation = operator.reputation.saturating_sub(delta);
            if operator.status == OperatorRegistrationStatus::Active && operator.reputation < 500 {
                operator.status = OperatorRegistrationStatus::Suspended;
            }
        }
    }

    pub fn get_active_operators(&self) -> Vec<&OperatorState> {
        self.operators
            .values()
            .filter(|o| o.status == OperatorRegistrationStatus::Active)
            .collect()
    }
}

impl Default for OperatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// ReceiptAnchor - Hash of computation output in state
/// Used to anchor verification results and enable fraud proofs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptAnchor {
    pub receipt_id: [u8; 32],
    pub job_id: [u8; 32],
    pub operator_id: [u8; 32],
    pub output_hash: [u8; 32],
    pub input_hash: [u8; 32],
    pub compute_used: u64,
    pub status: ReceiptStatus,
    pub created_at_block: u64,
    pub settled_at_block: Option<u64>,
    pub challenger_id: Option<[u8; 32]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceiptStatus {
    Submitted,
    PendingVerification,
    Verified,
    Challenged,
    Disputed,
    Settled,
    Rejected,
}

impl ReceiptAnchor {
    pub fn new(
        job_id: [u8; 32],
        operator_id: [u8; 32],
        output_hash: [u8; 32],
        input_hash: [u8; 32],
        compute_used: u64,
        current_block: u64,
    ) -> Self {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&job_id);
        hasher.update(&operator_id);
        hasher.update(&output_hash);
        hasher.update(&current_block.to_le_bytes());

        Self {
            receipt_id: *hasher.finalize().as_bytes(),
            job_id,
            operator_id,
            output_hash,
            input_hash,
            compute_used,
            status: ReceiptStatus::Submitted,
            created_at_block: current_block,
            settled_at_block: None,
            challenger_id: None,
        }
    }

    pub fn verify(&mut self, _verifier_id: [u8; 32], _current_block: u64) {
        self.status = ReceiptStatus::Verified;
    }

    pub fn challenge(&mut self, challenger_id: [u8; 32]) {
        self.status = ReceiptStatus::Challenged;
        self.challenger_id = Some(challenger_id);
    }

    pub fn escalate(&mut self) {
        self.status = ReceiptStatus::Disputed;
    }

    pub fn settle(&mut self, current_block: u64) {
        self.status = ReceiptStatus::Settled;
        self.settled_at_block = Some(current_block);
    }

    pub fn is_within_challenge_window(&self, current_block: u64, window: u64) -> bool {
        current_block <= self.created_at_block + window
    }
}

/// ArtifactCommitment - Content-addressed artifacts
/// Enables IPFS-like content addressing for computation outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactCommitment {
    pub artifact_id: [u8; 32],
    pub content_hash: [u8; 32],
    pub size_bytes: u64,
    pub creator: [u8; 32],
    pub commitment_block: u64,
    pub sealed: bool,
}

impl ArtifactCommitment {
    pub fn new(content: &[u8], creator: [u8; 32], current_block: u64) -> Self {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(content);
        let content_hash = *hasher.finalize().as_bytes();

        let mut hasher2 = Hasher::new();
        hasher2.update(&content_hash);
        hasher2.update(&current_block.to_le_bytes());

        Self {
            artifact_id: *hasher2.finalize().as_bytes(),
            content_hash,
            size_bytes: content.len() as u64,
            creator,
            commitment_block: current_block,
            sealed: false,
        }
    }

    pub fn seal(&mut self) {
        self.sealed = true;
    }

    pub fn verify(&self, content: &[u8]) -> bool {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(content);
        let binding = hasher.finalize();
        let computed = binding.as_bytes();
        *computed == self.content_hash
    }
}

/// SlashingConditions - Fraud not consensus failures
/// Defines when operators can be slashed for fraudulent behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashingConditions {
    pub min_stake: u64,
    pub slash_percentage: u64,
    pub challenge_window_blocks: u64,
    pub double_signing_window_blocks: u64,
    pub max_challenges_before_slash: u32,
}

impl Default for SlashingConditions {
    fn default() -> Self {
        Self {
            min_stake: 1000,
            slash_percentage: 5000, // 50% of stake
            challenge_window_blocks: 100,
            double_signing_window_blocks: 10,
            max_challenges_before_slash: 3,
        }
    }
}

impl SlashingConditions {
    pub fn calculate_slash_amount(&self, stake: u64) -> u64 {
        (stake * self.slash_percentage) / 10000
    }

    pub fn should_slash(&self, challenge_count: u32) -> bool {
        challenge_count >= self.max_challenges_before_slash
    }
}

/// State Extensions for Settlement
/// Replaces the account model with economic settlement primitives
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementState {
    pub job_escrows: HashMap<[u8; 32], JobEscrow>,
    pub operator_registry: OperatorRegistry,
    pub receipt_anchors: HashMap<[u8; 32], ReceiptAnchor>,
    pub artifacts: HashMap<[u8; 32], ArtifactCommitment>,
    pub slashing_conditions: SlashingConditions,
    pub slashing_pool: u64,
    pub verifier_rewards: HashMap<[u8; 32], u64>,
}

impl SettlementState {
    pub fn new() -> Self {
        Self {
            job_escrows: HashMap::new(),
            operator_registry: OperatorRegistry::new(),
            receipt_anchors: HashMap::new(),
            artifacts: HashMap::new(),
            slashing_conditions: SlashingConditions::default(),
            slashing_pool: 0,
            verifier_rewards: HashMap::new(),
        }
    }

    pub fn create_escrow(
        &mut self,
        job_id: [u8; 32],
        client_id: [u8; 32],
        amount: u64,
        current_block: u64,
        deadline_blocks: u64,
    ) {
        let escrow = JobEscrow::new(
            job_id,
            client_id,
            amount,
            10,
            current_block,
            deadline_blocks,
        );
        self.job_escrows.insert(job_id, escrow);
    }

    pub fn get_escrow(&self, job_id: &[u8; 32]) -> Option<&JobEscrow> {
        self.job_escrows.get(job_id)
    }

    pub fn get_escrow_mut(&mut self, job_id: &[u8; 32]) -> Option<&mut JobEscrow> {
        self.job_escrows.get_mut(job_id)
    }

    pub fn release_escrow(&mut self, job_id: &[u8; 32], to: [u8; 32]) -> Result<u64, StateError> {
        let escrow = self
            .job_escrows
            .get_mut(job_id)
            .ok_or(StateError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Active {
            return Err(StateError::EscrowNotActive);
        }

        Ok(escrow.release(to))
    }

    pub fn refund_escrow(&mut self, job_id: &[u8; 32]) -> Result<u64, StateError> {
        let escrow = self
            .job_escrows
            .get_mut(job_id)
            .ok_or(StateError::EscrowNotFound)?;

        if escrow.status != EscrowStatus::Active {
            return Err(StateError::EscrowNotActive);
        }

        Ok(escrow.refund())
    }

    pub fn anchor_receipt(&mut self, receipt: ReceiptAnchor) {
        self.receipt_anchors.insert(receipt.receipt_id, receipt);
    }

    pub fn get_receipt(&self, receipt_id: &[u8; 32]) -> Option<&ReceiptAnchor> {
        self.receipt_anchors.get(receipt_id)
    }

    pub fn commit_artifact(&mut self, artifact: ArtifactCommitment) {
        self.artifacts.insert(artifact.artifact_id, artifact);
    }

    pub fn get_artifact(&self, artifact_id: &[u8; 32]) -> Option<&ArtifactCommitment> {
        self.artifacts.get(artifact_id)
    }

    pub fn add_slashed_to_pool(&mut self, amount: u64) {
        self.slashing_pool += amount;
    }

    pub fn distribute_verifier_rewards(
        &mut self,
        verifiers: &[[u8; 32]],
        amount: u64,
    ) -> Result<(), StateError> {
        if self.slashing_pool < amount {
            return Err(StateError::InsufficientBalance);
        }

        self.slashing_pool -= amount;

        let per_verifier = amount / verifiers.len() as u64;
        for v in verifiers {
            *self.verifier_rewards.entry(*v).or_insert(0) += per_verifier;
        }

        Ok(())
    }
}

impl Default for SettlementState {
    fn default() -> Self {
        Self::new()
    }
}

/// Extended State Error for Settlement Operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementError {
    EscrowNotFound,
    EscrowNotActive,
    OperatorNotFound,
    InsufficientStake,
    OperatorNotActive,
    ReceiptNotFound,
    ArtifactNotFound,
    OutsideChallengeWindow,
}

impl std::fmt::Display for SettlementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettlementError::EscrowNotFound => write!(f, "Escrow not found"),
            SettlementError::EscrowNotActive => write!(f, "Escrow not active"),
            SettlementError::OperatorNotFound => write!(f, "Operator not found"),
            SettlementError::InsufficientStake => write!(f, "Insufficient stake"),
            SettlementError::OperatorNotActive => write!(f, "Operator not active"),
            SettlementError::ReceiptNotFound => write!(f, "Receipt not found"),
            SettlementError::ArtifactNotFound => write!(f, "Artifact not found"),
            SettlementError::OutsideChallengeWindow => write!(f, "Outside challenge window"),
        }
    }
}

impl std::error::Error for SettlementError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_escrow_creation() {
        let escrow = JobEscrow::new([1u8; 32], [2u8; 32], 1000, 10, 100, 500);

        assert_eq!(escrow.job_id, [1u8; 32]);
        assert_eq!(escrow.client_id, [2u8; 32]);
        assert_eq!(escrow.amount, 1000);
        assert!(escrow.releaseable_amount() > 0);
    }

    #[test]
    fn test_job_escrow_release() {
        let mut escrow = JobEscrow::new([1u8; 32], [2u8; 32], 1000, 10, 100, 500);

        let released = escrow.release([3u8; 32]);

        assert_eq!(released, 1000);
        assert_eq!(escrow.status, EscrowStatus::Released);
    }

    #[test]
    fn test_job_escrow_refund() {
        let mut escrow = JobEscrow::new([1u8; 32], [2u8; 32], 1000, 10, 100, 500);

        let refunded = escrow.refund();

        assert_eq!(refunded, 1000);
        assert_eq!(escrow.status, EscrowStatus::Refunded);
    }

    #[test]
    fn test_job_escrow_freeze() {
        let mut escrow = JobEscrow::new([1u8; 32], [2u8; 32], 1000, 10, 100, 500);

        escrow.freeze();

        assert_eq!(escrow.status, EscrowStatus::Frozen);

        escrow.unfreeze();

        assert_eq!(escrow.status, EscrowStatus::Active);
    }

    #[test]
    fn test_job_escrow_expiry() {
        let escrow = JobEscrow::new([1u8; 32], [2u8; 32], 1000, 10, 100, 500);

        assert!(!escrow.is_expired(200));
        assert!(escrow.is_expired(700));
    }

    #[test]
    fn test_operator_registry_register() {
        let mut registry = OperatorRegistry::new();

        let operator = OperatorState {
            id: [1u8; 32],
            pubkey: [2u8; 32],
            stake: 5000,
            reputation: 5000,
            status: OperatorRegistrationStatus::Active,
            region: "us-east".to_string(),
            capabilities: vec![],
            slashed_count: 0,
            total_jobs_completed: 0,
            total_compute_units: 0,
            last_update_block: 100,
        };

        registry.register(operator);

        assert!(registry.is_active(&[1u8; 32]));
    }

    #[test]
    fn test_operator_registry_slash() {
        let mut registry = OperatorRegistry::new();

        let operator = OperatorState {
            id: [1u8; 32],
            pubkey: [2u8; 32],
            stake: 10000,
            reputation: 5000,
            status: OperatorRegistrationStatus::Active,
            region: "us-east".to_string(),
            capabilities: vec![],
            slashed_count: 0,
            total_jobs_completed: 0,
            total_compute_units: 0,
            last_update_block: 100,
        };

        registry.register(operator);
        let slashed = registry.slash(&[1u8; 32], 5000).unwrap();

        assert_eq!(slashed, 5000);
        let op = registry.get(&[1u8; 32]).unwrap();
        assert_eq!(op.status, OperatorRegistrationStatus::Slashed);
    }

    #[test]
    fn test_operator_registry_reputation() {
        let mut registry = OperatorRegistry::new();

        let operator = OperatorState {
            id: [1u8; 32],
            pubkey: [2u8; 32],
            stake: 5000,
            reputation: 5000,
            status: OperatorRegistrationStatus::Active,
            region: "us-east".to_string(),
            capabilities: vec![],
            slashed_count: 0,
            total_jobs_completed: 0,
            total_compute_units: 0,
            last_update_block: 100,
        };

        registry.register(operator);

        registry.increase_reputation(&[1u8; 32], 1000);
        assert_eq!(registry.get(&[1u8; 32]).unwrap().reputation, 6000);

        // Decrease by 5501 to go from 6000 to 499, triggering suspension (< 500)
        registry.decrease_reputation(&[1u8; 32], 5501);
        assert_eq!(
            registry.get(&[1u8; 32]).unwrap().status,
            OperatorRegistrationStatus::Suspended
        );
    }

    #[test]
    fn test_receipt_anchor() {
        let receipt = ReceiptAnchor::new([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], 1000, 100);

        assert_eq!(receipt.job_id, [1u8; 32]);
        assert_eq!(receipt.operator_id, [2u8; 32]);
        assert_eq!(receipt.output_hash, [3u8; 32]);
    }

    #[test]
    fn test_receipt_verify() {
        let mut receipt = ReceiptAnchor::new([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], 1000, 100);

        receipt.verify([5u8; 32], 150);

        assert_eq!(receipt.status, ReceiptStatus::Verified);
    }

    #[test]
    fn test_receipt_challenge() {
        let mut receipt = ReceiptAnchor::new([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], 1000, 100);

        receipt.challenge([5u8; 32]);

        assert_eq!(receipt.status, ReceiptStatus::Challenged);
        assert_eq!(receipt.challenger_id, Some([5u8; 32]));
    }

    #[test]
    fn test_artifact_commitment() {
        let content = b"test artifact content";
        let artifact = ArtifactCommitment::new(content, [1u8; 32], 100);

        assert!(artifact.verify(content));
    }

    #[test]
    fn test_settlement_state_escrow() {
        let mut state = SettlementState::new();

        state.create_escrow([1u8; 32], [2u8; 32], 1000, 100, 500);

        let escrow = state.get_escrow(&[1u8; 32]).unwrap();
        assert_eq!(escrow.amount, 1000);
    }

    #[test]
    fn test_settlement_state_release_escrow() {
        let mut state = SettlementState::new();

        state.create_escrow([1u8; 32], [2u8; 32], 1000, 100, 500);

        let released = state.release_escrow(&[1u8; 32], [3u8; 32]).unwrap();

        assert_eq!(released, 1000);
    }

    #[test]
    fn test_settlement_state_refund_escrow() {
        let mut state = SettlementState::new();

        state.create_escrow([1u8; 32], [2u8; 32], 1000, 100, 500);

        let refunded = state.refund_escrow(&[1u8; 32]).unwrap();

        assert_eq!(refunded, 1000);
    }

    #[test]
    fn test_slashing_conditions() {
        let conditions = SlashingConditions::default();

        let slash_amount = conditions.calculate_slash_amount(10000);

        assert_eq!(slash_amount, 5000);
        assert!(conditions.should_slash(3));
    }

    #[test]
    fn test_settlement_state_refund_escrow_not_found() {
        let mut state = SettlementState::new();

        let result = state.refund_escrow(&[1u8; 32]);
        assert!(result.is_err());
    }

    #[test]
    fn test_settlement_state_release_escrow_not_found() {
        let mut state = SettlementState::new();

        let result = state.release_escrow(&[1u8; 32], [3u8; 32]);
        assert!(result.is_err());
    }

    #[test]
    fn test_settlement_state_escrow_wrong_status() {
        let mut state = SettlementState::new();

        state.create_escrow([1u8; 32], [2u8; 32], 1000, 100, 500);

        // Release first time
        state.release_escrow(&[1u8; 32], [3u8; 32]).unwrap();

        // Try to release again - should fail
        let result = state.release_escrow(&[1u8; 32], [3u8; 32]);
        assert!(result.is_err());
    }

    #[test]
    fn test_receipt_anchor_challenge_window() {
        let receipt = ReceiptAnchor::new([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], 1000, 100);

        assert!(receipt.is_within_challenge_window(150, 50));
        assert!(!receipt.is_within_challenge_window(200, 50));
    }

    #[test]
    fn test_receipt_anchor_settle() {
        let mut receipt = ReceiptAnchor::new([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], 1000, 100);

        receipt.settle(200);

        assert_eq!(receipt.status, ReceiptStatus::Settled);
        assert_eq!(receipt.settled_at_block, Some(200));
    }

    #[test]
    fn test_artifact_commitment_seal() {
        let content = b"test artifact content";
        let mut artifact = ArtifactCommitment::new(content, [1u8; 32], 100);

        assert!(!artifact.sealed);

        artifact.seal();

        assert!(artifact.sealed);
    }

    #[test]
    fn test_artifact_commitment_verify_wrong_content() {
        let content = b"test artifact content";
        let artifact = ArtifactCommitment::new(content, [1u8; 32], 100);

        assert!(!artifact.verify(b"wrong content"));
    }

    #[test]
    fn test_operator_registry_deactivate() {
        let mut registry = OperatorRegistry::new();

        let operator = OperatorState {
            id: [1u8; 32],
            pubkey: [2u8; 32],
            stake: 5000,
            reputation: 5000,
            status: OperatorRegistrationStatus::Active,
            region: "us-east".to_string(),
            capabilities: vec![],
            slashed_count: 0,
            total_jobs_completed: 0,
            total_compute_units: 0,
            last_update_block: 100,
        };

        registry.register(operator);

        registry.deactivate(&[1u8; 32]);

        assert!(!registry.is_active(&[1u8; 32]));
    }

    #[test]
    fn test_operator_registry_update_stake() {
        let mut registry = OperatorRegistry::new();

        let operator = OperatorState {
            id: [1u8; 32],
            pubkey: [2u8; 32],
            stake: 5000,
            reputation: 5000,
            status: OperatorRegistrationStatus::Active,
            region: "us-east".to_string(),
            capabilities: vec![],
            slashed_count: 0,
            total_jobs_completed: 0,
            total_compute_units: 0,
            last_update_block: 100,
        };

        registry.register(operator);

        registry.update_stake(&[1u8; 32], 10000).unwrap();

        assert_eq!(registry.get(&[1u8; 32]).unwrap().stake, 10000);
    }

    #[test]
    fn test_operator_registry_update_stake_not_found() {
        let mut registry = OperatorRegistry::new();

        let result = registry.update_stake(&[1u8; 32], 5000);
        assert!(result.is_err());
    }

    #[test]
    fn test_operator_registry_get_active_operators() {
        let mut registry = OperatorRegistry::new();

        // Add active operator
        let op1 = OperatorState {
            id: [1u8; 32],
            pubkey: [2u8; 32],
            stake: 5000,
            reputation: 5000,
            status: OperatorRegistrationStatus::Active,
            region: "us-east".to_string(),
            capabilities: vec![],
            slashed_count: 0,
            total_jobs_completed: 0,
            total_compute_units: 0,
            last_update_block: 100,
        };

        // Add inactive operator
        let op2 = OperatorState {
            id: [2u8; 32],
            pubkey: [3u8; 32],
            stake: 3000,
            reputation: 3000,
            status: OperatorRegistrationStatus::Suspended,
            region: "us-west".to_string(),
            capabilities: vec![],
            slashed_count: 0,
            total_jobs_completed: 0,
            total_compute_units: 0,
            last_update_block: 100,
        };

        registry.register(op1);
        registry.register(op2);

        let active = registry.get_active_operators();
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn test_receipt_anchor_escalate() {
        let mut receipt = ReceiptAnchor::new([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], 1000, 100);

        receipt.escalate();

        assert_eq!(receipt.status, ReceiptStatus::Disputed);
    }

    #[test]
    fn test_settlement_state_anchor_receipt() {
        let mut state = SettlementState::new();

        let receipt = ReceiptAnchor::new([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], 1000, 100);

        state.anchor_receipt(receipt.clone());

        let retrieved = state.get_receipt(&receipt.receipt_id);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_settlement_state_commit_artifact() {
        let mut state = SettlementState::new();

        let artifact = ArtifactCommitment::new(b"content", [1u8; 32], 100);

        state.commit_artifact(artifact.clone());

        let retrieved = state.get_artifact(&artifact.artifact_id);
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_settlement_state_verifier_rewards() {
        let mut state = SettlementState::new();
        state.slashing_pool = 1000;

        let verifiers = [[1u8; 32], [2u8; 32], [3u8; 32]];

        state.distribute_verifier_rewards(&verifiers, 300).unwrap();

        assert_eq!(state.verifier_rewards.get(&[1u8; 32]), Some(&100));
        assert_eq!(state.verifier_rewards.get(&[2u8; 32]), Some(&100));
        assert_eq!(state.verifier_rewards.get(&[3u8; 32]), Some(&100));
    }

    #[test]
    fn test_settlement_state_verifier_rewards_insufficient() {
        let mut state = SettlementState::new();
        state.slashing_pool = 100;

        let verifiers = [[1u8; 32], [2u8; 32], [3u8; 32]];

        let result = state.distribute_verifier_rewards(&verifiers, 300);
        assert!(result.is_err());
    }
}
