use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PartitionId(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalStateRoot {
    pub partition_roots: HashMap<PartitionId, [u8; 32]>,
    pub version: u32,
}

impl GlobalStateRoot {
    pub fn new() -> Self {
        Self {
            partition_roots: HashMap::new(),
            version: 0,
        }
    }

    pub fn update_partition(&mut self, partition_id: PartitionId, root: [u8; 32]) {
        self.partition_roots.insert(partition_id, root);
    }

    pub fn get_partition_root(&self, partition_id: &PartitionId) -> Option<[u8; 32]> {
        self.partition_roots.get(partition_id).copied()
    }

    pub fn hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&self.version.to_le_bytes());
        let mut keys: Vec<_> = self.partition_roots.keys().collect();
        keys.sort_by_key(|k| k.0);
        for key in keys {
            hasher.update(&key.0.to_le_bytes());
            hasher.update(self.partition_roots[key].as_slice());
        }
        *hasher.finalize().as_bytes()
    }
}

impl Default for GlobalStateRoot {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossPartitionMessage {
    pub message_id: [u8; 32],
    pub source_partition_id: PartitionId,
    pub target_partition_id: PartitionId,
    pub source_block_height: u64,
    pub payload_hash: [u8; 32],
    pub nonce: u64,
}

impl CrossPartitionMessage {
    pub fn new(
        source: PartitionId,
        target: PartitionId,
        source_height: u64,
        payload_hash: [u8; 32],
        nonce: u64,
    ) -> Self {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&source.0.to_le_bytes());
        hasher.update(&target.0.to_le_bytes());
        hasher.update(&source_height.to_le_bytes());
        hasher.update(payload_hash.as_slice());
        hasher.update(&nonce.to_le_bytes());

        Self {
            message_id: *hasher.finalize().as_bytes(),
            source_partition_id: source,
            target_partition_id: target,
            source_block_height: source_height,
            payload_hash,
            nonce,
        }
    }

    pub fn domain_separator() -> &'static [u8] {
        b"LITE-VISION-CROSS-PARTITION-v1"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossPartitionProof {
    pub source_block_hash: [u8; 32],
    pub source_partition_root: [u8; 32],
    pub merkle_proof: Vec<[u8; 32]>,
    pub partition_inclusion_proof: Vec<[u8; 32]>,
    pub qc_validator_count: u32,
    pub qc_signatures: Vec<([u8; 32], Vec<u8>)>,
}

impl CrossPartitionProof {
    pub fn verify(
        &self,
        global_state_root: &GlobalStateRoot,
        message: &CrossPartitionMessage,
    ) -> Result<(), CrossPartitionError> {
        let expected_partition_root = global_state_root
            .get_partition_root(&message.source_partition_id)
            .ok_or(CrossPartitionError::PartitionNotFound)?;

        if self.source_partition_root != expected_partition_root {
            return Err(CrossPartitionError::PartitionRootMismatch);
        }

        if !self.verify_merkle_proof(message.payload_hash) {
            return Err(CrossPartitionError::MerkleProofInvalid);
        }

        if self.qc_validator_count < 2 {
            return Err(CrossPartitionError::InvalidQC);
        }

        Ok(())
    }

    fn verify_merkle_proof(&self, leaf: [u8; 32]) -> bool {
        let mut current = leaf;
        for proof_element in &self.merkle_proof {
            let mut hasher = blake3::Hasher::new();
            if current < *proof_element {
                hasher.update(current.as_slice());
                hasher.update(proof_element.as_slice());
            } else {
                hasher.update(proof_element.as_slice());
                hasher.update(current.as_slice());
            }
            current = *hasher.finalize().as_bytes();
        }
        current == self.source_partition_root
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossPartitionError {
    InvalidQC,
    QCVerificationFailed,
    PartitionRootMismatch,
    PartitionNotFound,
    MerkleProofInvalid,
    MessageAlreadyConsumed,
    SourceHeightTooOld,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumedMessageRegistry {
    pub consumed: HashSet<[u8; 32]>,
    pub min_valid_height: u64,
}

impl ConsumedMessageRegistry {
    pub fn new() -> Self {
        Self {
            consumed: HashSet::new(),
            min_valid_height: 0,
        }
    }

    pub fn mark_consumed(&mut self, message_id: [u8; 32]) {
        self.consumed.insert(message_id);
    }

    pub fn is_consumed(&self, message_id: &[u8; 32]) -> bool {
        self.consumed.contains(message_id)
    }

    pub fn prune(&mut self, current_height: u64, window: u64) {
        self.min_valid_height = current_height.saturating_sub(window);
    }
}

impl Default for ConsumedMessageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactCommit {
    pub operator_id: [u8; 32],
    pub job_id: [u8; 32],
    pub input_hash: [u8; 32],
    pub output_hash: [u8; 32],
    pub resource_hash: [u8; 32],
    pub receipt_signature: Vec<u8>,
    pub execution_nonce: u64,
    pub block_height: u64,
}

impl ArtifactCommit {
    pub fn new(
        operator_id: [u8; 32],
        job_id: [u8; 32],
        input_hash: [u8; 32],
        output_hash: [u8; 32],
        resource_hash: [u8; 32],
        execution_nonce: u64,
    ) -> Self {
        Self {
            operator_id,
            job_id,
            input_hash,
            output_hash,
            resource_hash,
            receipt_signature: Vec::new(),
            execution_nonce,
            block_height: 0,
        }
    }

    pub fn domain_separator() -> &'static [u8] {
        b"LITE-VISION-ARTIFACT-COMMIT-v1"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptRegistry {
    pub receipts: HashMap<([u8; 32], [u8; 32]), ReceiptEntry>,
    pub execution_nonces: HashMap<([u8; 32], [u8; 32]), HashSet<u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptEntry {
    pub output_hash: [u8; 32],
    pub operator_id: [u8; 32],
    pub block_height: u64,
}

impl ReceiptRegistry {
    pub fn new() -> Self {
        Self {
            receipts: HashMap::new(),
            execution_nonces: HashMap::new(),
        }
    }

    pub fn register_receipt(&mut self, commit: &ArtifactCommit) -> Result<(), ReceiptError> {
        let key = (commit.job_id, commit.operator_id);

        if self.receipts.contains_key(&key) {
            return Err(ReceiptError::DuplicateReceipt);
        }

        let nonces = self
            .execution_nonces
            .entry(key)
            .or_insert_with(HashSet::new);

        if nonces.contains(&commit.execution_nonce) {
            return Err(ReceiptError::NonceReused);
        }

        nonces.insert(commit.execution_nonce);

        self.receipts.insert(
            key,
            ReceiptEntry {
                output_hash: commit.output_hash,
                operator_id: commit.operator_id,
                block_height: commit.block_height,
            },
        );

        Ok(())
    }

    pub fn get_receipt(&self, job_id: &[u8; 32], operator_id: &[u8; 32]) -> Option<&ReceiptEntry> {
        self.receipts.get(&(*job_id, *operator_id))
    }

    pub fn has_receipt_for_job(&self, job_id: &[u8; 32]) -> bool {
        self.receipts.keys().any(|(j, _)| j == job_id)
    }
}

impl Default for ReceiptRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptError {
    DuplicateReceipt,
    NonceReused,
    JobClosed,
}

pub struct PartitionManager {
    pub partitions: HashMap<PartitionId, Partition>,
    pub global_state_root: GlobalStateRoot,
    pub message_registry: ConsumedMessageRegistry,
    pub receipt_registry: ReceiptRegistry,
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {
            partitions: HashMap::new(),
            global_state_root: GlobalStateRoot::new(),
            message_registry: ConsumedMessageRegistry::new(),
            receipt_registry: ReceiptRegistry::new(),
        }
    }

    pub fn add_partition(&mut self, partition_id: PartitionId) {
        self.partitions.insert(
            partition_id,
            Partition {
                id: partition_id,
                merkle_root: [0u8; 32],
                block_height: 0,
                validators: Vec::new(),
            },
        );
        self.global_state_root
            .update_partition(partition_id, [0u8; 32]);
    }

    pub fn remove_partition(&mut self, partition_id: &PartitionId) -> Option<Partition> {
        self.global_state_root.partition_roots.remove(partition_id);
        self.partitions.remove(partition_id)
    }

    pub fn update_partition_root(&mut self, partition_id: &PartitionId, root: [u8; 32]) {
        if let Some(partition) = self.partitions.get_mut(partition_id) {
            partition.merkle_root = root;
        }
        self.global_state_root.update_partition(*partition_id, root);
    }

    pub fn process_cross_partition_message(
        &mut self,
        message: CrossPartitionMessage,
        proof: CrossPartitionProof,
    ) -> Result<(), CrossPartitionError> {
        if self.message_registry.is_consumed(&message.message_id) {
            return Err(CrossPartitionError::MessageAlreadyConsumed);
        }

        if message.source_block_height < self.message_registry.min_valid_height {
            return Err(CrossPartitionError::SourceHeightTooOld);
        }

        proof.verify(&self.global_state_root, &message)?;

        self.message_registry.mark_consumed(message.message_id);

        Ok(())
    }

    pub fn commit_artifact(&mut self, commit: ArtifactCommit) -> Result<(), ReceiptError> {
        self.receipt_registry.register_receipt(&commit)
    }

    pub fn verify_artifact(&self, job_id: &[u8; 32]) -> Option<([u8; 32], [u8; 32])> {
        self.receipt_registry
            .receipts
            .iter()
            .find(|((j, _), _)| j == job_id)
            .map(|(_, entry)| (entry.output_hash, entry.operator_id))
    }

    pub fn prune(&mut self, current_height: u64) {
        const PRUNE_WINDOW: u64 = 10000;
        self.message_registry.prune(current_height, PRUNE_WINDOW);
    }
}

impl Default for PartitionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    pub id: PartitionId,
    pub merkle_root: [u8; 32],
    pub block_height: u64,
    pub validators: Vec<[u8; 32]>,
}

impl Partition {
    pub fn new(id: PartitionId) -> Self {
        Self {
            id,
            merkle_root: [0u8; 32],
            block_height: 0,
            validators: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_partition(id: u32) -> Partition {
        Partition::new(PartitionId(id))
    }

    #[test]
    fn test_global_state_root() {
        let mut root = GlobalStateRoot::new();
        root.update_partition(PartitionId(1), [1u8; 32]);
        root.update_partition(PartitionId(2), [2u8; 32]);

        assert_eq!(root.get_partition_root(&PartitionId(1)), Some([1u8; 32]));
        assert_eq!(root.get_partition_root(&PartitionId(2)), Some([2u8; 32]));
        assert_eq!(root.get_partition_root(&PartitionId(3)), None);
    }

    #[test]
    fn test_cross_partition_message_id_unique() {
        let msg1 = CrossPartitionMessage::new(PartitionId(1), PartitionId(2), 100, [5u8; 32], 1);

        let msg2 = CrossPartitionMessage::new(PartitionId(1), PartitionId(2), 100, [5u8; 32], 2);

        assert_ne!(msg1.message_id, msg2.message_id);
    }

    #[test]
    fn test_consumed_message_registry() {
        let mut registry = ConsumedMessageRegistry::new();
        let msg_id = [1u8; 32];

        assert!(!registry.is_consumed(&msg_id));

        registry.mark_consumed(msg_id);

        assert!(registry.is_consumed(&msg_id));
    }

    #[test]
    fn test_consumed_message_registry_prune() {
        let mut registry = ConsumedMessageRegistry::new();

        registry.mark_consumed([1u8; 32]);
        registry.min_valid_height = 100;

        registry.prune(200, 50);

        assert_eq!(registry.min_valid_height, 150);
    }

    #[test]
    fn test_receipt_registry_duplicate_receipt() {
        let mut registry = ReceiptRegistry::new();

        let mut commit =
            ArtifactCommit::new([1u8; 32], [5u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], 1);
        commit.block_height = 100;

        assert!(registry.register_receipt(&commit).is_ok());

        commit.block_height = 101;
        assert!(matches!(
            registry.register_receipt(&commit),
            Err(ReceiptError::DuplicateReceipt)
        ));
    }

    #[test]
    fn test_partition_manager_add_remove() {
        let mut manager = PartitionManager::new();

        manager.add_partition(PartitionId(1));
        manager.add_partition(PartitionId(2));

        assert!(manager.partitions.contains_key(&PartitionId(1)));
        assert!(manager.partitions.contains_key(&PartitionId(2)));

        manager.remove_partition(&PartitionId(1));

        assert!(!manager.partitions.contains_key(&PartitionId(1)));
        assert!(manager.partitions.contains_key(&PartitionId(2)));
        assert!(!manager
            .global_state_root
            .partition_roots
            .contains_key(&PartitionId(1)));
    }

    #[test]
    fn test_partition_manager_update_root() {
        let mut manager = PartitionManager::new();

        manager.add_partition(PartitionId(1));
        manager.update_partition_root(&PartitionId(1), [9u8; 32]);

        let partition = &manager.partitions[&PartitionId(1)];
        assert_eq!(partition.merkle_root, [9u8; 32]);

        assert_eq!(
            manager
                .global_state_root
                .get_partition_root(&PartitionId(1)),
            Some([9u8; 32])
        );
    }

    #[test]
    fn test_artifact_commit() {
        let commit = ArtifactCommit::new([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32], 42);

        assert_eq!(commit.operator_id, [1u8; 32]);
        assert_eq!(commit.job_id, [2u8; 32]);
        assert_eq!(commit.execution_nonce, 42);
    }

    #[test]
    fn test_verify_artifact_not_found() {
        let manager = PartitionManager::new();

        let result = manager.verify_artifact(&[9u8; 32]);

        assert!(result.is_none());
    }

    #[test]
    fn test_commit_artifact_and_verify() {
        let mut manager = PartitionManager::new();

        let mut commit =
            ArtifactCommit::new([1u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], [5u8; 32], 1);
        commit.block_height = 100;

        manager.commit_artifact(commit).unwrap();

        let result = manager.verify_artifact(&[2u8; 32]);

        assert!(result.is_some());
        assert_eq!(result.unwrap().0, [4u8; 32]);
    }

    #[test]
    fn test_different_operators_same_job() {
        let mut registry = ReceiptRegistry::new();

        let mut commit1 =
            ArtifactCommit::new([1u8; 32], [5u8; 32], [2u8; 32], [3u8; 32], [4u8; 32], 1);
        commit1.block_height = 100;

        assert!(registry.register_receipt(&commit1).is_ok());

        let mut commit2 =
            ArtifactCommit::new([2u8; 32], [5u8; 32], [2u8; 32], [6u8; 32], [7u8; 32], 1);
        commit2.block_height = 101;

        assert!(registry.register_receipt(&commit2).is_ok());
    }

    #[test]
    fn test_merkle_proof_verification() {
        let proof = CrossPartitionProof {
            source_block_hash: [1u8; 32],
            source_partition_root: [3u8; 32],
            merkle_proof: vec![],
            partition_inclusion_proof: vec![],
            qc_validator_count: 3,
            qc_signatures: vec![],
        };

        assert!(proof.verify_merkle_proof([3u8; 32]));
        assert!(!proof.verify_merkle_proof([4u8; 32]));
    }

    #[test]
    fn test_cross_partition_proof_verification() {
        let mut global_root = GlobalStateRoot::new();
        global_root.update_partition(PartitionId(1), [3u8; 32]);

        let message = CrossPartitionMessage::new(PartitionId(1), PartitionId(2), 100, [3u8; 32], 1);

        let proof = CrossPartitionProof {
            source_block_hash: [1u8; 32],
            source_partition_root: [3u8; 32],
            merkle_proof: vec![],
            partition_inclusion_proof: vec![],
            qc_validator_count: 3,
            qc_signatures: vec![],
        };

        assert!(proof.verify(&global_root, &message).is_ok());
    }

    #[test]
    fn test_cross_partition_proof_wrong_partition_root() {
        let mut global_root = GlobalStateRoot::new();
        global_root.update_partition(PartitionId(1), [3u8; 32]);

        let message = CrossPartitionMessage::new(PartitionId(1), PartitionId(2), 100, [4u8; 32], 1);

        let proof = CrossPartitionProof {
            source_block_hash: [1u8; 32],
            source_partition_root: [5u8; 32],
            merkle_proof: vec![],
            partition_inclusion_proof: vec![],
            qc_validator_count: 3,
            qc_signatures: vec![],
        };

        assert!(matches!(
            proof.verify(&global_root, &message),
            Err(CrossPartitionError::PartitionRootMismatch)
        ));
    }

    #[test]
    fn test_message_already_consumed() {
        let mut manager = PartitionManager::new();
        manager.add_partition(PartitionId(1));

        let message = CrossPartitionMessage::new(PartitionId(1), PartitionId(2), 100, [3u8; 32], 1);

        let proof = CrossPartitionProof {
            source_block_hash: [1u8; 32],
            source_partition_root: [0u8; 32],
            merkle_proof: vec![],
            partition_inclusion_proof: vec![],
            qc_validator_count: 3,
            qc_signatures: vec![],
        };

        manager.message_registry.mark_consumed(message.message_id);

        assert!(matches!(
            manager.process_cross_partition_message(message, proof),
            Err(CrossPartitionError::MessageAlreadyConsumed)
        ));
    }
}
