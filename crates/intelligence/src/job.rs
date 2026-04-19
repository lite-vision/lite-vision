use serde::{Deserialize, Serialize};

pub use crate::kernel::{KernelExecutionContext, KernelExecutor, KernelOutput, KernelSpec};
pub use crate::verification::{
    VerificationEngine, VerificationJob, VerificationMode, VerificationPolicy as VPolicy,
    VerificationResult, VerificationStatus,
};

use crate::kernel::KernelRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobTicket {
    pub job_id: [u8; 32],
    pub client_id: [u8; 32],
    pub kernel_id: [u8; 32],
    pub input_hash: [u8; 32],
    pub execution_mode: ExecutionMode,
    pub budget: Budget,
    pub deadline: u64,
    pub qos_class: QoSClass,
    pub verification_policy: VerificationPolicy,
    pub max_retries: u32,
    pub partial_allowed: bool,
    pub cancellation_policy: CancellationPolicy,
    pub creation_block_height: u64,
    pub signature: Vec<u8>,
}

impl JobTicket {
    pub fn new(
        client_id: [u8; 32],
        kernel_id: [u8; 32],
        input_hash: [u8; 32],
        budget: Budget,
        deadline: u64,
    ) -> Self {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&client_id);
        hasher.update(&kernel_id);
        hasher.update(&input_hash);
        hasher.update(&budget.to_bytes());
        hasher.update(&deadline.to_le_bytes());

        Self {
            job_id: *hasher.finalize().as_bytes(),
            client_id,
            kernel_id,
            input_hash,
            execution_mode: ExecutionMode::Deterministic,
            budget,
            deadline,
            qos_class: QoSClass::Balanced,
            verification_policy: VerificationPolicy::default(),
            max_retries: 3,
            partial_allowed: false,
            cancellation_policy: CancellationPolicy::default(),
            creation_block_height: 0,
            signature: Vec::new(),
        }
    }

    pub fn domain_separator() -> &'static [u8] {
        b"LITE-VISION-JOB-TICKET-v1"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    Deterministic,
    Soft,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Budget {
    pub max_total_fee: u64,
    pub max_gpu_cycles: u64,
    pub max_cpu_cycles: u64,
    pub max_memory_bytes: u64,
    pub max_output_size: u64,
    pub verifier_reserve: u64,
}

impl Budget {
    pub fn new(max_total_fee: u64) -> Self {
        Self {
            max_total_fee,
            max_gpu_cycles: 1_000_000_000,
            max_cpu_cycles: 100_000_000,
            max_memory_bytes: 8_589_934_592,
            max_output_size: 1_073_741_824,
            verifier_reserve: max_total_fee / 10,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.max_total_fee.to_le_bytes());
        bytes.extend_from_slice(&self.max_gpu_cycles.to_le_bytes());
        bytes.extend_from_slice(&self.max_cpu_cycles.to_le_bytes());
        bytes.extend_from_slice(&self.max_memory_bytes.to_le_bytes());
        bytes.extend_from_slice(&self.max_output_size.to_le_bytes());
        bytes.extend_from_slice(&self.verifier_reserve.to_le_bytes());
        bytes
    }

    pub fn can_cover(&self, cost: &ExecutionCost) -> bool {
        cost.total_fee <= self.max_total_fee
            && cost.gpu_cycles <= self.max_gpu_cycles
            && cost.cpu_cycles <= self.max_cpu_cycles
            && cost.memory_bytes <= self.max_memory_bytes
            && cost.output_size <= self.max_output_size
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCost {
    pub total_fee: u64,
    pub gpu_cycles: u64,
    pub cpu_cycles: u64,
    pub memory_bytes: u64,
    pub output_size: u64,
}

impl ExecutionCost {
    pub fn new() -> Self {
        Self {
            total_fee: 0,
            gpu_cycles: 0,
            cpu_cycles: 0,
            memory_bytes: 0,
            output_size: 0,
        }
    }

    pub fn calculate_fee(&self, price_per_cycle: u64) -> u64 {
        self.gpu_cycles * price_per_cycle + self.cpu_cycles * (price_per_cycle / 10)
    }
}

impl Default for ExecutionCost {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QoSClass {
    LowLatency,
    Balanced,
    HighAssurance,
    DeterministicCritical,
}

impl Default for QoSClass {
    fn default() -> Self {
        QoSClass::Balanced
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationPolicy {
    pub verification_rate: f64,
    pub redundancy_factor: u32,
    pub escalation_threshold: u32,
    pub challenge_window_blocks: u32,
}

impl Default for VerificationPolicy {
    fn default() -> Self {
        Self {
            verification_rate: 0.1,
            redundancy_factor: 1,
            escalation_threshold: 2,
            challenge_window_blocks: 100,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CancellationPolicy {
    Immediate,
    AfterDeadline,
    Never,
}

impl Default for CancellationPolicy {
    fn default() -> Self {
        CancellationPolicy::AfterDeadline
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub ticket: JobTicket,
    pub status: JobStatus,
    pub assigned_operator: Option<[u8; 32]>,
    pub result: Option<JobResult>,
    pub execution_cost: Option<ExecutionCost>,
    pub retries_remaining: u32,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Pending,
    Assigned,
    Executing,
    Completed,
    Failed,
    Expired,
    Disputed,
    Cancelled,
}

impl Job {
    pub fn from_ticket(ticket: JobTicket, created_at: u64) -> Self {
        Self {
            ticket,
            status: JobStatus::Pending,
            assigned_operator: None,
            result: None,
            execution_cost: None,
            retries_remaining: 0,
            created_at,
            started_at: None,
            completed_at: None,
        }
    }

    pub fn assign(&mut self, operator_id: [u8; 32], current_block: u64) {
        self.status = JobStatus::Assigned;
        self.assigned_operator = Some(operator_id);
        self.started_at = Some(current_block);
        self.retries_remaining = self.ticket.max_retries;
    }

    pub fn start_execution(&mut self) {
        self.status = JobStatus::Executing;
    }

    pub fn complete(&mut self, result: JobResult, cost: ExecutionCost, completed_at: u64) {
        self.status = JobStatus::Completed;
        self.result = Some(result);
        self.execution_cost = Some(cost);
        self.completed_at = Some(completed_at);
    }

    pub fn fail(&mut self) {
        if self.retries_remaining > 0 {
            self.retries_remaining -= 1;
            if self.retries_remaining == 0 {
                self.status = JobStatus::Failed;
            } else {
                self.status = JobStatus::Pending;
            }
            self.assigned_operator = None;
        }
    }

    pub fn expire(&mut self) {
        self.status = JobStatus::Expired;
    }

    pub fn dispute(&mut self) {
        self.status = JobStatus::Disputed;
    }

    pub fn cancel(&mut self) {
        if self.ticket.cancellation_policy == CancellationPolicy::Immediate
            && matches!(self.status, JobStatus::Pending | JobStatus::Assigned)
        {
            self.status = JobStatus::Cancelled;
        }
    }

    pub fn is_expired(&self, current_block: u64) -> bool {
        current_block > self.ticket.deadline
            && matches!(
                self.status,
                JobStatus::Pending | JobStatus::Assigned | JobStatus::Executing
            )
    }

    pub fn can_retry(&self) -> bool {
        self.retries_remaining > 0 && matches!(self.status, JobStatus::Failed | JobStatus::Expired)
    }

    pub fn refund_amount(&self) -> u64 {
        let used = self
            .execution_cost
            .as_ref()
            .map(|c| c.total_fee)
            .unwrap_or(0);
        self.ticket.budget.max_total_fee.saturating_sub(used)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub output_hash: [u8; 32],
    pub output_size: u64,
    pub compute_used: ExecutionCost,
    pub execution_time_ms: u64,
    pub signature: Vec<u8>,
    pub partial: bool,
}

impl JobResult {
    pub fn new(output_hash: [u8; 32], compute_used: ExecutionCost) -> Self {
        Self {
            output_hash,
            output_size: 0,
            compute_used,
            execution_time_ms: 0,
            signature: Vec::new(),
            partial: false,
        }
    }
}

use std::sync::Arc;
use tokio::sync::RwLock;

pub struct JobExecutor {
    inner: Arc<RwLock<JobExecutorInner>>,
}

struct JobExecutorInner {
    pub jobs: std::collections::HashMap<[u8; 32], Job>,
    pub receipts: std::collections::HashMap<[u8; 32], crate::receipts::Receipt>,
    kernel_executor: Option<KernelExecutor>,
    kernel_registry: KernelRegistry,
    pub verification_engine: Option<VerificationEngine>,
    capacity: usize,
}

impl JobExecutor {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(JobExecutorInner {
                jobs: std::collections::HashMap::new(),
                receipts: std::collections::HashMap::new(),
                kernel_executor: None,
                kernel_registry: KernelRegistry::new(),
                verification_engine: None,
                capacity,
            })),
        }
    }

    /// Create JobExecutor with a verification engine
    pub fn with_verification(capacity: usize, policy: crate::verification::VerificationPolicy) -> Self {
        Self {
            inner: Arc::new(RwLock::new(JobExecutorInner {
                jobs: std::collections::HashMap::new(),
                receipts: std::collections::HashMap::new(),
                kernel_executor: None,
                kernel_registry: KernelRegistry::new(),
                verification_engine: Some(VerificationEngine::new(policy)),
                capacity,
            })),
        }
    }

    pub async fn submit(&self, job: Job) -> Result<crate::receipts::Receipt, JobError> {
        let mut inner = self.inner.write().await;
        if inner.jobs.len() >= inner.capacity {
            return Err(JobError::CapacityExceeded);
        }

        let job_id = job.ticket.job_id;
        inner.jobs.insert(job_id, job.clone());

        let execution_mode = match job.ticket.execution_mode {
            ExecutionMode::Soft => crate::receipts::ExecutionMode::Soft,
            ExecutionMode::Deterministic => crate::receipts::ExecutionMode::Deterministic,
        };

        // Create initial receipt using receipts::Receipt::new
        let receipt = crate::receipts::Receipt::new(
            job_id,
            [0u8; 32], // Operator to be assigned
            job.ticket.kernel_id,
            (1, 0, 0), // Kernel version placeholder
            job.ticket.input_hash,
            [0u8; 32], // Output to be computed
            &crate::receipts::ResourceUsage::default(),
            execution_mode,
            0,
            0,
        );

        inner.receipts.insert(job_id, receipt.clone());
        Ok(receipt)
    }

    pub async fn submit_job(&self, ticket: JobTicket, block_height: u64) -> Result<[u8; 32], JobError> {
        let job = Job::from_ticket(ticket, block_height);
        let job_id = job.ticket.job_id;
        self.submit(job).await.map(|_| job_id)
    }

    pub async fn get_job(&self, job_id: [u8; 32]) -> Option<Job> {
        let inner = self.inner.read().await;
        inner.jobs.get(&job_id).cloned()
    }

    pub async fn get_receipt(&self, job_id: [u8; 32]) -> Option<crate::receipts::Receipt> {
        let inner = self.inner.read().await;
        inner.receipts.get(&job_id).cloned()
    }

    pub async fn assign_job(&self, job_id: &[u8; 32], operator_id: [u8; 32], block_height: u64) -> Result<(), JobError> {
        let mut inner = self.inner.write().await;
        inner.assign_job(job_id, operator_id, block_height)
    }

    pub async fn complete_job(&self, job_id: &[u8; 32], result: JobResult, block_height: u64) -> Result<u64, JobError> {
        let mut inner = self.inner.write().await;
        inner.complete_job(job_id, result, block_height)
    }

    pub async fn get_pending_jobs(&self) -> Vec<Job> {
        let inner = self.inner.read().await;
        inner.get_pending_jobs()
    }

    pub async fn get_jobs_by_operator(&self, operator_id: &[u8; 32]) -> Vec<Job> {
        let inner = self.inner.read().await;
        inner.get_jobs_by_operator(operator_id)
    }

    pub async fn get_jobs_by_client(&self, client_id: &[u8; 32]) -> Vec<Job> {
        let inner = self.inner.read().await;
        inner.get_jobs_by_client(client_id)
    }
}

impl JobExecutorInner {
    /// Schedule a job for verification sampling
    pub fn schedule_verification(
        &mut self,
        job_id: [u8; 32],
        operator_id: [u8; 32],
        input_hash: [u8; 32],
        output_hash: [u8; 32],
    ) -> Result<(), JobError> {
        let job = self.jobs.get(&job_id).ok_or(JobError::JobNotFound)?;

        // Check if verification engine is available
        if let Some(ref mut v_engine) = self.verification_engine {
            // Convert job verification policy to verification engine policy
            let _v_policy = crate::verification::VerificationPolicy {
                mode: crate::verification::VerificationMode::Probabilistic,
                redundancy_factor: job.ticket.verification_policy.redundancy_factor,
                verification_rate: job.ticket.verification_policy.verification_rate,
                escalation_threshold: job.ticket.verification_policy.escalation_threshold,
                challenge_window_blocks: job.ticket.verification_policy.challenge_window_blocks,
                sampling_strategy: crate::verification::SamplingStrategy::Random,
            };

            let v_job = VerificationJob {
                job_id,
                operator_id,
                input_hash,
                output_hash,
                expected_hash: None,
                verification_mode: VerificationMode::Probabilistic,
                status: VerificationStatus::Pending,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                completed_at: None,
                result: None,
            };

            v_engine.schedule_verification(v_job);
        }

        Ok(())
    }

    /// Complete a verification and get result
    pub fn complete_verification(
        &mut self,
        job_id: &[u8; 32],
        result: VerificationResult,
    ) -> Result<VerificationStatus, crate::verification::VerificationError> {
        if let Some(ref mut v_engine) = self.verification_engine {
            v_engine.complete_verification(job_id, result)
        } else {
            Err(crate::verification::VerificationError::JobNotFound)
        }
    }

    /// Check if job should be verified based on verification policy
    pub fn should_verify(&self, _job_id: &[u8; 32]) -> bool {
        if let Some(ref v_engine) = self.verification_engine {
            v_engine.should_verify()
        } else {
            false
        }
    }

    /// Get verification result for a job
    pub fn get_verification_result(&self, job_id: &[u8; 32]) -> Option<&VerificationResult> {
        if let Some(ref v_engine) = self.verification_engine {
            v_engine
                .get_verification(job_id)
                .and_then(|v| v.result.as_ref())
        } else {
            None
        }
    }

    /// Trigger dispute for a job (called when verification fails)
    pub fn trigger_dispute(
        &mut self,
        job_id: [u8; 32],
        challenger_id: [u8; 32],
    ) -> Result<crate::verification::Dispute, crate::verification::VerificationError> {
        if let Some(ref mut v_engine) = self.verification_engine {
            if let Some(job) = self.jobs.get(&job_id) {
                let operator_id = job.assigned_operator.unwrap_or([0u8; 32]);
                return v_engine.create_dispute(job_id, challenger_id, operator_id);
            }
        }
        Err(crate::verification::VerificationError::JobNotFound)
    }

    /// Execute a job using the kernel executor
    pub fn execute_job(
        &mut self,
        job_id: &[u8; 32],
        input_data: Vec<u8>,
        operator_id: [u8; 32],
        block_height: u64,
    ) -> Result<KernelOutput, JobExecutionError> {
        // Get job status first, check if executable
        let is_executable = {
            let job = self
                .jobs
                .get(job_id)
                .ok_or(JobExecutionError::JobNotFound)?;
            matches!(job.status, JobStatus::Assigned | JobStatus::Executing)
        };

        if !is_executable {
            return Err(JobExecutionError::InvalidState);
        }

        // Get required data from job (copy needed data to avoid borrow conflicts)
        let (kernel_id, budget) = {
            let job = self
                .jobs
                .get(job_id)
                .ok_or(JobExecutionError::JobNotFound)?;
            (job.ticket.kernel_id, job.ticket.budget)
        };

        // Get the kernel spec
        let spec = self.get_kernel_spec(&kernel_id)?;

        // Create execution context
        let ctx = KernelExecutionContext::new(*job_id, operator_id, block_height)
            .with_budget(budget.max_gpu_cycles)
            .with_memory_limit(budget.max_memory_bytes);

        // Create or reuse kernel executor with context
        let mut executor = KernelExecutor::new().with_context(ctx);

        // Execute the kernel
        let output = executor
            .execute(&spec, input_data)
            .map_err(JobExecutionError::KernelError)?;

        // Update job status to executing (now we have mutable access)
        if let Some(job) = self.jobs.get_mut(job_id) {
            job.start_execution();
        }

        Ok(output)
    }

    /// Get kernel spec from registry
    pub fn get_kernel_spec(&self, _kernel_id: &[u8; 32]) -> Result<KernelSpec, JobExecutionError> {
        // For now, create a default spec if not found in registry
        // In production, this would query the registry
        Ok(KernelSpec::new(
            "default".to_string(),
            1,
            1_000_000_000,
            8_589_934_592,
            true,
        ))
    }

    /// Register a kernel for job execution
    pub fn register_kernel<K: super::kernel::Kernel + 'static>(&mut self, kernel: K) {
        self.kernel_registry.register(kernel);
    }

    /// Check if kernel is registered
    pub fn has_kernel(&self, kernel_id: &[u8; 32]) -> bool {
        self.kernel_registry.has_kernel(kernel_id)
    }

    pub fn submit_job(
        &mut self,
        ticket: JobTicket,
        current_block: u64,
    ) -> Result<[u8; 32], JobError> {
        if self.jobs.contains_key(&ticket.job_id) {
            return Err(JobError::JobAlreadyExists);
        }

        let job = Job::from_ticket(ticket, current_block);
        let job_id = job.ticket.job_id;
        self.jobs.insert(job_id, job);

        Ok(job_id)
    }

    pub fn get_job(&self, job_id: &[u8; 32]) -> Option<&Job> {
        self.jobs.get(job_id)
    }

    pub fn get_job_mut(&mut self, job_id: &[u8; 32]) -> Option<&mut Job> {
        self.jobs.get_mut(job_id)
    }

    pub fn assign_job(
        &mut self,
        job_id: &[u8; 32],
        operator_id: [u8; 32],
        current_block: u64,
    ) -> Result<(), JobError> {
        let job = self.jobs.get_mut(job_id).ok_or(JobError::JobNotFound)?;

        if job.status != JobStatus::Pending {
            return Err(JobError::InvalidStateTransition);
        }

        job.assign(operator_id, current_block);
        Ok(())
    }

    pub fn complete_job(
        &mut self,
        job_id: &[u8; 32],
        result: JobResult,
        current_block: u64,
    ) -> Result<u64, JobError> {
        let job = self.jobs.get_mut(job_id).ok_or(JobError::JobNotFound)?;

        if !matches!(job.status, JobStatus::Assigned | JobStatus::Executing) {
            return Err(JobError::InvalidStateTransition);
        }

        if !job.ticket.budget.can_cover(&result.compute_used) {
            return Err(JobError::BudgetExceeded);
        }

        let refund = job.ticket.budget.max_total_fee - result.compute_used.total_fee;
        let cost = result.compute_used.clone();
        job.complete(result, cost, current_block);

        Ok(refund)
    }

    pub fn fail_job(&mut self, job_id: &[u8; 32]) -> Result<(), JobError> {
        let job = self.jobs.get_mut(job_id).ok_or(JobError::JobNotFound)?;
        job.fail();
        Ok(())
    }

    pub fn expire_jobs(&mut self, current_block: u64) -> Vec<[u8; 32]> {
        let mut expired = Vec::new();

        for (job_id, job) in self.jobs.iter_mut() {
            if job.is_expired(current_block) {
                job.expire();
                expired.push(*job_id);
            }
        }

        expired
    }

    pub fn get_pending_jobs(&self) -> Vec<Job> {
        self.jobs
            .values()
            .filter(|j| matches!(j.status, JobStatus::Pending))
            .cloned()
            .collect()
    }

    pub fn get_jobs_by_operator(&self, operator_id: &[u8; 32]) -> Vec<Job> {
        self.jobs
            .values()
            .filter(|j| j.assigned_operator == Some(*operator_id))
            .cloned()
            .collect()
    }


    pub fn get_jobs_by_client(&self, client_id: &[u8; 32]) -> Vec<Job> {
        self.jobs
            .values()
            .filter(|j| j.ticket.client_id == *client_id)
            .cloned()
            .collect()
    }
}

impl Default for JobExecutor {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobError {
    JobNotFound,
    JobAlreadyExists,
    InvalidStateTransition,
    BudgetExceeded,
    DeadlineExceeded,
    OperatorNotFound,
    CapacityExceeded,
    JobExecutionFailed(String),
}

/// Errors that can occur during job execution
#[derive(Debug, Clone)]
pub enum JobExecutionError {
    JobNotFound,
    KernelError(String),
    InvalidState,
    InsufficientBudget,
}

impl std::fmt::Display for JobExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobExecutionError::JobNotFound => write!(f, "Job not found"),
            JobExecutionError::KernelError(msg) => write!(f, "Kernel error: {}", msg),
            JobExecutionError::InvalidState => write!(f, "Invalid job state for execution"),
            JobExecutionError::InsufficientBudget => write!(f, "Insufficient budget"),
        }
    }
}

impl std::error::Error for JobExecutionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_ticket_creation() {
        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

        assert_eq!(ticket.client_id, [1u8; 32]);
        assert_eq!(ticket.kernel_id, [2u8; 32]);
        assert_eq!(ticket.execution_mode, ExecutionMode::Deterministic);
        assert_eq!(ticket.max_retries, 3);
    }

    #[test]
    fn test_job_ticket_job_id_unique() {
        let ticket1 = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

        let ticket2 = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1001), 100);

        assert_ne!(ticket1.job_id, ticket2.job_id);
    }

    #[test]
    fn test_budget_can_cover() {
        let budget = Budget::new(1000);
        let cost = ExecutionCost {
            total_fee: 500,
            gpu_cycles: 500_000_000,
            cpu_cycles: 50_000_000,
            memory_bytes: 4_000_000_000,
            output_size: 500_000_000,
        };

        assert!(budget.can_cover(&cost));
    }

    #[test]
    fn test_budget_cannot_cover() {
        let budget = Budget::new(1000);
        let cost = ExecutionCost {
            total_fee: 1500,
            gpu_cycles: 500_000_000,
            cpu_cycles: 50_000_000,
            memory_bytes: 4_000_000_000,
            output_size: 500_000_000,
        };

        assert!(!budget.can_cover(&cost));
    }

    #[test]
    fn test_job_assignment() {
        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

        let mut job = Job::from_ticket(ticket, 10);

        assert_eq!(job.status, JobStatus::Pending);

        job.assign([5u8; 32], 15);

        assert_eq!(job.status, JobStatus::Assigned);
        assert_eq!(job.assigned_operator, Some([5u8; 32]));
        assert_eq!(job.retries_remaining, 3);
    }

    #[test]
    fn test_job_completion() {
        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

        let mut job = Job::from_ticket(ticket, 10);
        job.assign([5u8; 32], 15);

        let cost = ExecutionCost {
            total_fee: 600,
            gpu_cycles: 600_000_000,
            cpu_cycles: 60_000_000,
            memory_bytes: 5_000_000_000,
            output_size: 600_000_000,
        };

        let result = JobResult::new([7u8; 32], cost.clone());

        job.complete(result, cost, 20);

        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.refund_amount(), 400);
    }

    #[test]
    fn test_job_retry() {
        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

        let mut job = Job::from_ticket(ticket, 10);
        job.assign([5u8; 32], 15);

        assert_eq!(job.retries_remaining, 3);

        job.fail();

        assert_eq!(job.status, JobStatus::Pending);
        assert_eq!(job.retries_remaining, 2);
    }

    #[test]
    fn test_job_retry_exhausted() {
        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

        let mut job = Job::from_ticket(ticket, 10);
        job.assign([5u8; 32], 15);

        job.fail();
        job.fail();
        job.fail();

        assert_eq!(job.status, JobStatus::Failed);
    }

    #[test]
    fn test_job_expiration() {
        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

        let mut job = Job::from_ticket(ticket, 10);
        job.assign([5u8; 32], 15);

        assert!(!job.is_expired(50));
        assert!(!job.is_expired(100));

        job.expire();

        assert_eq!(job.status, JobStatus::Expired);
    }

    #[tokio::test]
    async fn test_job_executor_submit() {
        let executor = JobExecutor::new(100);

        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

        let receipt = executor.submit(Job::from_ticket(ticket, 10)).await.unwrap();
        let job_id = receipt.job_id;

        let job = executor.get_job(job_id).await.unwrap();
        assert_eq!(job.status, JobStatus::Pending);
    }

    #[tokio::test]
    async fn test_job_executor_complete() {
        let executor = JobExecutor::new(100);

        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);
        let job = Job::from_ticket(ticket, 10);
        let job_id = job.ticket.job_id;

        executor.submit(job).await.unwrap();

        executor.assign_job(&job_id, [5u8; 32], 15).await.unwrap();

        let cost = ExecutionCost {
            total_fee: 600,
            gpu_cycles: 600_000_000,
            cpu_cycles: 60_000_000,
            memory_bytes: 5_000_000_000,
            output_size: 600_000_000,
        };

        let result = JobResult::new([7u8; 32], cost.clone());

        let refund = executor.complete_job(&job_id, result, 20).await.unwrap();

        assert_eq!(refund, 400);

        let job = executor.get_job(job_id).await.unwrap();
        assert_eq!(job.status, JobStatus::Completed);
    }

    #[tokio::test]
    async fn test_job_executor_budget_exceeded() {
        let executor = JobExecutor::new(100);

        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);
        let job = Job::from_ticket(ticket, 10);
        let job_id = job.ticket.job_id;

        executor.submit(job).await.unwrap();

        executor.assign_job(&job_id, [5u8; 32], 15).await.unwrap();

        let cost = ExecutionCost {
            total_fee: 1500,
            gpu_cycles: 600_000_000,
            cpu_cycles: 60_000_000,
            memory_bytes: 5_000_000_000,
            output_size: 600_000_000,
        };

        let result = JobResult::new([7u8; 32], cost.clone());

        let result_err = executor.complete_job(&job_id, result, 20).await;

        assert!(matches!(result_err, Err(JobError::BudgetExceeded)));
    }

    #[tokio::test]
    async fn test_get_pending_jobs() {
        let executor = JobExecutor::new(100);

        let ticket1 = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);
        let ticket2 = JobTicket::new([1u8; 32], [3u8; 32], [4u8; 32], Budget::new(1000), 100);

        executor.submit(Job::from_ticket(ticket1, 10)).await.unwrap();
        executor.submit(Job::from_ticket(ticket2, 10)).await.unwrap();

        let pending = executor.get_pending_jobs().await;

        assert_eq!(pending.len(), 2);
    }

    #[tokio::test]
    async fn test_get_jobs_by_operator() {
        let executor = JobExecutor::new(100);

        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);
        let job = Job::from_ticket(ticket, 10);
        let job_id = job.ticket.job_id;

        executor.submit(job).await.unwrap();

        executor.assign_job(&job_id, [5u8; 32], 15).await.unwrap();

        let jobs = executor.get_jobs_by_operator(&[5u8; 32]).await;

        assert_eq!(jobs.len(), 1);
    }

    #[test]
    fn test_cancel_policy() {
        let mut ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);
        ticket.cancellation_policy = CancellationPolicy::Immediate;

        let mut job = Job::from_ticket(ticket, 10);

        job.cancel();

        assert_eq!(job.status, JobStatus::Cancelled);
    }

    #[test]
    fn test_cancel_not_allowed() {
        let mut ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);
        ticket.cancellation_policy = CancellationPolicy::Never;

        let mut job = Job::from_ticket(ticket, 10);

        job.cancel();

        assert_eq!(job.status, JobStatus::Pending);
    }
}
