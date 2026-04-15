use crate::network::{NetworkNode, NetworkConfig, NetworkMode};
use crate::truth::{State, ConsensusEngine, Validator, RpcServer};
use crate::intelligence::{JobExecutor, OperatorRegistry, Receipt};
use crate::health::{HealthMonitor, HealthStatus};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct LiteVisionNode {
    pub network: NetworkNode,
    pub truth_state: Arc<RwLock<State>>,
    pub consensus: Arc<ConsensusEngine>,
    pub job_executor: Arc<JobExecutor>,
    pub operator_registry: Arc<OperatorRegistry>,
    pub health_monitor: Arc<HealthMonitor>,
    pub running: bool,
}

impl LiteVisionNode {
    pub fn new(config: NetworkConfig) -> Self {
        let truth_state = Arc::new(RwLock::new(State::new()));
        let consensus = Arc::new(ConsensusEngine::new(vec![], 3000));
        let job_executor = Arc::new(JobExecutor::new(1000));
        let operator_registry = Arc::new(OperatorRegistry::new());
        let health_monitor = Arc::new(HealthMonitor::new());

        Self {
            network: NetworkNode::new(config),
            truth_state,
            consensus,
            job_executor,
            operator_registry,
            health_monitor,
            running: false,
        }
    }

    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.consensus.add_validator(validator);
        self
    }

    pub async fn start(&mut self) -> Result<(), NodeError> {
        self.network.start().await?;
        
        self.health_monitor.register_component("truth");
        self.health_monitor.register_component("intelligence");
        self.health_monitor.register_component("network");
        
        self.health_monitor.update("truth", "consensus", HealthStatus::Healthy, None);
        self.health_monitor.update("network", "p2p", HealthStatus::Healthy, None);
        
        self.running = true;
        Ok(())
    }

    pub async fn stop(&mut self) {
        self.running = false;
        self.network.stop().await;
        
        self.health_monitor.update("truth", "consensus", HealthStatus::Unknown, None);
        self.health_monitor.update("network", "p2p", HealthStatus::Unknown, None);
    }

    pub fn is_running(&self) -> bool {
        self.running && self.network.is_running()
    }

    pub async fn get_health(&self) -> crate::health::HealthReport {
        self.health_monitor.get_report(self.network.config.node_id)
    }

    pub async fn submit_job(&self, job: crate::intelligence::job::Job) -> Result<[u8; 32], NodeError> {
        if !self.is_running() {
            return Err(NodeError::NotRunning);
        }

        self.job_executor.submit(job).await
            .map(|receipt| receipt.id)
            .map_err(|e| NodeError::JobError(e.to_string()))
    }

    pub async fn get_receipt(&self, job_id: [u8; 32]) -> Option<Receipt> {
        self.job_executor.get_receipt(job_id).await
    }
}

#[derive(Debug)]
pub struct NodeError(String);

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for NodeError {}

impl From<std::io::Error> for NodeError {
    fn from(e: std::io::Error) -> Self {
        NodeError(e.to_string())
    }
}

impl From<String> for NodeError {
    fn from(s: String) -> Self {
        NodeError(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_creation() {
        let config = NetworkConfig::default();
        let node = LiteVisionNode::new(config);
        
        assert!(!node.is_running());
    }

    #[test]
    fn test_with_validator() {
        let config = NetworkConfig::default();
        let node = LiteVisionNode::new(config);
        
        let validator = crate::truth::Validator::new(
            [1u8; 32],
            1000,
            [2u8; 32],
        );
        
        let node = node.with_validator(validator);
        assert!(!node.is_running());
    }

    #[tokio::test]
    async fn test_node_health() {
        let config = NetworkConfig::default();
        let node = LiteVisionNode::new(config);
        
        let report = node.get_health().await;
        
        assert_eq!(report.overall_status, HealthStatus::Unknown);
    }
}