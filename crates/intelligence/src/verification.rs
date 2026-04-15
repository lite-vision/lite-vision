use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Verification engine for job result verification
/// Integrates with job lifecycle: samples executed jobs for verification
/// and triggers disputes on failure

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationMode {
    None,
    Probabilistic,
    Deterministic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedundancyPolicy {
    Parallel,
    Sequential,
    Majority,
    FirstValid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationPolicy {
    pub mode: VerificationMode,
    pub redundancy_factor: u32,
    pub verification_rate: f64,
    pub escalation_threshold: u32,
    pub challenge_window_blocks: u32,
    pub sampling_strategy: SamplingStrategy,
}

impl Default for VerificationPolicy {
    fn default() -> Self {
        Self {
            mode: VerificationMode::Probabilistic,
            redundancy_factor: 1,
            verification_rate: 0.1,
            escalation_threshold: 2,
            challenge_window_blocks: 100,
            sampling_strategy: SamplingStrategy::Random,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SamplingStrategy {
    Random,
    Weighted,
    Deterministic,
    Targeted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationJob {
    pub job_id: [u8; 32],
    pub operator_id: [u8; 32],
    pub input_hash: [u8; 32],
    pub output_hash: [u8; 32],
    pub expected_hash: Option<[u8; 32]>,
    pub verification_mode: VerificationMode,
    pub status: VerificationStatus,
    pub created_at: u64,
    pub completed_at: Option<u64>,
    pub result: Option<VerificationResult>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    Pending,
    InProgress,
    Completed,
    Challenged,
    Escalated,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub matches: bool,
    pub computed_hash: [u8; 32],
    pub execution_time_ms: u64,
    pub verifier_id: [u8; 32],
    pub confidence: f64,
    pub evidence: Vec<VerificationEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationEvidence {
    pub evidence_type: EvidenceType,
    pub data: Vec<u8>,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceType {
    OutputMismatch,
    ExecutionDiff,
    Timeout,
    InvalidState,
    SignatureInvalid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispute {
    pub dispute_id: [u8; 32],
    pub job_id: [u8; 32],
    pub challenger_id: [u8; 32],
    pub accused_operator_id: [u8; 32],
    pub evidence: Vec<VerificationEvidence>,
    pub status: DisputeStatus,
    pub created_at: u64,
    pub resolved_at: Option<u64>,
    pub slash_amount: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisputeStatus {
    Pending,
    Voting,
    Resolved,
    Rejected,
    Expired,
}

impl Dispute {
    pub fn new(
        job_id: [u8; 32],
        challenger_id: [u8; 32],
        accused_operator_id: [u8; 32],
        evidence: Vec<VerificationEvidence>,
    ) -> Self {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&job_id);
        hasher.update(&challenger_id);
        hasher.update(&accused_operator_id);

        Self {
            dispute_id: *hasher.finalize().as_bytes(),
            job_id,
            challenger_id,
            accused_operator_id,
            evidence,
            status: DisputeStatus::Pending,
            created_at: current_timestamp(),
            resolved_at: None,
            slash_amount: None,
        }
    }
}

pub struct VerificationEngine {
    pub verifications: HashMap<[u8; 32], VerificationJob>,
    pub disputes: HashMap<[u8; 32], Dispute>,
    pub verification_queue: VecDeque<[u8; 32]>,
    pub policy: VerificationPolicy,
    pub challenge_counter: HashMap<[u8; 32], u32>,
    /// Metrics tracking
    pub total_sampled: u64,
    pub total_passed: u64,
    pub total_failed: u64,
    pub total_disputes: u64,
}

impl VerificationEngine {
    pub fn new(policy: VerificationPolicy) -> Self {
        Self {
            verifications: HashMap::new(),
            disputes: HashMap::new(),
            verification_queue: VecDeque::new(),
            policy,
            challenge_counter: HashMap::new(),
            total_sampled: 0,
            total_passed: 0,
            total_failed: 0,
            total_disputes: 0,
        }
    }

    /// Sample a completed job for verification
    /// Called when a job completes execution
    pub fn sample_job(
        &mut self,
        job_id: [u8; 32],
        operator_id: [u8; 32],
        input_hash: [u8; 32],
        output_hash: [u8; 32],
    ) -> bool {
        // Check if we should verify this job based on policy rate
        if !self.should_verify() {
            return false;
        }

        self.total_sampled += 1;

        let verification_job = VerificationJob {
            job_id,
            operator_id,
            input_hash,
            output_hash,
            expected_hash: None,
            verification_mode: self.policy.mode,
            status: VerificationStatus::Pending,
            created_at: current_timestamp(),
            completed_at: None,
            result: None,
        };

        self.schedule_verification(verification_job);
        true
    }

    /// Record verification result and update metrics
    pub fn record_verification_result(
        &mut self,
        job_id: &[u8; 32],
        result: VerificationResult,
    ) -> Result<VerificationStatus, VerificationError> {
        let status = self.complete_verification(job_id, result.clone())?;

        if result.matches {
            self.total_passed += 1;
        } else {
            self.total_failed += 1;
        }

        Ok(status)
    }

    /// Get verification metrics
    pub fn get_metrics(&self) -> VerificationMetrics {
        let pass_rate = if self.total_sampled > 0 {
            self.total_passed as f64 / self.total_sampled as f64
        } else {
            0.0
        };

        VerificationMetrics {
            total_sampled: self.total_sampled,
            total_passed: self.total_passed,
            total_failed: self.total_failed,
            total_disputes: self.total_disputes,
            pass_rate,
        }
    }

    pub fn schedule_verification(&mut self, job: VerificationJob) {
        let job_id = job.job_id;
        self.verifications.insert(job_id, job);
        self.verification_queue.push_back(job_id);
    }

    pub fn next_verification(&mut self) -> Option<VerificationJob> {
        self.verification_queue
            .pop_front()
            .and_then(|id| self.verifications.get(&id).cloned())
    }

    pub fn complete_verification(
        &mut self,
        job_id: &[u8; 32],
        result: VerificationResult,
    ) -> Result<VerificationStatus, VerificationError> {
        let escalate = if !result.matches {
            self.increment_challenge_counter(job_id);
            self.should_escalate(job_id)
        } else {
            false
        };

        let job = self
            .verifications
            .get_mut(job_id)
            .ok_or(VerificationError::JobNotFound)?;

        if escalate {
            job.status = VerificationStatus::Escalated;
        } else if !result.matches {
            job.status = VerificationStatus::Challenged;
        } else {
            job.status = VerificationStatus::Completed;
        }

        job.result = Some(result);
        job.completed_at = Some(current_timestamp());

        Ok(job.status)
    }

    pub fn create_dispute(
        &mut self,
        job_id: [u8; 32],
        challenger_id: [u8; 32],
        accused_operator_id: [u8; 32],
    ) -> Result<Dispute, VerificationError> {
        let job = self
            .verifications
            .get(&job_id)
            .ok_or(VerificationError::JobNotFound)?;

        let evidence = if let Some(ref result) = job.result {
            vec![VerificationEvidence {
                evidence_type: EvidenceType::OutputMismatch,
                data: result.computed_hash.to_vec(),
                timestamp: current_timestamp(),
            }]
        } else {
            vec![]
        };

        let dispute = Dispute::new(job_id, challenger_id, accused_operator_id, evidence);
        self.disputes.insert(dispute.dispute_id, dispute.clone());

        if let Some(job) = self.verifications.get_mut(&job_id) {
            job.status = VerificationStatus::Challenged;
        }

        Ok(dispute)
    }

    pub fn resolve_dispute(
        &mut self,
        dispute_id: &[u8; 32],
        slash_amount: u64,
    ) -> Result<DisputeStatus, VerificationError> {
        let dispute = self
            .disputes
            .get_mut(dispute_id)
            .ok_or(VerificationError::DisputeNotFound)?;

        dispute.status = DisputeStatus::Resolved;
        dispute.resolved_at = Some(current_timestamp());
        dispute.slash_amount = Some(slash_amount);

        if let Some(job) = self.verifications.get_mut(&dispute.job_id) {
            job.status = VerificationStatus::Resolved;
        }

        Ok(dispute.status)
    }

    pub fn reject_dispute(
        &mut self,
        dispute_id: &[u8; 32],
    ) -> Result<DisputeStatus, VerificationError> {
        let dispute = self
            .disputes
            .get_mut(dispute_id)
            .ok_or(VerificationError::DisputeNotFound)?;

        dispute.status = DisputeStatus::Rejected;
        dispute.resolved_at = Some(current_timestamp());

        if let Some(job) = self.verifications.get_mut(&dispute.job_id) {
            job.status = VerificationStatus::Resolved;
        }

        Ok(dispute.status)
    }

    pub fn should_verify(&self) -> bool {
        let rand_val = rand_simple() as f64 / u32::MAX as f64;
        rand_val < self.policy.verification_rate
    }

    fn increment_challenge_counter(&mut self, job_id: &[u8; 32]) {
        *self.challenge_counter.entry(*job_id).or_insert(0) += 1;
    }

    fn should_escalate(&self, job_id: &[u8; 32]) -> bool {
        let count = self.challenge_counter.get(job_id).copied().unwrap_or(0);
        count >= self.policy.escalation_threshold
    }

    pub fn get_verification(&self, job_id: &[u8; 32]) -> Option<&VerificationJob> {
        self.verifications.get(job_id)
    }

    pub fn get_dispute(&self, dispute_id: &[u8; 32]) -> Option<&Dispute> {
        self.disputes.get(dispute_id)
    }

    pub fn get_pending_disputes(&self) -> Vec<&Dispute> {
        self.disputes
            .values()
            .filter(|d| d.status == DisputeStatus::Pending)
            .collect()
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn rand_simple() -> u32 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    nanos.wrapping_mul(1103515245).wrapping_add(12345)
}

pub struct RedundancyManager {
    pub policy: RedundancyPolicy,
    pub max_redundancy: u32,
}

impl RedundancyManager {
    pub fn new(policy: RedundancyPolicy, max_redundancy: u32) -> Self {
        Self {
            policy,
            max_redundancy,
        }
    }

    pub fn determine_k(&self, base_k: u32, qos_class: QoSClass) -> u32 {
        let min_k = match qos_class {
            QoSClass::LowLatency => 1,
            QoSClass::Balanced => 2,
            QoSClass::HighAssurance => 3,
            QoSClass::DeterministicCritical => 3,
        };

        base_k.max(min_k).min(self.max_redundancy)
    }

    pub fn evaluate_redundancy(&self, results: &[(u32, [u8; 32])]) -> RedundancyResult {
        if results.is_empty() {
            return RedundancyResult::Inconclusive;
        }

        match self.policy {
            RedundancyPolicy::Parallel => {
                if results.len() == 1 {
                    return RedundancyResult::SingleResult(results[0].1);
                }
                RedundancyResult::AllResults(results.iter().map(|(_, h)| *h).collect())
            }
            RedundancyPolicy::Sequential => {
                if let Some((_, hash)) = results.first() {
                    RedundancyResult::SingleResult(*hash)
                } else {
                    RedundancyResult::Inconclusive
                }
            }
            RedundancyPolicy::Majority => {
                let mut hash_counts: HashMap<[u8; 32], u32> = HashMap::new();
                for (_, hash) in results {
                    *hash_counts.entry(*hash).or_insert(0) += 1;
                }

                let majority = results.len() / 2 + 1;
                for (hash, count) in hash_counts {
                    if count >= majority as u32 {
                        return RedundancyResult::Majority(hash, count);
                    }
                }

                RedundancyResult::Split(results.len())
            }
            RedundancyPolicy::FirstValid => {
                if let Some((_, hash)) = results.first() {
                    RedundancyResult::SingleResult(*hash)
                } else {
                    RedundancyResult::Inconclusive
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QoSClass {
    LowLatency,
    Balanced,
    HighAssurance,
    DeterministicCritical,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RedundancyResult {
    SingleResult([u8; 32]),
    Majority([u8; 32], u32),
    AllResults(Vec<[u8; 32]>),
    Split(usize),
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationError {
    JobNotFound,
    DisputeNotFound,
    InvalidState,
    InsufficientEvidence,
}

/// Metrics from verification engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationMetrics {
    pub total_sampled: u64,
    pub total_passed: u64,
    pub total_failed: u64,
    pub total_disputes: u64,
    pub pass_rate: f64,
}

/// Async version of VerificationEngine with thread-safe state
pub struct AsyncVerificationEngine {
    inner: Arc<RwLock<VerificationEngine>>,
}

impl AsyncVerificationEngine {
    pub fn new(policy: VerificationPolicy) -> Self {
        Self {
            inner: Arc::new(RwLock::new(VerificationEngine::new(policy))),
        }
    }

    pub fn arc(&self) -> Arc<RwLock<VerificationEngine>> {
        self.inner.clone()
    }

    /// Sample a job for verification (async)
    pub async fn sample_job(
        &self,
        job_id: [u8; 32],
        operator_id: [u8; 32],
        input_hash: [u8; 32],
        output_hash: [u8; 32],
    ) -> bool {
        self.inner.write().await.sample_job(job_id, operator_id, input_hash, output_hash)
    }

    /// Record verification result (async)
    pub async fn record_result(
        &self,
        job_id: &[u8; 32],
        result: VerificationResult,
    ) -> Result<VerificationStatus, VerificationError> {
        self.inner.write().await.record_verification_result(job_id, result)
    }

    /// Create dispute (async)
    pub async fn create_dispute(
        &self,
        job_id: [u8; 32],
        challenger_id: [u8; 32],
        accused_operator_id: [u8; 32],
    ) -> Result<Dispute, VerificationError> {
        self.inner.write().await.create_dispute(job_id, challenger_id, accused_operator_id)
    }

    /// Get metrics (async)
    pub async fn get_metrics(&self) -> VerificationMetrics {
        self.inner.read().await.get_metrics()
    }

    /// Get pending verifications (async)
    pub async fn get_pending(&self) -> Option<VerificationJob> {
        self.inner.write().await.next_verification()
    }
}

impl Default for AsyncVerificationEngine {
    fn default() -> Self {
        Self::new(VerificationPolicy::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_policy_default() {
        let policy = VerificationPolicy::default();
        assert_eq!(policy.verification_rate, 0.1);
        assert_eq!(policy.redundancy_factor, 1);
    }

    #[test]
    fn test_schedule_verification() {
        let policy = VerificationPolicy::default();
        let mut engine = VerificationEngine::new(policy);

        let job = VerificationJob {
            job_id: [1u8; 32],
            operator_id: [2u8; 32],
            input_hash: [3u8; 32],
            output_hash: [4u8; 32],
            expected_hash: Some([4u8; 32]),
            verification_mode: VerificationMode::Deterministic,
            status: VerificationStatus::Pending,
            created_at: 100,
            completed_at: None,
            result: None,
        };

        engine.schedule_verification(job);

        let next = engine.next_verification();
        assert!(next.is_some());
    }

    #[test]
    fn test_complete_verification_matching() {
        let policy = VerificationPolicy::default();
        let mut engine = VerificationEngine::new(policy);

        let job = VerificationJob {
            job_id: [1u8; 32],
            operator_id: [2u8; 32],
            input_hash: [3u8; 32],
            output_hash: [4u8; 32],
            expected_hash: Some([4u8; 32]),
            verification_mode: VerificationMode::Deterministic,
            status: VerificationStatus::Pending,
            created_at: 100,
            completed_at: None,
            result: None,
        };

        engine.schedule_verification(job);

        let result = VerificationResult {
            matches: true,
            computed_hash: [4u8; 32],
            execution_time_ms: 100,
            verifier_id: [5u8; 32],
            confidence: 1.0,
            evidence: vec![],
        };

        let status = engine.complete_verification(&[1u8; 32], result).unwrap();
        assert_eq!(status, VerificationStatus::Completed);
    }

    #[test]
    fn test_complete_verification_not_matching() {
        let policy = VerificationPolicy::default();
        let mut engine = VerificationEngine::new(policy);

        let job = VerificationJob {
            job_id: [1u8; 32],
            operator_id: [2u8; 32],
            input_hash: [3u8; 32],
            output_hash: [4u8; 32],
            expected_hash: Some([4u8; 32]),
            verification_mode: VerificationMode::Deterministic,
            status: VerificationStatus::Pending,
            created_at: 100,
            completed_at: None,
            result: None,
        };

        engine.schedule_verification(job);

        let result = VerificationResult {
            matches: false,
            computed_hash: [5u8; 32],
            execution_time_ms: 100,
            verifier_id: [6u8; 32],
            confidence: 1.0,
            evidence: vec![VerificationEvidence {
                evidence_type: EvidenceType::OutputMismatch,
                data: vec![],
                timestamp: 100,
            }],
        };

        let status = engine.complete_verification(&[1u8; 32], result).unwrap();
        assert_eq!(status, VerificationStatus::Challenged);
    }

    #[test]
    fn test_create_dispute() {
        let policy = VerificationPolicy::default();
        let mut engine = VerificationEngine::new(policy);

        let job = VerificationJob {
            job_id: [1u8; 32],
            operator_id: [2u8; 32],
            input_hash: [3u8; 32],
            output_hash: [4u8; 32],
            expected_hash: Some([5u8; 32]),
            verification_mode: VerificationMode::Deterministic,
            status: VerificationStatus::Completed,
            created_at: 100,
            completed_at: Some(200),
            result: Some(VerificationResult {
                matches: false,
                computed_hash: [5u8; 32],
                execution_time_ms: 100,
                verifier_id: [6u8; 32],
                confidence: 1.0,
                evidence: vec![],
            }),
        };

        engine.schedule_verification(job);

        let dispute = engine
            .create_dispute([1u8; 32], [7u8; 32], [2u8; 32])
            .unwrap();

        assert_eq!(dispute.job_id, [1u8; 32]);
        assert_eq!(dispute.challenger_id, [7u8; 32]);
        assert_eq!(dispute.accused_operator_id, [2u8; 32]);
    }

    #[test]
    fn test_resolve_dispute() {
        let policy = VerificationPolicy::default();
        let mut engine = VerificationEngine::new(policy);

        let job = VerificationJob {
            job_id: [1u8; 32],
            operator_id: [2u8; 32],
            input_hash: [3u8; 32],
            output_hash: [4u8; 32],
            expected_hash: Some([5u8; 32]),
            verification_mode: VerificationMode::Deterministic,
            status: VerificationStatus::Completed,
            created_at: 100,
            completed_at: Some(200),
            result: Some(VerificationResult {
                matches: false,
                computed_hash: [5u8; 32],
                execution_time_ms: 100,
                verifier_id: [6u8; 32],
                confidence: 1.0,
                evidence: vec![],
            }),
        };

        engine.schedule_verification(job);
        let dispute = engine
            .create_dispute([1u8; 32], [7u8; 32], [2u8; 32])
            .unwrap();

        let status = engine.resolve_dispute(&dispute.dispute_id, 500).unwrap();
        assert_eq!(status, DisputeStatus::Resolved);
    }

    #[test]
    fn test_redundancy_manager_parallel() {
        let manager = RedundancyManager::new(RedundancyPolicy::Parallel, 5);

        let results = vec![(1, [1u8; 32]), (2, [1u8; 32])];

        let result = manager.evaluate_redundancy(&results);
        assert_eq!(
            result,
            RedundancyResult::AllResults(vec![[1u8; 32], [1u8; 32]])
        );
    }

    #[test]
    fn test_redundancy_manager_majority() {
        let manager = RedundancyManager::new(RedundancyPolicy::Majority, 5);

        let results = vec![(1, [1u8; 32]), (2, [1u8; 32]), (3, [2u8; 32])];

        let result = manager.evaluate_redundancy(&results);
        assert_eq!(result, RedundancyResult::Majority([1u8; 32], 2));
    }

    #[test]
    fn test_redundancy_manager_determine_k() {
        let manager = RedundancyManager::new(RedundancyPolicy::Parallel, 5);

        assert_eq!(manager.determine_k(1, QoSClass::LowLatency), 1);
        assert_eq!(manager.determine_k(1, QoSClass::Balanced), 2);
        assert_eq!(manager.determine_k(1, QoSClass::HighAssurance), 3);
        assert_eq!(manager.determine_k(1, QoSClass::DeterministicCritical), 3);
    }
}
