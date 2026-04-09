use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRequest {
    pub kernel_id: [u8; 32],
    pub input_data: Vec<u8>,
    pub budget: Budget,
    pub deadline: u64,
    pub execution_mode: ExecutionMode,
    pub qos_class: QoSClass,
    pub max_retries: u32,
    pub partial_allowed: bool,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Budget {
    pub max_total_fee: u64,
    pub max_gpu_cycles: u64,
    pub max_cpu_cycles: u64,
    pub max_memory_bytes: u64,
    pub max_output_size: u64,
}

impl Budget {
    pub fn new(max_total_fee: u64) -> Self {
        Self {
            max_total_fee,
            max_gpu_cycles: 1_000_000_000,
            max_cpu_cycles: 100_000_000,
            max_memory_bytes: 8_589_934_592,
            max_output_size: 1_073_741_824,
        }
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionMode {
    Deterministic,
    Soft,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        ExecutionMode::Soft
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
pub struct JobResponse {
    pub job_id: [u8; 32],
    pub status: JobStatus,
    pub created_at: u64,
    pub escrow_amount: u64,
    pub estimated_completion: Option<u64>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobCancelRequest {
    pub job_id: [u8; 32],
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobCancelResponse {
    pub job_id: [u8; 32],
    pub cancelled: bool,
    pub refund_amount: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobListRequest {
    pub client_id: Option<[u8; 32]>,
    pub status: Option<JobStatus>,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobListResponse {
    pub jobs: Vec<JobResponse>,
    pub total_count: u64,
    pub page: u32,
    pub page_size: u32,
    pub has_more: bool,
}

impl JobRequest {
    pub fn new(kernel_id: [u8; 32], input_data: Vec<u8>, budget: Budget) -> Self {
        Self {
            kernel_id,
            input_data,
            budget,
            deadline: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 3600,
            execution_mode: ExecutionMode::default(),
            qos_class: QoSClass::default(),
            max_retries: 3,
            partial_allowed: false,
            metadata: HashMap::new(),
        }
    }
}
