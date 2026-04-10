use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: [u8; 32],
    pub content_hash: [u8; 32],
    pub size: u64,
    pub created_at: u64,
    pub pinned: bool,
    pub replication_factor: u32,
    pub storage_tier: StorageTier,
    pub access_count: u64,
    pub last_accessed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageTier {
    Hot,
    Warm,
    Cold,
    Archived,
}

impl Default for StorageTier {
    fn default() -> Self {
        StorageTier::Hot
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactMetadata {
    pub content_type: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub custom: HashMap<String, String>,
}

impl Default for ArtifactMetadata {
    fn default() -> Self {
        Self {
            content_type: "application/octet-stream".to_string(),
            description: None,
            tags: Vec::new(),
            custom: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityProof {
    pub artifact_id: [u8; 32],
    pub content_hash: [u8; 32],
    pub size: u64,
    pub proof_data: Vec<u8>,
    pub timestamp: u64,
}

impl IntegrityProof {
    pub fn verify(&self) -> bool {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&self.artifact_id);
        hasher.update(&self.content_hash);
        hasher.update(&self.size.to_le_bytes());
        let computed = hasher.finalize();
        computed.as_bytes()[..8] == self.proof_data[..8]
    }
}

pub struct ArtifactStore {
    artifacts: HashMap<[u8; 32], Artifact>,
    index: HashMap<[u8; 32], Vec<[u8; 32]>>,
    metadata: HashMap<[u8; 32], ArtifactMetadata>,
    integrity_proofs: HashMap<[u8; 32], IntegrityProof>,
}

impl ArtifactStore {
    pub fn new() -> Self {
        Self {
            artifacts: HashMap::new(),
            index: HashMap::new(),
            metadata: HashMap::new(),
            integrity_proofs: HashMap::new(),
        }
    }

    pub fn store(&mut self, id: [u8; 32], content_hash: [u8; 32], size: u64) -> &Artifact {
        let artifact = Artifact {
            id,
            content_hash,
            size,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            pinned: false,
            replication_factor: 3,
            storage_tier: StorageTier::Hot,
            access_count: 0,
            last_accessed: 0,
        };
        self.artifacts.insert(id, artifact);
        self.index.entry(content_hash).or_default().push(id);
        self.artifacts.get(&id).unwrap()
    }

    pub fn store_with_metadata(
        &mut self,
        id: [u8; 32],
        content_hash: [u8; 32],
        size: u64,
        metadata: ArtifactMetadata,
    ) -> Result<&Artifact, String> {
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let artifact = Artifact {
            id,
            content_hash,
            size,
            created_at,
            pinned: false,
            replication_factor: 3,
            storage_tier: StorageTier::Hot,
            access_count: 0,
            last_accessed: created_at,
        };

        self.artifacts.insert(id, artifact);
        self.index.entry(content_hash).or_default().push(id);
        self.metadata.insert(id, metadata);
        self.integrity_proofs.insert(
            id,
            IntegrityProof {
                artifact_id: id,
                content_hash,
                size,
                proof_data: vec![0u8; 8],
                timestamp: created_at,
            },
        );

        self.artifacts
            .get(&id)
            .ok_or("Artifact not found".to_string())
    }

    pub fn get(&self, id: &[u8; 32]) -> Option<&Artifact> {
        self.artifacts.get(id)
    }

    pub fn get_mut(&mut self, id: &[u8; 32]) -> Option<&mut Artifact> {
        self.artifacts.get_mut(id)
    }

    pub fn find_by_hash(&self, content_hash: &[u8; 32]) -> Vec<&Artifact> {
        self.index
            .get(content_hash)
            .map(|ids| ids.iter().filter_map(|id| self.artifacts.get(id)).collect())
            .unwrap_or_default()
    }

    pub fn pin(&mut self, id: &[u8; 32]) -> Result<(), String> {
        if let Some(artifact) = self.artifacts.get_mut(id) {
            artifact.pinned = true;
            Ok(())
        } else {
            Err("Artifact not found".to_string())
        }
    }

    pub fn unpin(&mut self, id: &[u8; 32]) -> Result<(), String> {
        if let Some(artifact) = self.artifacts.get_mut(id) {
            artifact.pinned = false;
            Ok(())
        } else {
            Err("Artifact not found".to_string())
        }
    }

    pub fn set_storage_tier(&mut self, id: &[u8; 32], tier: StorageTier) -> Result<(), String> {
        if let Some(artifact) = self.artifacts.get_mut(id) {
            artifact.storage_tier = tier;
            Ok(())
        } else {
            Err("Artifact not found".to_string())
        }
    }

    pub fn get_by_tier(&self, tier: StorageTier) -> Vec<&Artifact> {
        self.artifacts
            .values()
            .filter(|a| a.storage_tier == tier)
            .collect()
    }

    pub fn update_access(&mut self, id: &[u8; 32]) -> Result<(), String> {
        if let Some(artifact) = self.artifacts.get_mut(id) {
            artifact.access_count += 1;
            artifact.last_accessed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            Ok(())
        } else {
            Err("Artifact not found".to_string())
        }
    }

    pub fn get_metadata(&self, id: &[u8; 32]) -> Option<&ArtifactMetadata> {
        self.metadata.get(id)
    }

    pub fn verify_integrity(&self, id: &[u8; 32]) -> bool {
        if let Some(artifact) = self.artifacts.get(id) {
            if let Some(proof) = self.integrity_proofs.get(id) {
                return proof.verify() && proof.content_hash == artifact.content_hash;
            }
        }
        false
    }

    pub fn get_integrity_proof(&self, id: &[u8; 32]) -> Option<&IntegrityProof> {
        self.integrity_proofs.get(id)
    }

    pub fn evict(&mut self, id: &[u8; 32]) -> Result<Artifact, String> {
        if let Some(artifact) = self.artifacts.remove(id) {
            if let Some(ids) = self.index.get_mut(&artifact.content_hash) {
                let id_array: [u8; 32] = *id;
                ids.retain(|i| *i != id_array);
            }
            self.metadata.remove(id);
            self.integrity_proofs.remove(id);
            Ok(artifact)
        } else {
            Err("Artifact not found".to_string())
        }
    }

    pub fn list_pinned(&self) -> Vec<&Artifact> {
        self.artifacts.values().filter(|a| a.pinned).collect()
    }

    pub fn list_unpinned(&self) -> Vec<&Artifact> {
        self.artifacts.values().filter(|a| !a.pinned).collect()
    }

    pub fn get_eviction_candidates(&self, count: usize) -> Vec<[u8; 32]> {
        let mut candidates: Vec<_> = self
            .artifacts
            .iter()
            .filter(|(_, a)| !a.pinned && a.storage_tier == StorageTier::Hot)
            .map(|(id, a)| (id, a.last_accessed, a.access_count))
            .collect();

        candidates.sort_by(|a, b| a.1.cmp(&b.1));
        candidates
            .into_iter()
            .take(count)
            .map(|(id, _, _)| *id)
            .collect()
    }

    pub fn total_size(&self) -> u64 {
        self.artifacts.values().map(|a| a.size).sum()
    }

    pub fn count(&self) -> usize {
        self.artifacts.len()
    }
}

impl Default for ArtifactStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifact_store() {
        let mut store = ArtifactStore::new();
        let id = [1u8; 32];
        let content_hash = [2u8; 32];

        store.store(id, content_hash, 100);

        let artifact = store.get(&id).unwrap();
        assert_eq!(artifact.size, 100);
        assert_eq!(artifact.pinned, false);
    }

    #[test]
    fn test_pin_unpin() {
        let mut store = ArtifactStore::new();
        let id = [1u8; 32];

        store.store(id, [2u8; 32], 100);

        store.pin(&id).unwrap();
        assert!(store.get(&id).unwrap().pinned);

        store.unpin(&id).unwrap();
        assert!(!store.get(&id).unwrap().pinned);
    }

    #[test]
    fn test_storage_tier() {
        let mut store = ArtifactStore::new();
        let id = [1u8; 32];

        store.store(id, [2u8; 32], 100);
        store.set_storage_tier(&id, StorageTier::Cold).unwrap();

        assert_eq!(store.get(&id).unwrap().storage_tier, StorageTier::Cold);
    }

    #[test]
    fn test_find_by_hash() {
        let mut store = ArtifactStore::new();
        let content_hash = [1u8; 32];

        store.store([1u8; 32], content_hash, 100);
        store.store([2u8; 32], content_hash, 200);

        let results = store.find_by_hash(&content_hash);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_eviction_candidates() {
        let mut store = ArtifactStore::new();

        store.store([1u8; 32], [1u8; 32], 100);
        store.store([2u8; 32], [2u8; 32], 100);
        store.pin(&[1u8; 32]).unwrap();

        let candidates = store.get_eviction_candidates(2);
        assert!(candidates.contains(&[2u8; 32]));
    }
}
