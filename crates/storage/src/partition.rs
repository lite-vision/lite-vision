use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    pub id: u32,
    pub state_root: [u8; 32],
    pub size: u64,
    pub validator_count: u32,
    pub status: PartitionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartitionStatus {
    Active,
    Migrating,
    Quarantined,
    Deleted,
}

pub struct PartitionManager {
    partitions: HashMap<u32, Partition>,
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {
            partitions: HashMap::new(),
        }
    }

    pub fn create(&mut self, id: u32) -> &Partition {
        let partition = Partition {
            id,
            state_root: [0u8; 32],
            size: 0,
            validator_count: 0,
            status: PartitionStatus::Active,
        };
        self.partitions.insert(id, partition);
        self.partitions.get(&id).unwrap()
    }

    pub fn get(&self, id: u32) -> Option<&Partition> {
        self.partitions.get(&id)
    }

    pub fn migrate(&mut self, id: u32, target_id: u32) -> Result<(), String> {
        let partition_data = {
            let partition = self.partitions.get(&id).ok_or("Partition not found")?;
            Partition {
                id: target_id,
                state_root: partition.state_root,
                size: partition.size,
                validator_count: partition.validator_count,
                status: PartitionStatus::Migrating,
            }
        };

        self.partitions.remove(&id);
        self.partitions.insert(target_id, partition_data);

        if let Some(p) = self.partitions.get_mut(&target_id) {
            p.status = PartitionStatus::Active;
        }

        Ok(())
    }

    pub fn delete(&mut self, id: u32) -> Result<(), String> {
        if let Some(partition) = self.partitions.get_mut(&id) {
            partition.status = PartitionStatus::Deleted;
            self.partitions.remove(&id);
            Ok(())
        } else {
            Err("Partition not found".to_string())
        }
    }
}

impl Default for PartitionManager {
    fn default() -> Self {
        Self::new()
    }
}
