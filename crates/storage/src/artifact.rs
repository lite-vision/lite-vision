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
}

pub struct ArtifactStore {
    artifacts: HashMap<[u8; 32], Artifact>,
    index: HashMap<[u8; 32], Vec<[u8; 32]>>,
}

impl ArtifactStore {
    pub fn new() -> Self {
        Self {
            artifacts: HashMap::new(),
            index: HashMap::new(),
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
        };
        self.artifacts.insert(id, artifact);
        self.index.entry(content_hash).or_default().push(id);
        self.artifacts.get(&id).unwrap()
    }

    pub fn get(&self, id: &[u8; 32]) -> Option<&Artifact> {
        self.artifacts.get(id)
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
}

impl Default for ArtifactStore {
    fn default() -> Self {
        Self::new()
    }
}
