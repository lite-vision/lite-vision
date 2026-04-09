use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptResponse {
    pub receipt_id: [u8; 32],
    pub job_id: [u8; 32],
    pub operator_id: [u8; 32],
    pub input_hash: [u8; 32],
    pub output_hash: [u8; 32],
    pub compute_used: ComputeUsage,
    pub fee: u64,
    pub verification_status: VerificationStatus,
    pub block_height: u64,
    pub timestamp: u64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeUsage {
    pub gpu_cycles: u64,
    pub cpu_cycles: u64,
    pub memory_bytes: u64,
    pub output_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    Unverified,
    Verified,
    Challenged,
    Disputed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptQuery {
    pub job_id: Option<[u8; 32]>,
    pub operator_id: Option<[u8; 32]>,
    pub from_height: Option<u64>,
    pub to_height: Option<u64>,
    pub limit: u32,
    pub offset: u32,
}

impl Default for ReceiptQuery {
    fn default() -> Self {
        Self {
            job_id: None,
            operator_id: None,
            from_height: None,
            to_height: None,
            limit: 100,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptListResponse {
    pub receipts: Vec<ReceiptResponse>,
    pub total_count: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRequest {
    pub artifact_id: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactResponse {
    pub artifact_id: [u8; 32],
    pub content_hash: [u8; 32],
    pub size_bytes: u64,
    pub content_type: String,
    pub created_at: u64,
    pub data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactListRequest {
    pub job_id: Option<[u8; 32]>,
    pub content_type: Option<String>,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactListResponse {
    pub artifacts: Vec<ArtifactResponse>,
    pub total_count: u64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RPACKDeltaRequest {
    pub base_artifact_id: [u8; 32],
    pub delta_artifact_id: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RPACKDeltaResponse {
    pub applied: bool,
    pub result_artifact_id: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub chain_height: u64,
    pub finalized_height: u64,
    pub active_partitions: u32,
    pub total_operators: u32,
    pub active_operators: u32,
    pub current_epoch: u64,
    pub network_id: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: [u8; 32],
    pub node_type: NodeType,
    pub version: String,
    pub endpoint: String,
    pub partitions: Vec<u32>,
    pub status: NodeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    Validator,
    Operator,
    FullNode,
    LightNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Active,
    Syncing,
    Offline,
    Faulted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationParams {
    pub page: u32,
    pub page_size: u32,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: 50,
        }
    }
}

impl PaginationParams {
    pub fn max_page_size() -> u32 {
        100
    }

    pub fn validate(&self) -> bool {
        self.page > 0 && self.page_size > 0 && self.page_size <= Self::max_page_size()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub expires_at: u64,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    pub client_id: [u8; 32],
    pub private_key: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitInfo {
    pub requests_remaining: u32,
    pub reset_at: u64,
    pub limit: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_params_validate() {
        let params = PaginationParams::default();
        assert!(params.validate());
    }

    #[test]
    fn test_pagination_params_invalid() {
        let params = PaginationParams {
            page: 0,
            page_size: 50,
        };
        assert!(!params.validate());
    }

    #[test]
    fn test_pagination_params_max_size() {
        let params = PaginationParams {
            page: 1,
            page_size: 200,
        };
        assert!(!params.validate());
    }

    #[test]
    fn test_receipt_query_default() {
        let query = ReceiptQuery::default();
        assert_eq!(query.limit, 100);
        assert_eq!(query.offset, 0);
    }
}
