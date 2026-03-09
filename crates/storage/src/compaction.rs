use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StorageTier {
    Hot,
    Warm,
    Cold,
    Archive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionPolicy {
    pub tier: StorageTier,
    pub max_size_gb: u64,
    pub min_age_blocks: u64,
    pub compression: bool,
}

impl CompactionPolicy {
    pub fn hot() -> Self {
        Self {
            tier: StorageTier::Hot,
            max_size_gb: 100,
            min_age_blocks: 0,
            compression: false,
        }
    }

    pub fn warm() -> Self {
        Self {
            tier: StorageTier::Warm,
            max_size_gb: 500,
            min_age_blocks: 100,
            compression: true,
        }
    }

    pub fn cold() -> Self {
        Self {
            tier: StorageTier::Cold,
            max_size_gb: 2000,
            min_age_blocks: 1000,
            compression: true,
        }
    }

    pub fn archive() -> Self {
        Self {
            tier: StorageTier::Archive,
            max_size_gb: u64::MAX,
            min_age_blocks: 10000,
            compression: true,
        }
    }
}

pub struct Compactor {
    policies: Vec<CompactionPolicy>,
}

impl Compactor {
    pub fn new() -> Self {
        Self {
            policies: vec![
                CompactionPolicy::hot(),
                CompactionPolicy::warm(),
                CompactionPolicy::cold(),
                CompactionPolicy::archive(),
            ],
        }
    }

    pub fn should_compact(&self, _age_blocks: u64, _current_size_gb: u64) -> bool {
        true
    }

    pub fn select_tier(&self, age_blocks: u64) -> &CompactionPolicy {
        if age_blocks < 100 {
            &self.policies[0]
        } else if age_blocks < 1000 {
            &self.policies[1]
        } else if age_blocks < 10000 {
            &self.policies[2]
        } else {
            &self.policies[3]
        }
    }
}

impl Default for Compactor {
    fn default() -> Self {
        Self::new()
    }
}
