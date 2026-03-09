use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operator {
    pub id: [u8; 32],
    pub pubkey: [u8; 32],
    pub capabilities: OperatorCapabilities,
    pub stake: u64,
    pub reputation: u64,
    pub status: OperatorStatus,
    pub region: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorCapabilities {
    pub gpu_models: Vec<String>,
    pub vram_gb: u32,
    pub kernels_supported: Vec<[u8; 32]>,
    pub max_concurrent_jobs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperatorStatus {
    Registered,
    Active,
    Suspended,
    Deregistered,
}

impl Operator {
    pub fn new(id: [u8; 32], pubkey: [u8; 32], capabilities: OperatorCapabilities) -> Self {
        Self {
            id,
            pubkey,
            capabilities,
            stake: 0,
            reputation: 1000,
            status: OperatorStatus::Registered,
            region: "global".to_string(),
        }
    }

    pub fn activate(&mut self) {
        self.status = OperatorStatus::Active;
    }

    pub fn suspend(&mut self) {
        self.status = OperatorStatus::Suspended;
    }

    pub fn update_reputation(&mut self, delta: i64) {
        let new_rep = (self.reputation as i64 + delta).max(0).min(10000);
        self.reputation = new_rep as u64;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperatorRegistry {
    pub operators: HashMap<[u8; 32], Operator>,
    pub capability_index: HashMap<[u8; 32], Vec<[u8; 32]>>,
}

impl OperatorRegistry {
    pub fn new() -> Self {
        Self {
            operators: HashMap::new(),
            capability_index: HashMap::new(),
        }
    }

    pub fn register(&mut self, operator: Operator) {
        for kernel in &operator.capabilities.kernels_supported {
            self.capability_index
                .entry(*kernel)
                .or_insert_with(Vec::new)
                .push(operator.id);
        }
        self.operators.insert(operator.id, operator);
    }

    pub fn get(&self, id: &[u8; 32]) -> Option<&Operator> {
        self.operators.get(id)
    }

    pub fn get_by_capability(&self, kernel_id: &[u8; 32]) -> Vec<&Operator> {
        self.capability_index
            .get(kernel_id)
            .map(|ids| ids.iter().filter_map(|id| self.operators.get(id)).collect())
            .unwrap_or_default()
    }

    pub fn get_active(&self) -> Vec<&Operator> {
        self.operators
            .values()
            .filter(|o| matches!(o.status, OperatorStatus::Active))
            .collect()
    }
}

impl Default for OperatorRegistry {
    fn default() -> Self {
        Self::new()
    }
}
