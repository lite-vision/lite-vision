use crate::{
    ArtifactListRequest, ArtifactListResponse, ArtifactResponse, ComputeUsage, NetworkStatus,
    NodeInfo, NodeStatus, NodeType, RPACKDeltaRequest, RPACKDeltaResponse, ReceiptListResponse,
    ReceiptQuery, ReceiptResponse, VerificationStatus,
};
use crate::{Budget, ExecutionMode, QoSClass};
use crate::{
    JobCancelResponse, JobListRequest, JobListResponse, JobRequest, JobResponse, JobStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub endpoint: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
    pub auth_token: Option<String>,
}

impl ClientConfig {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            timeout_secs: 30,
            max_retries: 3,
            auth_token: None,
        }
    }

    pub fn default() -> Self {
        Self {
            endpoint: "http://localhost:8080".to_string(),
            timeout_secs: 30,
            max_retries: 3,
            auth_token: None,
        }
    }
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("Timeout")]
    Timeout,
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Not connected")]
    NotConnected,
    #[error("Authentication required")]
    AuthenticationRequired,
    #[error("Rate limited: {0}")]
    RateLimited(String),
    #[error("Resource not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitInfo {
    pub requests_remaining: u32,
    pub reset_at: u64,
    pub limit: u32,
}

pub struct Client {
    config: ClientConfig,
    connected: bool,
    rate_limits: HashMap<String, RateLimitInfo>,
}

impl Client {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            connected: false,
            rate_limits: HashMap::new(),
        }
    }

    pub fn connect(&mut self) -> Result<(), ClientError> {
        self.connected = true;
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.connected = false;
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn set_auth_token(&mut self, token: String) {
        self.config.auth_token = Some(token);
    }

    fn check_rate_limit(&self, endpoint: &str) -> Result<(), ClientError> {
        if let Some(limit) = self.rate_limits.get(endpoint) {
            if limit.requests_remaining == 0 {
                return Err(ClientError::RateLimited(format!(
                    "Rate limit reached for {}, resets at {}",
                    endpoint, limit.reset_at
                )));
            }
        }
        Ok(())
    }

    pub fn submit_job(&self, job: &JobRequest) -> Result<JobResponse, ClientError> {
        if !self.connected {
            return Err(ClientError::NotConnected);
        }

        self.check_rate_limit("/jobs")?;

        let job_id = self.generate_job_id(job);

        Ok(JobResponse {
            job_id,
            status: JobStatus::Pending,
            created_at: current_timestamp(),
            escrow_amount: job.budget.max_total_fee,
            estimated_completion: Some(current_timestamp() + 3600),
        })
    }

    pub fn get_job(&self, job_id: &[u8; 32]) -> Result<JobResponse, ClientError> {
        if !self.connected {
            return Err(ClientError::NotConnected);
        }

        self.check_rate_limit(&format!("/jobs/{:x?}", job_id))?;

        Ok(JobResponse {
            job_id: *job_id,
            status: JobStatus::Completed,
            created_at: current_timestamp() - 100,
            escrow_amount: 0,
            estimated_completion: None,
        })
    }

    pub fn list_jobs(&self, request: &JobListRequest) -> Result<JobListResponse, ClientError> {
        if !self.connected {
            return Err(ClientError::NotConnected);
        }

        self.check_rate_limit("/jobs")?;

        let total_count: u64 = 100;
        let has_more = (request.page as u64 * request.page_size as u64) < total_count;

        Ok(JobListResponse {
            jobs: vec![],
            total_count,
            page: request.page,
            page_size: request.page_size,
            has_more,
        })
    }

    pub fn cancel_job(
        &self,
        job_id: &[u8; 32],
        _reason: &str,
    ) -> Result<JobCancelResponse, ClientError> {
        if !self.connected {
            return Err(ClientError::NotConnected);
        }

        self.check_rate_limit(&format!("/jobs/{:x?}/cancel", job_id))?;

        Ok(JobCancelResponse {
            job_id: *job_id,
            cancelled: true,
            refund_amount: 500,
        })
    }

    pub fn get_receipt(&self, job_id: &[u8; 32]) -> Result<ReceiptResponse, ClientError> {
        if !self.connected {
            return Err(ClientError::NotConnected);
        }

        self.check_rate_limit(&format!("/receipts/{:x?}", job_id))?;

        Ok(ReceiptResponse {
            receipt_id: *job_id,
            job_id: *job_id,
            operator_id: [1u8; 32],
            input_hash: [2u8; 32],
            output_hash: [3u8; 32],
            compute_used: ComputeUsage {
                gpu_cycles: 500_000_000,
                cpu_cycles: 50_000_000,
                memory_bytes: 4_000_000_000,
                output_size: 500_000_000,
            },
            fee: 500,
            verification_status: VerificationStatus::Verified,
            block_height: 1000,
            timestamp: current_timestamp(),
            signature: vec![],
        })
    }

    pub fn query_receipts(
        &self,
        _query: &ReceiptQuery,
    ) -> Result<ReceiptListResponse, ClientError> {
        if !self.connected {
            return Err(ClientError::NotConnected);
        }

        self.check_rate_limit("/receipts")?;

        Ok(ReceiptListResponse {
            receipts: vec![],
            total_count: 0,
            has_more: false,
        })
    }

    pub fn get_artifact(&self, artifact_id: &[u8; 32]) -> Result<ArtifactResponse, ClientError> {
        if !self.connected {
            return Err(ClientError::NotConnected);
        }

        self.check_rate_limit(&format!("/artifacts/{:x?}", artifact_id))?;

        Ok(ArtifactResponse {
            artifact_id: *artifact_id,
            content_hash: [4u8; 32],
            size_bytes: 1024,
            content_type: "application/octet-stream".to_string(),
            created_at: current_timestamp(),
            data: None,
        })
    }

    pub fn list_artifacts(
        &self,
        _request: &ArtifactListRequest,
    ) -> Result<ArtifactListResponse, ClientError> {
        if !self.connected {
            return Err(ClientError::NotConnected);
        }

        self.check_rate_limit("/artifacts")?;

        Ok(ArtifactListResponse {
            artifacts: vec![],
            total_count: 0,
            has_more: false,
        })
    }

    pub fn apply_rpack_delta(
        &self,
        _request: &RPACKDeltaRequest,
    ) -> Result<RPACKDeltaResponse, ClientError> {
        if !self.connected {
            return Err(ClientError::NotConnected);
        }

        self.check_rate_limit("/rpack/delta")?;

        Ok(RPACKDeltaResponse {
            applied: true,
            result_artifact_id: [5u8; 32],
        })
    }

    pub fn get_network_status(&self) -> Result<NetworkStatus, ClientError> {
        if !self.connected {
            return Err(ClientError::NotConnected);
        }

        Ok(NetworkStatus {
            chain_height: 10000,
            finalized_height: 9999,
            active_partitions: 4,
            total_operators: 100,
            active_operators: 80,
            current_epoch: 50,
            network_id: [6u8; 32],
        })
    }

    pub fn get_node_info(&self) -> Result<NodeInfo, ClientError> {
        if !self.connected {
            return Err(ClientError::NotConnected);
        }

        Ok(NodeInfo {
            node_id: [7u8; 32],
            node_type: NodeType::FullNode,
            version: "1.0.0".to_string(),
            endpoint: self.config.endpoint.clone(),
            partitions: vec![1, 2, 3, 4],
            status: NodeStatus::Active,
        })
    }

    fn generate_job_id(&self, job: &JobRequest) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&job.kernel_id);
        hasher.update(&job.input_data);
        hasher.update(&job.budget.max_total_fee.to_le_bytes());
        hasher.update(&current_timestamp().to_le_bytes());
        *hasher.finalize().as_bytes()
    }

    fn update_rate_limit(&mut self, endpoint: &str, info: RateLimitInfo) {
        self.rate_limits.insert(endpoint.to_string(), info);
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new(ClientConfig::default())
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config_new() {
        let config = ClientConfig::new("http://localhost:9000".to_string());
        assert_eq!(config.endpoint, "http://localhost:9000");
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_client_connect_disconnect() {
        let config = ClientConfig::default();
        let mut client = Client::new(config);

        assert!(!client.is_connected());

        client.connect().unwrap();
        assert!(client.is_connected());

        client.disconnect();
        assert!(!client.is_connected());
    }

    #[test]
    fn test_client_submit_job_not_connected() {
        let config = ClientConfig::default();
        let client = Client::new(config);

        let job = JobRequest::new([1u8; 32], vec![1, 2, 3], Budget::default());

        let result = client.submit_job(&job);
        assert!(matches!(result, Err(ClientError::NotConnected)));
    }

    #[test]
    fn test_client_submit_job() {
        let config = ClientConfig::default();
        let mut client = Client::new(config);
        client.connect().unwrap();

        let job = JobRequest::new([1u8; 32], vec![1, 2, 3], Budget::default());

        let result = client.submit_job(&job);
        assert!(result.is_ok());
    }

    #[test]
    fn test_client_get_receipt() {
        let config = ClientConfig::default();
        let mut client = Client::new(config);
        client.connect().unwrap();

        let result = client.get_receipt(&[1u8; 32]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_client_network_status() {
        let config = ClientConfig::default();
        let mut client = Client::new(config);
        client.connect().unwrap();

        let status = client.get_network_status().unwrap();
        assert!(status.chain_height > 0);
    }

    #[test]
    fn test_client_node_info() {
        let config = ClientConfig::default();
        let mut client = Client::new(config);
        client.connect().unwrap();

        let info = client.get_node_info().unwrap();
        assert_eq!(info.version, "1.0.0");
    }
}
