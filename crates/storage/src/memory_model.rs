use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryType {
    Ephemeral,
    Regional,
    Committed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub memory_type: MemoryType,
    pub max_size_gb: u64,
    pub eviction_policy: EvictionPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionPolicy {
    LRU,
    LFU,
    FIFO,
}

impl MemoryConfig {
    pub fn ephemeral() -> Self {
        Self {
            memory_type: MemoryType::Ephemeral,
            max_size_gb: 16,
            eviction_policy: EvictionPolicy::LRU,
        }
    }

    pub fn regional() -> Self {
        Self {
            memory_type: MemoryType::Regional,
            max_size_gb: 64,
            eviction_policy: EvictionPolicy::LFU,
        }
    }

    pub fn committed() -> Self {
        Self {
            memory_type: MemoryType::Committed,
            max_size_gb: 256,
            eviction_policy: EvictionPolicy::FIFO,
        }
    }
}

pub struct DualPlaneMemory {
    pub ephemeral: MemoryConfig,
    pub regional: MemoryConfig,
    pub committed: MemoryConfig,
}

impl DualPlaneMemory {
    pub fn new() -> Self {
        Self {
            ephemeral: MemoryConfig::ephemeral(),
            regional: MemoryConfig::regional(),
            committed: MemoryConfig::committed(),
        }
    }

    pub fn config_for(&self, memory_type: &MemoryType) -> &MemoryConfig {
        match memory_type {
            MemoryType::Ephemeral => &self.ephemeral,
            MemoryType::Regional => &self.regional,
            MemoryType::Committed => &self.committed,
        }
    }
}

impl Default for DualPlaneMemory {
    fn default() -> Self {
        Self::new()
    }
}
