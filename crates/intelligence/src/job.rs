use serde::{Deserialize, Serialize};

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

pub struct JobExecutor {
    pub jobs: std::collections::HashMap<[u8; 32], Job>,
}

impl JobExecutor {
    pub fn new() -> Self {
        Self {
            jobs: std::collections::HashMap::new(),
        }
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

    pub fn get_pending_jobs(&self) -> Vec<&Job> {
        self.jobs
            .values()
            .filter(|j| j.status == JobStatus::Pending)
            .collect()
    }

    pub fn get_jobs_by_operator(&self, operator_id: &[u8; 32]) -> Vec<&Job> {
        self.jobs
            .values()
            .filter(|j| j.assigned_operator == Some(*operator_id))
            .collect()
    }

    pub fn get_jobs_by_client(&self, client_id: &[u8; 32]) -> Vec<&Job> {
        self.jobs
            .values()
            .filter(|j| j.ticket.client_id == *client_id)
            .collect()
    }
}

impl Default for JobExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobError {
    JobNotFound,
    JobAlreadyExists,
    InvalidStateTransition,
    BudgetExceeded,
    DeadlineExceeded,
    OperatorNotFound,
}

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

    #[test]
    fn test_job_executor_submit() {
        let mut executor = JobExecutor::new();

        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

        let job_id = executor.submit_job(ticket, 10).unwrap();

        let job = executor.get_job(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Pending);
    }

    #[test]
    fn test_job_executor_complete() {
        let mut executor = JobExecutor::new();

        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

        let job_id = executor.submit_job(ticket.clone(), 10).unwrap();

        executor.assign_job(&job_id, [5u8; 32], 15).unwrap();

        let cost = ExecutionCost {
            total_fee: 600,
            gpu_cycles: 600_000_000,
            cpu_cycles: 60_000_000,
            memory_bytes: 5_000_000_000,
            output_size: 600_000_000,
        };

        let result = JobResult::new([7u8; 32], cost);

        let refund = executor.complete_job(&job_id, result, 20).unwrap();

        assert_eq!(refund, 400);

        let job = executor.get_job(&job_id).unwrap();
        assert_eq!(job.status, JobStatus::Completed);
    }

    #[test]
    fn test_job_executor_budget_exceeded() {
        let mut executor = JobExecutor::new();

        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

        let job_id = executor.submit_job(ticket, 10).unwrap();

        executor.assign_job(&job_id, [5u8; 32], 15).unwrap();

        let cost = ExecutionCost {
            total_fee: 1500,
            gpu_cycles: 600_000_000,
            cpu_cycles: 60_000_000,
            memory_bytes: 5_000_000_000,
            output_size: 600_000_000,
        };

        let result = JobResult::new([7u8; 32], cost);

        let result_err = executor.complete_job(&job_id, result, 20);

        assert!(matches!(result_err, Err(JobError::BudgetExceeded)));
    }

    #[test]
    fn test_get_pending_jobs() {
        let mut executor = JobExecutor::new();

        let ticket1 = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);
        let ticket2 = JobTicket::new([1u8; 32], [3u8; 32], [4u8; 32], Budget::new(1000), 100);

        executor.submit_job(ticket1, 10).unwrap();
        executor.submit_job(ticket2, 10).unwrap();

        let pending = executor.get_pending_jobs();

        assert_eq!(pending.len(), 2);
    }

    #[test]
    fn test_get_jobs_by_operator() {
        let mut executor = JobExecutor::new();

        let ticket = JobTicket::new([1u8; 32], [2u8; 32], [3u8; 32], Budget::new(1000), 100);

        let job_id = executor.submit_job(ticket, 10).unwrap();

        executor.assign_job(&job_id, [5u8; 32], 15).unwrap();

        let jobs = executor.get_jobs_by_operator(&[5u8; 32]);

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
