use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryTier {
    Ephemeral,
    Regional,
    Committed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRegion {
    pub id: [u8; 32],
    pub tier: MemoryTier,
    pub data: HashMap<Vec<u8>, Vec<u8>>,
    pub partition_id: u32,
    pub last_access: u64,
    pub size_bytes: u64,
}

impl MemoryRegion {
    pub fn new_ephemeral(id: [u8; 32], partition_id: u32) -> Self {
        Self {
            id,
            tier: MemoryTier::Ephemeral,
            data: HashMap::new(),
            partition_id,
            last_access: 0,
            size_bytes: 0,
        }
    }

    pub fn put(&mut self, key: Vec<u8>, value: Vec<u8>) {
        self.size_bytes = self
            .size_bytes
            .saturating_sub(self.data.get(&key).map(|v| v.len() as u64).unwrap_or(0));
        self.size_bytes += value.len() as u64;
        self.data.insert(key, value);
        self.last_access = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }

    pub fn get(&mut self, key: &[u8]) -> Option<&Vec<u8>> {
        self.last_access = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.data.get(key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceMemory {
    pub ephemeral: HashMap<[u8; 32], MemoryRegion>,
    pub regional: HashMap<[u8; 32], MemoryRegion>,
    pub committed: HashMap<[u8; 32], MemoryRegion>,
}

impl IntelligenceMemory {
    pub fn new() -> Self {
        Self {
            ephemeral: HashMap::new(),
            regional: HashMap::new(),
            committed: HashMap::new(),
        }
    }

    pub fn create_ephemeral(&mut self, id: [u8; 32], partition_id: u32) {
        let region = MemoryRegion::new_ephemeral(id, partition_id);
        self.ephemeral.insert(id, region);
    }

    pub fn commit(
        &mut self,
        region_id: [u8; 32],
        commitment_hash: [u8; 32],
    ) -> Option<MemoryRegion> {
        if let Some(region) = self.ephemeral.remove(&region_id) {
            let mut committed = region;
            committed.tier = MemoryTier::Committed;
            self.committed.insert(commitment_hash, committed.clone());
            Some(committed)
        } else {
            None
        }
    }
}

impl Default for IntelligenceMemory {
    fn default() -> Self {
        Self::new()
    }
}
