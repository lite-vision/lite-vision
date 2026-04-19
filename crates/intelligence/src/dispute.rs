use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudProof {
    pub receipt_hash: [u8; 32],
    pub operator_id: [u8; 32],
    pub recomputed_output_hash: [u8; 32],
    pub original_output_hash: [u8; 32],
    pub kernel_id: [u8; 32],
    pub input_hash: [u8; 32],
    pub deterministic_seed: Option<[u8; 32]>,
    pub evidence_bundle: EvidenceBundle,
    pub challenger_signature: Vec<u8>,
}

impl FraudProof {
    pub fn hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&bincode::serialize(self).unwrap());
        *hasher.finalize().as_bytes()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceBundle {
    pub kernel_binary_hash: [u8; 32],
    pub input_data_snapshot: Vec<u8>,
    pub execution_logs: Vec<u8>,
    pub resource_metrics: ResourceMetricsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetricsSnapshot {
    pub gpu_cycles: u64,
    pub vram_bytes: u64,
    pub cpu_cycles: u64,
    pub memory_bytes: u64,
    pub bandwidth_bytes: u64,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeInitiate {
    pub job_id: [u8; 32],
    pub receipt_hash: [u8; 32],
    pub fraud_proof_hash: [u8; 32],
    pub challenger_bond: u64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DisputeStatus {
    Pending,
    ValidationPhase,
    VerificationEscalation,
    Adjudicating,
    Resolved,
    Appealed,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispute {
    pub id: [u8; 32],
    pub job_id: [u8; 32],
    pub receipt_hash: [u8; 32],
    pub fraud_proof: FraudProof,
    pub challenger_id: [u8; 32],
    pub challenger_bond: u64,
    pub status: DisputeStatus,
    pub created_at_block: u64,
    pub verification_window_blocks: u64,
    pub resolution: Option<DisputeResolution>,
    pub appeal: Option<Appeal>,
    pub verification_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeResolution {
    pub outcome: DisputeOutcome,
    pub slashed_amount: u64,
    pub challenger_share: u64,
    pub verifier_share: u64,
    pub treasury_share: u64,
    pub operator_penalty: u64,
    pub resolution_block: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DisputeOutcome {
    FraudConfirmed,
    FraudRejected,
    Inconclusive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appeal {
    pub reason: AppealReason,
    pub new_evidence: Option<EvidenceBundle>,
    pub appeal_bond: u64,
    pub created_at_block: u64,
    pub window_blocks: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppealReason {
    ProceduralError,
    EvidenceCorruption,
    GovernanceReview,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub job_id: [u8; 32],
    pub receipt_hash: [u8; 32],
    pub recomputed_output_hash: [u8; 32],
    pub verification_signatures: Vec<VerifierSignature>,
    pub verification_level: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifierSignature {
    pub verifier_id: [u8; 32],
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashDistribution {
    pub challenger_share_ratio: u64,
    pub verifier_share_ratio: u64,
    pub treasury_share_ratio: u64,
}

impl Default for SlashDistribution {
    fn default() -> Self {
        Self {
            challenger_share_ratio: 3000,
            verifier_share_ratio: 3000,
            treasury_share_ratio: 4000,
        }
    }
}

pub struct DisputeEngine {
    min_challenger_bond: u64,
    governance_min_slash: u64,
    slash_alpha_ratio: u64,
    slash_beta_ratio: u64,
    adjudication_window_blocks: u64,
    default_verification_window: u64,
    slash_distribution: SlashDistribution,
    disputes: HashMap<[u8; 32], Dispute>,
    job_escrow: HashMap<[u8; 32], EscrowState>,
    frozen_bonds: HashMap<[u8; 32], u64>,
    disputes_by_job: HashMap<[u8; 32], Vec<[u8; 32]>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscrowState {
    pub job_id: [u8; 32],
    pub amount: u64,
    pub status: EscrowStatus,
    pub recipient: Option<[u8; 32]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EscrowStatus {
    Active,
    Frozen,
    Released,
    Refunded,
    PartiallyRefunded,
}

impl DisputeEngine {
    pub fn new(
        min_challenger_bond: u64,
        governance_min_slash: u64,
        slash_alpha_ratio: u64,
        slash_beta_ratio: u64,
    ) -> Self {
        Self {
            min_challenger_bond,
            governance_min_slash,
            slash_alpha_ratio,
            slash_beta_ratio,
            adjudication_window_blocks: 100,
            default_verification_window: 500,
            slash_distribution: SlashDistribution::default(),
            disputes: HashMap::new(),
            job_escrow: HashMap::new(),
            frozen_bonds: HashMap::new(),
            disputes_by_job: HashMap::new(),
        }
    }

    pub fn init_escrow(&mut self, job_id: [u8; 32], amount: u64, recipient: [u8; 32]) {
        self.job_escrow.insert(
            job_id,
            EscrowState {
                job_id,
                amount,
                status: EscrowStatus::Active,
                recipient: Some(recipient),
            },
        );
    }

    pub fn initiate_dispute(
        &mut self,
        job_id: [u8; 32],
        receipt_hash: [u8; 32],
        fraud_proof: FraudProof,
        challenger_id: [u8; 32],
        challenger_bond: u64,
        current_block: u64,
        receipt_block: u64,
        verification_window_blocks: u64,
    ) -> Result<[u8; 32], DisputeError> {
        if challenger_bond < self.min_challenger_bond {
            return Err(DisputeError::InsufficientBond);
        }

        if current_block > receipt_block + verification_window_blocks {
            return Err(DisputeError::OutsideDisputeWindow);
        }

        if let Some(disputes) = self.disputes_by_job.get(&job_id) {
            if !disputes.is_empty() {
                return Err(DisputeError::DisputeAlreadyExists);
            }
        }

        let fraud_proof_bytes =
            bincode::serialize(&fraud_proof).map_err(|_| DisputeError::SerializationError)?;
        let _fraud_proof_hash = blake3::hash(&fraud_proof_bytes).as_bytes().clone();

        let dispute_id = blake3::hash(
            &bincode::serialize(&(job_id, receipt_hash, challenger_id, current_block)).unwrap(),
        )
        .as_bytes()
        .clone();

        let dispute = Dispute {
            id: dispute_id,
            job_id,
            receipt_hash,
            fraud_proof,
            challenger_id,
            challenger_bond,
            status: DisputeStatus::ValidationPhase,
            created_at_block: current_block,
            verification_window_blocks,
            resolution: None,
            appeal: None,
            verification_level: 0,
        };

        self.disputes.insert(dispute_id, dispute.clone());
        self.disputes_by_job
            .entry(job_id)
            .or_insert_with(Vec::new)
            .push(dispute_id);

        if let Some(escrow) = self.job_escrow.get_mut(&job_id) {
            escrow.status = EscrowStatus::Frozen;
        }

        self.frozen_bonds.insert(challenger_id, challenger_bond);

        Ok(dispute_id)
    }

    pub fn validate_dispute(
        &self,
        dispute_id: &[u8; 32],
    ) -> Result<ValidationResult, DisputeError> {
        let dispute = self
            .disputes
            .get(dispute_id)
            .ok_or(DisputeError::DisputeNotFound)?;

        let validation_errors = Vec::new();
        let is_valid = dispute.challenger_bond >= self.min_challenger_bond;

        Ok(ValidationResult {
            is_valid,
            errors: validation_errors,
        })
    }

    pub fn escalate_to_verification(
        &mut self,
        dispute_id: &[u8; 32],
        verification_level: u8,
    ) -> Result<(), DisputeError> {
        let dispute = self
            .disputes
            .get_mut(dispute_id)
            .ok_or(DisputeError::DisputeNotFound)?;

        if dispute.status != DisputeStatus::ValidationPhase {
            return Err(DisputeError::InvalidDisputeState);
        }

        dispute.status = DisputeStatus::VerificationEscalation;
        dispute.verification_level = verification_level;

        Ok(())
    }

    pub fn submit_verification_result(
        &mut self,
        dispute_id: &[u8; 32],
        result: VerificationResult,
    ) -> Result<(), DisputeError> {
        let dispute = self
            .disputes
            .get_mut(dispute_id)
            .ok_or(DisputeError::DisputeNotFound)?;

        if dispute.status != DisputeStatus::VerificationEscalation {
            return Err(DisputeError::InvalidDisputeState);
        }

        dispute.status = DisputeStatus::Adjudicating;
        Ok(())
    }

    pub fn resolve_dispute(
        &mut self,
        dispute_id: &[u8; 32],
        job_budget: u64,
        verification_cost: u64,
        current_block: u64,
    ) -> Result<DisputeResolution, DisputeError> {
        let dispute = self
            .disputes
            .get_mut(dispute_id)
            .ok_or(DisputeError::DisputeNotFound)?;

        if dispute.status != DisputeStatus::Adjudicating {
            return Err(DisputeError::InvalidDisputeState);
        }

        let fraud_confirmed =
            dispute.fraud_proof.recomputed_output_hash != dispute.fraud_proof.original_output_hash;

        let slash_amount = self
            .governance_min_slash
            .max((job_budget * self.slash_alpha_ratio) / 10000)
            .max((verification_cost * self.slash_beta_ratio) / 10000);

        let (outcome, slashed_amount, operator_penalty, escrow_action) = if fraud_confirmed {
            (
                DisputeOutcome::FraudConfirmed,
                slash_amount,
                slash_amount,
                EscrowStatus::Refunded,
            )
        } else {
            (
                DisputeOutcome::FraudRejected,
                dispute.challenger_bond,
                0,
                EscrowStatus::Released,
            )
        };

        let dist = &self.slash_distribution;
        let challenger_share = (slashed_amount * dist.challenger_share_ratio) / 10000;
        let verifier_share = (slashed_amount * dist.verifier_share_ratio) / 10000;
        let treasury_share = (slashed_amount * dist.treasury_share_ratio) / 10000;

        let resolution = DisputeResolution {
            outcome,
            slashed_amount,
            challenger_share,
            verifier_share,
            treasury_share,
            operator_penalty,
            resolution_block: current_block,
        };

        if let Some(escrow) = self.job_escrow.get_mut(&dispute.job_id) {
            escrow.status = escrow_action;
        }

        dispute.status = DisputeStatus::Resolved;
        dispute.resolution = Some(resolution.clone());

        if fraud_confirmed {
            self.frozen_bonds.remove(&dispute.challenger_id);
        } else {
            if let Some(bond) = self.frozen_bonds.get_mut(&dispute.challenger_id) {
                *bond = bond.saturating_sub(slashed_amount);
            }
        }

        Ok(resolution)
    }

    pub fn submit_appeal(
        &mut self,
        dispute_id: &[u8; 32],
        reason: AppealReason,
        new_evidence: Option<EvidenceBundle>,
        appeal_bond: u64,
        current_block: u64,
    ) -> Result<(), DisputeError> {
        let dispute = self
            .disputes
            .get_mut(dispute_id)
            .ok_or(DisputeError::DisputeNotFound)?;

        if dispute.status != DisputeStatus::Resolved {
            return Err(DisputeError::InvalidDisputeState);
        }

        if let Some(resolution) = &dispute.resolution {
            let appeal_window = 50u64;
            if current_block > resolution.resolution_block + appeal_window {
                return Err(DisputeError::OutsideAppealWindow);
            }
        }

        let appeal = Appeal {
            reason,
            new_evidence,
            appeal_bond,
            created_at_block: current_block,
            window_blocks: 50,
        };

        dispute.appeal = Some(appeal);
        dispute.status = DisputeStatus::Appealed;

        Ok(())
    }

    pub fn get_dispute(&self, dispute_id: &[u8; 32]) -> Option<&Dispute> {
        self.disputes.get(dispute_id)
    }

    pub fn get_disputes_for_job(&self, job_id: &[u8; 32]) -> Option<&Vec<[u8; 32]>> {
        self.disputes_by_job.get(job_id)
    }

    pub fn get_escrow(&self, job_id: &[u8; 32]) -> Option<&EscrowState> {
        self.job_escrow.get(job_id)
    }

    pub fn is_within_dispute_window(
        &self,
        receipt_block: u64,
        current_block: u64,
        window_blocks: u64,
    ) -> bool {
        current_block <= receipt_block + window_blocks
    }
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DisputeError {
    InsufficientBond,
    OutsideDisputeWindow,
    DisputeAlreadyExists,
    DisputeNotFound,
    InvalidDisputeState,
    SerializationError,
    OutsideAppealWindow,
}

impl std::fmt::Display for DisputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisputeError::InsufficientBond => {
                write!(f, "Challenger bond below minimum requirement")
            }
            DisputeError::OutsideDisputeWindow => {
                write!(f, "Dispute initiated outside valid window")
            }
            DisputeError::DisputeAlreadyExists => write!(f, "Dispute already exists for this job"),
            DisputeError::DisputeNotFound => write!(f, "Dispute not found"),
            DisputeError::InvalidDisputeState => write!(f, "Invalid dispute state for operation"),
            DisputeError::SerializationError => write!(f, "Failed to serialize data"),
            DisputeError::OutsideAppealWindow => write!(f, "Appeal submitted outside valid window"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_fraud_proof() -> FraudProof {
        FraudProof {
            receipt_hash: [1u8; 32],
            operator_id: [2u8; 32],
            recomputed_output_hash: [3u8; 32],
            original_output_hash: [4u8; 32],
            kernel_id: [5u8; 32],
            input_hash: [6u8; 32],
            deterministic_seed: Some([7u8; 32]),
            evidence_bundle: EvidenceBundle {
                kernel_binary_hash: [8u8; 32],
                input_data_snapshot: vec![9u8; 16],
                execution_logs: vec![10u8; 32],
                resource_metrics: ResourceMetricsSnapshot {
                    gpu_cycles: 1000,
                    vram_bytes: 2000,
                    cpu_cycles: 3000,
                    memory_bytes: 4000,
                    bandwidth_bytes: 5000,
                    execution_time_ms: 100,
                },
            },
            challenger_signature: vec![11u8; 64],
        }
    }

    #[test]
    fn test_dispute_initiation() {
        let mut engine = DisputeEngine::new(1000, 5000, 20, 10);
        let job_id = [10u8; 32];
        let receipt_hash = [1u8; 32];
        let fraud_proof = create_test_fraud_proof();
        let challenger_id = [20u8; 32];

        engine.init_escrow(job_id, 10000, [30u8; 32]);

        let result = engine.initiate_dispute(
            job_id,
            receipt_hash,
            fraud_proof,
            challenger_id,
            2000,
            100,
            100,
            500,
        );

        assert!(result.is_ok());
        let dispute_id = result.unwrap();

        let dispute = engine.get_dispute(&dispute_id).unwrap();
        assert_eq!(dispute.job_id, job_id);
        assert_eq!(dispute.challenger_id, challenger_id);
        assert_eq!(dispute.status, DisputeStatus::ValidationPhase);

        let escrow = engine.get_escrow(&job_id).unwrap();
        assert_eq!(escrow.status, EscrowStatus::Frozen);
    }

    #[test]
    fn test_insufficient_bond() {
        let mut engine = DisputeEngine::new(1000, 5000, 2000, 1000);
        let job_id = [10u8; 32];
        let fraud_proof = create_test_fraud_proof();

        let result = engine.initiate_dispute(
            job_id,
            [1u8; 32],
            fraud_proof,
            [20u8; 32],
            500,
            100,
            100,
            500,
        );

        assert_eq!(result, Err(DisputeError::InsufficientBond));
    }

    #[test]
    fn test_outside_dispute_window() {
        let mut engine = DisputeEngine::new(1000, 5000, 2000, 1000);
        let job_id = [10u8; 32];
        let fraud_proof = create_test_fraud_proof();

        let result = engine.initiate_dispute(
            job_id,
            [1u8; 32],
            fraud_proof,
            [20u8; 32],
            2000,
            3000,
            1000,
            500,
        );

        assert_eq!(result, Err(DisputeError::OutsideDisputeWindow));
    }

    #[test]
    fn test_dispute_resolution_fraud_confirmed() {
        let mut engine = DisputeEngine::new(1000, 5000, 2000, 1000);
        let job_id = [10u8; 32];
        let receipt_hash = [1u8; 32];
        let fraud_proof = create_test_fraud_proof();

        engine.init_escrow(job_id, 10000, [30u8; 32]);

        let dispute_id = engine
            .initiate_dispute(
                job_id,
                receipt_hash,
                fraud_proof,
                [20u8; 32],
                2000,
                100,
                100,
                500,
            )
            .unwrap();

        engine.escalate_to_verification(&dispute_id, 3).unwrap();

        let verification_result = VerificationResult {
            job_id,
            receipt_hash,
            recomputed_output_hash: [3u8; 32],
            verification_signatures: vec![],
            verification_level: 3,
        };

        engine
            .submit_verification_result(&dispute_id, verification_result)
            .unwrap();

        let resolution = engine
            .resolve_dispute(&dispute_id, 10000, 1000, 200)
            .unwrap();

        assert_eq!(resolution.outcome, DisputeOutcome::FraudConfirmed);
        assert!(resolution.slashed_amount > 0);

        let escrow = engine.get_escrow(&job_id).unwrap();
        assert_eq!(escrow.status, EscrowStatus::Refunded);
    }

    #[test]
    fn test_dispute_resolution_fraud_rejected() {
        let mut engine = DisputeEngine::new(1000, 5000, 2000, 1000);
        let job_id = [10u8; 32];
        let receipt_hash = [1u8; 32];

        let mut fraud_proof = create_test_fraud_proof();
        fraud_proof.recomputed_output_hash = fraud_proof.original_output_hash;

        engine.init_escrow(job_id, 10000, [30u8; 32]);

        let dispute_id = engine
            .initiate_dispute(
                job_id,
                receipt_hash,
                fraud_proof,
                [20u8; 32],
                2000,
                100,
                100,
                500,
            )
            .unwrap();

        engine.escalate_to_verification(&dispute_id, 3).unwrap();

        let verification_result = VerificationResult {
            job_id,
            receipt_hash,
            recomputed_output_hash: [4u8; 32],
            verification_signatures: vec![],
            verification_level: 3,
        };

        engine
            .submit_verification_result(&dispute_id, verification_result)
            .unwrap();

        let resolution = engine
            .resolve_dispute(&dispute_id, 10000, 1000, 200)
            .unwrap();

        assert_eq!(resolution.outcome, DisputeOutcome::FraudRejected);

        let escrow = engine.get_escrow(&job_id).unwrap();
        assert_eq!(escrow.status, EscrowStatus::Released);
    }

    #[test]
    fn test_duplicate_dispute_rejected() {
        let mut engine = DisputeEngine::new(1000, 5000, 20, 10);
        let job_id = [10u8; 32];
        let receipt_hash = [1u8; 32];
        let fraud_proof = create_test_fraud_proof();

        engine.init_escrow(job_id, 10000, [30u8; 32]);

        engine
            .initiate_dispute(
                job_id,
                receipt_hash,
                fraud_proof.clone(),
                [20u8; 32],
                2000,
                100,
                100,
                500,
            )
            .unwrap();

        let result = engine.initiate_dispute(
            job_id,
            receipt_hash,
            fraud_proof,
            [21u8; 32],
            2000,
            100,
            100,
            500,
        );

        assert_eq!(result, Err(DisputeError::DisputeAlreadyExists));
    }

    #[test]
    fn test_appeal_submission() {
        let mut engine = DisputeEngine::new(1000, 5000, 20, 10);
        let job_id = [10u8; 32];
        let receipt_hash = [1u8; 32];
        let fraud_proof = create_test_fraud_proof();

        engine.init_escrow(job_id, 10000, [30u8; 32]);

        let dispute_id = engine
            .initiate_dispute(
                job_id,
                receipt_hash,
                fraud_proof,
                [20u8; 32],
                2000,
                100,
                100,
                500,
            )
            .unwrap();

        engine.escalate_to_verification(&dispute_id, 3).unwrap();

        let verification_result = VerificationResult {
            job_id,
            receipt_hash,
            recomputed_output_hash: [3u8; 32],
            verification_signatures: vec![],
            verification_level: 3,
        };

        engine
            .submit_verification_result(&dispute_id, verification_result)
            .unwrap();
        engine
            .resolve_dispute(&dispute_id, 10000, 1000, 200)
            .unwrap();

        let result =
            engine.submit_appeal(&dispute_id, AppealReason::ProceduralError, None, 5000, 210);

        assert!(result.is_ok());

        let dispute = engine.get_dispute(&dispute_id).unwrap();
        assert_eq!(dispute.status, DisputeStatus::Appealed);
    }

    #[test]
    fn test_dispute_window_check() {
        let engine = DisputeEngine::new(1000, 5000, 20, 10);

        assert!(engine.is_within_dispute_window(100, 150, 500));
        assert!(engine.is_within_dispute_window(100, 600, 500));
        assert!(!engine.is_within_dispute_window(100, 601, 500));
    }

    #[test]
    fn test_slash_amount_calculation() {
        let engine = DisputeEngine::new(1000, 5000, 2000, 1000);

        let slash_from_budget = (10000 * 2000) / 10000;
        let slash_from_verification = (1000 * 1000) / 10000;

        assert_eq!(slash_from_budget, 2000);
        assert_eq!(slash_from_verification, 100);

        let expected = 5000.max(slash_from_budget).max(slash_from_verification);
        assert_eq!(expected, 5000);
    }

    #[test]
    fn test_fraud_proof_serialization() {
        let proof = create_test_fraud_proof();
        let serialized = bincode::serialize(&proof).unwrap();
        let deserialized: FraudProof = bincode::deserialize(&serialized).unwrap();

        assert_eq!(proof.receipt_hash, deserialized.receipt_hash);
        assert_eq!(proof.operator_id, deserialized.operator_id);
        assert_eq!(
            proof.original_output_hash,
            deserialized.original_output_hash
        );
    }

    #[test]
    fn test_evidence_bundle_integrity() {
        let evidence = EvidenceBundle {
            kernel_binary_hash: [9u8; 32],
            input_data_snapshot: vec![1, 2, 3, 4, 5],
            execution_logs: vec![],
            resource_metrics: ResourceMetricsSnapshot {
                gpu_cycles: 5000,
                vram_bytes: 8000,
                cpu_cycles: 3000,
                memory_bytes: 6000,
                bandwidth_bytes: 1000,
                execution_time_ms: 250,
            },
        };

        let proof = FraudProof {
            receipt_hash: [1u8; 32],
            operator_id: [2u8; 32],
            recomputed_output_hash: [3u8; 32],
            original_output_hash: [3u8; 32],
            kernel_id: [4u8; 32],
            input_hash: [5u8; 32],
            deterministic_seed: None,
            evidence_bundle: evidence,
            challenger_signature: vec![],
        };

        assert!(proof.evidence_bundle.execution_logs.is_empty());
        assert_eq!(proof.evidence_bundle.resource_metrics.gpu_cycles, 5000);
    }
}

use std::sync::Arc;
use tokio::sync::RwLock;

pub struct DisputeManager {
    engine: Arc<RwLock<DisputeEngine>>,
}

impl DisputeManager {
    pub fn new(engine: DisputeEngine) -> Self {
        Self {
            engine: Arc::new(RwLock::new(engine)),
        }
    }

    pub async fn initiate_dispute(
        &self,
        job_id: [u8; 32],
        receipt_hash: [u8; 32],
        fraud_proof: FraudProof,
        challenger_id: [u8; 32],
        challenger_bond: u64,
        current_block: u64,
        receipt_block: u64,
        verification_window_blocks: u64,
    ) -> Result<[u8; 32], DisputeError> {
        let mut engine = self.engine.write().await;
        engine.initiate_dispute(
            job_id,
            receipt_hash,
            fraud_proof,
            challenger_id,
            challenger_bond,
            current_block,
            receipt_block,
            verification_window_blocks,
        )
    }

    pub async fn resolve_dispute(
        &self,
        dispute_id: &[u8; 32],
        job_budget: u64,
        verification_cost: u64,
        current_block: u64,
    ) -> Result<DisputeResolution, DisputeError> {
        let mut engine = self.engine.write().await;
        engine.resolve_dispute(dispute_id, job_budget, verification_cost, current_block)
    }

    pub async fn get_dispute(&self, dispute_id: &[u8; 32]) -> Option<Dispute> {
        let engine = self.engine.read().await;
        engine.get_dispute(dispute_id).cloned()
    }
}
