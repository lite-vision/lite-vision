use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PartitionStatus {
    Pending,
    Active,
    Migrating,
    Frozen,
    Deleted,
}

impl Default for PartitionStatus {
    fn default() -> Self {
        PartitionStatus::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    pub partition_id: u32,
    pub state_root: [u8; 32],
    pub validator_subset: Option<Vec<[u8; 32]>>,
    pub regional_memory_root: Option<[u8; 32]>,
    pub status: PartitionStatus,
    pub created_at: u64,
    pub epoch: u64,
}

impl Partition {
    pub fn new(partition_id: u32, created_at: u64) -> Self {
        Self {
            partition_id,
            state_root: [0u8; 32],
            validator_subset: None,
            regional_memory_root: None,
            status: PartitionStatus::Pending,
            created_at,
            epoch: 0,
        }
    }

    pub fn activate(&mut self, epoch: u64) {
        self.status = PartitionStatus::Active;
        self.epoch = epoch;
    }

    pub fn freeze(&mut self) {
        self.status = PartitionStatus::Frozen;
    }

    pub fn mark_migrating(&mut self) {
        self.status = PartitionStatus::Migrating;
    }

    pub fn delete(&mut self) {
        self.status = PartitionStatus::Deleted;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionCreate {
    pub new_partition_id: u32,
    pub initial_state_root: [u8; 32],
    pub configuration_hash: [u8; 32],
    pub governance_proposal_id: [u8; 32],
    pub signatures: Vec<([u8; 32], Vec<u8>)>,
}

impl PartitionCreate {
    pub fn verify_signatures(&self, validators: &[[u8; 32]]) -> bool {
        self.signatures.len() >= (validators.len() * 2) / 3
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionMigration {
    pub source_partition_id: u32,
    pub target_partition_id: u32,
    pub migration_key: [u8; 32],
    pub keys: Vec<[u8; 32]>,
    pub proof: MigrationProof,
    pub epoch: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationProof {
    pub source_state_root: [u8; 32],
    pub target_state_root: [u8; 32],
    pub migration_merkle_root: [u8; 32],
    pub validator_signatures: Vec<([u8; 32], Vec<u8>)>,
}

impl MigrationProof {
    pub fn verify(&self) -> bool {
        !self.validator_signatures.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionDelete {
    pub partition_id: u32,
    pub final_state_root: [u8; 32],
    pub tombstone_root: [u8; 32],
    pub governance_proposal_id: [u8; 32],
    pub epoch: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionRebalance {
    pub partition_id: u32,
    pub new_validator_subset: Vec<[u8; 32]>,
    pub reason: RebalanceReason,
    pub epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RebalanceReason {
    Load,
    Locality,
    Cost,
    ValidatorChange,
    Governance,
}

pub struct PartitionManager {
    pub partitions: BTreeMap<u32, Partition>,
    pub pending_operations: Vec<PartitionOperation>,
    pub global_state_root: [u8; 32],
    pub current_epoch: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PartitionOperation {
    Create(PartitionCreate),
    Migrate(PartitionMigration),
    Delete(PartitionDelete),
    Rebalance(PartitionRebalance),
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {
            partitions: BTreeMap::new(),
            pending_operations: Vec::new(),
            global_state_root: [0u8; 32],
            current_epoch: 0,
        }
    }

    pub fn create_partition(
        &mut self,
        create: PartitionCreate,
        validators: &[[u8; 32]],
    ) -> Result<Partition, PartitionError> {
        if self.partitions.contains_key(&create.new_partition_id) {
            return Err(PartitionError::PartitionExists);
        }

        if !create.verify_signatures(validators) {
            return Err(PartitionError::InsufficientSignatures);
        }

        let partition = Partition::new(create.new_partition_id, self.current_epoch);

        self.partitions
            .insert(create.new_partition_id, partition.clone());

        self.pending_operations
            .push(PartitionOperation::Create(create));

        Ok(partition)
    }

    pub fn activate_partition(
        &mut self,
        partition_id: u32,
        epoch: u64,
    ) -> Result<(), PartitionError> {
        let partition = self
            .partitions
            .get_mut(&partition_id)
            .ok_or(PartitionError::PartitionNotFound)?;

        if partition.status != PartitionStatus::Pending {
            return Err(PartitionError::InvalidStateTransition);
        }

        partition.activate(epoch);
        self.current_epoch = epoch;
        self.update_global_state_root();

        Ok(())
    }

    pub fn migrate_partition(
        &mut self,
        source_id: u32,
        target_id: u32,
        migration: PartitionMigration,
    ) -> Result<(), PartitionError> {
        let source = self
            .partitions
            .get_mut(&source_id)
            .ok_or(PartitionError::PartitionNotFound)?;

        if source.status != PartitionStatus::Active {
            return Err(PartitionError::InvalidStateTransition);
        }

        source.mark_migrating();

        if let Some(target) = self.partitions.get_mut(&target_id) {
            if target.status == PartitionStatus::Active {
                target.state_root = migration.proof.target_state_root;
            }
        }

        self.pending_operations
            .push(PartitionOperation::Migrate(migration));

        Ok(())
    }

    pub fn complete_migration(
        &mut self,
        partition_id: u32,
        epoch: u64,
    ) -> Result<(), PartitionError> {
        let partition = self
            .partitions
            .get_mut(&partition_id)
            .ok_or(PartitionError::PartitionNotFound)?;

        if partition.status != PartitionStatus::Migrating {
            return Err(PartitionError::InvalidStateTransition);
        }

        partition.activate(epoch);
        self.current_epoch = epoch;
        self.update_global_state_root();

        Ok(())
    }

    pub fn delete_partition(&mut self, delete: PartitionDelete) -> Result<(), PartitionError> {
        let partition = self
            .partitions
            .get_mut(&delete.partition_id)
            .ok_or(PartitionError::PartitionNotFound)?;

        if partition.status != PartitionStatus::Frozen
            && partition.status != PartitionStatus::Active
        {
            return Err(PartitionError::InvalidStateTransition);
        }

        partition.delete();
        self.pending_operations
            .push(PartitionOperation::Delete(delete));
        self.update_global_state_root();

        Ok(())
    }

    pub fn freeze_partition(&mut self, partition_id: u32) -> Result<(), PartitionError> {
        let partition = self
            .partitions
            .get_mut(&partition_id)
            .ok_or(PartitionError::PartitionNotFound)?;

        if partition.status != PartitionStatus::Active {
            return Err(PartitionError::InvalidStateTransition);
        }

        partition.freeze();
        self.update_global_state_root();

        Ok(())
    }

    pub fn rebalance_partition(
        &mut self,
        rebalance: PartitionRebalance,
    ) -> Result<(), PartitionError> {
        let partition = self
            .partitions
            .get_mut(&rebalance.partition_id)
            .ok_or(PartitionError::PartitionNotFound)?;

        if partition.status != PartitionStatus::Active {
            return Err(PartitionError::InvalidStateTransition);
        }

        partition.validator_subset = Some(rebalance.new_validator_subset.clone());
        partition.epoch = rebalance.epoch;

        self.pending_operations
            .push(PartitionOperation::Rebalance(rebalance));

        Ok(())
    }

    pub fn get_partition(&self, partition_id: u32) -> Option<&Partition> {
        self.partitions.get(&partition_id)
    }

    pub fn get_active_partitions(&self) -> Vec<&Partition> {
        self.partitions
            .values()
            .filter(|p| p.status == PartitionStatus::Active)
            .collect()
    }

    pub fn get_partitions_by_status(&self, status: PartitionStatus) -> Vec<&Partition> {
        self.partitions
            .values()
            .filter(|p| p.status == status)
            .collect()
    }

    fn update_global_state_root(&mut self) {
        use blake3::Hasher;
        let mut hasher = Hasher::new();

        for (id, partition) in &self.partitions {
            if partition.status != PartitionStatus::Deleted {
                hasher.update(&id.to_le_bytes());
                hasher.update(&partition.state_root);
            }
        }

        self.global_state_root = *hasher.finalize().as_bytes();
    }

    pub fn advance_epoch(&mut self, new_epoch: u64) {
        self.current_epoch = new_epoch;
    }

    pub fn get_partition_count(&self) -> usize {
        self.partitions.len()
    }

    pub fn get_active_count(&self) -> usize {
        self.get_active_partitions().len()
    }
}

impl Default for PartitionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PartitionError {
    PartitionExists,
    PartitionNotFound,
    InvalidStateTransition,
    InsufficientSignatures,
    MigrationInProgress,
    DataLoss,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_partition(id: u32) -> Partition {
        Partition::new(id, 0)
    }

    #[test]
    fn test_partition_creation() {
        let manager = PartitionManager::new();
        assert_eq!(manager.get_partition_count(), 0);
    }

    #[test]
    fn test_create_partition() {
        let mut manager = PartitionManager::new();

        let create = PartitionCreate {
            new_partition_id: 1,
            initial_state_root: [1u8; 32],
            configuration_hash: [2u8; 32],
            governance_proposal_id: [3u8; 32],
            signatures: vec![],
        };

        let validators = [[1u8; 32], [2u8; 32], [3u8; 32]];

        let result = manager.create_partition(create.clone(), &validators);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_partition_with_signatures() {
        let mut manager = PartitionManager::new();

        let create = PartitionCreate {
            new_partition_id: 1,
            initial_state_root: [1u8; 32],
            configuration_hash: [2u8; 32],
            governance_proposal_id: [3u8; 32],
            signatures: vec![
                ([1u8; 32], vec![]),
                ([2u8; 32], vec![]),
                ([3u8; 32], vec![]),
            ],
        };

        let validators = [[1u8; 32], [2u8; 32], [3u8; 32]];

        let result = manager.create_partition(create, &validators);
        assert!(result.is_ok());
        assert_eq!(manager.get_partition_count(), 1);
    }

    #[test]
    fn test_activate_partition() {
        let mut manager = PartitionManager::new();

        let create = PartitionCreate {
            new_partition_id: 1,
            initial_state_root: [1u8; 32],
            configuration_hash: [2u8; 32],
            governance_proposal_id: [3u8; 32],
            signatures: vec![
                ([1u8; 32], vec![]),
                ([2u8; 32], vec![]),
                ([3u8; 32], vec![]),
            ],
        };

        let validators = [[1u8; 32], [2u8; 32], [3u8; 32]];

        manager.create_partition(create, &validators).unwrap();

        let result = manager.activate_partition(1, 10);
        assert!(result.is_ok());

        let partition = manager.get_partition(1).unwrap();
        assert_eq!(partition.status, PartitionStatus::Active);
        assert_eq!(partition.epoch, 10);
    }

    #[test]
    fn test_activate_nonexistent_partition() {
        let mut manager = PartitionManager::new();

        let result = manager.activate_partition(1, 10);
        assert!(matches!(result, Err(PartitionError::PartitionNotFound)));
    }

    #[test]
    fn test_freeze_partition() {
        let mut manager = PartitionManager::new();

        let create = PartitionCreate {
            new_partition_id: 1,
            initial_state_root: [1u8; 32],
            configuration_hash: [2u8; 32],
            governance_proposal_id: [3u8; 32],
            signatures: vec![
                ([1u8; 32], vec![]),
                ([2u8; 32], vec![]),
                ([3u8; 32], vec![]),
            ],
        };

        let validators = [[1u8; 32], [2u8; 32], [3u8; 32]];

        manager.create_partition(create, &validators).unwrap();
        manager.activate_partition(1, 10).unwrap();

        let result = manager.freeze_partition(1);
        assert!(result.is_ok());

        let partition = manager.get_partition(1).unwrap();
        assert_eq!(partition.status, PartitionStatus::Frozen);
    }

    #[test]
    fn test_delete_partition() {
        let mut manager = PartitionManager::new();

        let create = PartitionCreate {
            new_partition_id: 1,
            initial_state_root: [1u8; 32],
            configuration_hash: [2u8; 32],
            governance_proposal_id: [3u8; 32],
            signatures: vec![
                ([1u8; 32], vec![]),
                ([2u8; 32], vec![]),
                ([3u8; 32], vec![]),
            ],
        };

        let validators = [[1u8; 32], [2u8; 32], [3u8; 32]];

        manager.create_partition(create, &validators).unwrap();
        manager.activate_partition(1, 10).unwrap();

        let delete = PartitionDelete {
            partition_id: 1,
            final_state_root: [5u8; 32],
            tombstone_root: [6u8; 32],
            governance_proposal_id: [7u8; 32],
            epoch: 20,
        };

        let result = manager.delete_partition(delete);
        assert!(result.is_ok());

        let partition = manager.get_partition(1).unwrap();
        assert_eq!(partition.status, PartitionStatus::Deleted);
    }

    #[test]
    fn test_rebalance_partition() {
        let mut manager = PartitionManager::new();

        let create = PartitionCreate {
            new_partition_id: 1,
            initial_state_root: [1u8; 32],
            configuration_hash: [2u8; 32],
            governance_proposal_id: [3u8; 32],
            signatures: vec![
                ([1u8; 32], vec![]),
                ([2u8; 32], vec![]),
                ([3u8; 32], vec![]),
            ],
        };

        let validators = [[1u8; 32], [2u8; 32], [3u8; 32]];

        manager.create_partition(create, &validators).unwrap();
        manager.activate_partition(1, 10).unwrap();

        let rebalance = PartitionRebalance {
            partition_id: 1,
            new_validator_subset: vec![[10u8; 32], [11u8; 32]],
            reason: RebalanceReason::Load,
            epoch: 15,
        };

        let result = manager.rebalance_partition(rebalance);
        assert!(result.is_ok());

        let partition = manager.get_partition(1).unwrap();
        assert!(partition.validator_subset.is_some());
    }

    #[test]
    fn test_migrate_partition() {
        let mut manager = PartitionManager::new();

        let create1 = PartitionCreate {
            new_partition_id: 1,
            initial_state_root: [1u8; 32],
            configuration_hash: [2u8; 32],
            governance_proposal_id: [3u8; 32],
            signatures: vec![
                ([1u8; 32], vec![]),
                ([2u8; 32], vec![]),
                ([3u8; 32], vec![]),
            ],
        };

        let create2 = PartitionCreate {
            new_partition_id: 2,
            initial_state_root: [4u8; 32],
            configuration_hash: [5u8; 32],
            governance_proposal_id: [6u8; 32],
            signatures: vec![
                ([1u8; 32], vec![]),
                ([2u8; 32], vec![]),
                ([3u8; 32], vec![]),
            ],
        };

        let validators = [[1u8; 32], [2u8; 32], [3u8; 32]];

        manager.create_partition(create1, &validators).unwrap();
        manager.activate_partition(1, 10).unwrap();
        manager.create_partition(create2, &validators).unwrap();
        manager.activate_partition(2, 10).unwrap();

        let migration = PartitionMigration {
            source_partition_id: 1,
            target_partition_id: 2,
            migration_key: [7u8; 32],
            keys: vec![[8u8; 32]],
            proof: MigrationProof {
                source_state_root: [1u8; 32],
                target_state_root: [4u8; 32],
                migration_merkle_root: [9u8; 32],
                validator_signatures: vec![],
            },
            epoch: 15,
        };

        let result = manager.migrate_partition(1, 2, migration);
        assert!(result.is_ok());

        let partition = manager.get_partition(1).unwrap();
        assert_eq!(partition.status, PartitionStatus::Migrating);
    }

    #[test]
    fn test_get_active_partitions() {
        let mut manager = PartitionManager::new();

        let create1 = PartitionCreate {
            new_partition_id: 1,
            initial_state_root: [1u8; 32],
            configuration_hash: [2u8; 32],
            governance_proposal_id: [3u8; 32],
            signatures: vec![
                ([1u8; 32], vec![]),
                ([2u8; 32], vec![]),
                ([3u8; 32], vec![]),
            ],
        };

        let create2 = PartitionCreate {
            new_partition_id: 2,
            initial_state_root: [4u8; 32],
            configuration_hash: [5u8; 32],
            governance_proposal_id: [6u8; 32],
            signatures: vec![
                ([1u8; 32], vec![]),
                ([2u8; 32], vec![]),
                ([3u8; 32], vec![]),
            ],
        };

        let validators = [[1u8; 32], [2u8; 32], [3u8; 32]];

        manager.create_partition(create1, &validators).unwrap();
        manager.activate_partition(1, 10).unwrap();

        manager.create_partition(create2, &validators).unwrap();

        assert_eq!(manager.get_active_count(), 1);

        manager.activate_partition(2, 10).unwrap();

        assert_eq!(manager.get_active_count(), 2);
    }

    #[test]
    fn test_global_state_root_update() {
        let mut manager = PartitionManager::new();

        let create = PartitionCreate {
            new_partition_id: 1,
            initial_state_root: [1u8; 32],
            configuration_hash: [2u8; 32],
            governance_proposal_id: [3u8; 32],
            signatures: vec![
                ([1u8; 32], vec![]),
                ([2u8; 32], vec![]),
                ([3u8; 32], vec![]),
            ],
        };

        let validators = [[1u8; 32], [2u8; 32], [3u8; 32]];

        manager.create_partition(create, &validators).unwrap();
        manager.activate_partition(1, 10).unwrap();

        assert_ne!(manager.global_state_root, [0u8; 32]);
    }

    #[test]
    fn test_partition_status_transitions() {
        let mut partition = Partition::new(1, 100);

        assert_eq!(partition.status, PartitionStatus::Pending);

        partition.activate(10);
        assert_eq!(partition.status, PartitionStatus::Active);

        partition.freeze();
        assert_eq!(partition.status, PartitionStatus::Frozen);

        partition.mark_migrating();
        assert_eq!(partition.status, PartitionStatus::Migrating);

        partition.delete();
        assert_eq!(partition.status, PartitionStatus::Deleted);
    }

    #[test]
    fn test_partition_create_already_exists() {
        let mut manager = PartitionManager::new();

        let create = PartitionCreate {
            new_partition_id: 1,
            initial_state_root: [1u8; 32],
            configuration_hash: [2u8; 32],
            governance_proposal_id: [3u8; 32],
            signatures: vec![
                ([1u8; 32], vec![]),
                ([2u8; 32], vec![]),
                ([3u8; 32], vec![]),
            ],
        };

        let validators = [[1u8; 32], [2u8; 32], [3u8; 32]];

        manager
            .create_partition(create.clone(), &validators)
            .unwrap();
        let result = manager.create_partition(create, &validators);

        assert!(matches!(result, Err(PartitionError::PartitionExists)));
    }
}
