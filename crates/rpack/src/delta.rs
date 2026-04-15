use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RPackDelta {
    pub base_id: [u8; 32],
    pub target_id: [u8; 32],
    pub version: u32,
    pub patches: Vec<Patch>,
    pub metadata: DeltaMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaMetadata {
    pub author: [u8; 32],
    pub timestamp: u64,
    pub description: String,
    pub prerequisites: Vec<[u8; 32]>,
}

impl Default for DeltaMetadata {
    fn default() -> Self {
        Self {
            author: [0u8; 32],
            timestamp: 0,
            description: String::new(),
            prerequisites: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    pub patch_type: PatchType,
    pub path: String,
    pub operation: PatchOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchType {
    Object,
    Property,
    Animation,
    Asset,
    Material,
}

impl PatchType {
    pub fn priority(&self) -> u8 {
        match self {
            PatchType::Object => 3,
            PatchType::Property => 2,
            PatchType::Animation => 1,
            PatchType::Asset => 0,
            PatchType::Material => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatchOperation {
    Add(Vec<u8>),
    Remove,
    Modify(Vec<u8>),
}

impl PatchOperation {
    pub fn data(&self) -> Option<&Vec<u8>> {
        match self {
            PatchOperation::Add(d) => Some(d),
            PatchOperation::Modify(d) => Some(d),
            PatchOperation::Remove => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchSet {
    patches: HashMap<String, Patch>,
}

impl PatchSet {
    pub fn new() -> Self {
        Self {
            patches: HashMap::new(),
        }
    }

    pub fn add(&mut self, patch: Patch) {
        self.patches.insert(patch.path.clone(), patch);
    }

    pub fn get(&self, path: &str) -> Option<&Patch> {
        self.patches.get(path)
    }

    pub fn remove(&mut self, path: &str) {
        self.patches.remove(path);
    }

    pub fn len(&self) -> usize {
        self.patches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }
}

impl Default for PatchSet {
    fn default() -> Self {
        Self::new()
    }
}

impl RPackDelta {
    pub fn new(base_id: [u8; 32]) -> Self {
        Self {
            base_id,
            target_id: [0u8; 32],
            version: 1,
            patches: Vec::new(),
            metadata: DeltaMetadata::default(),
        }
    }

    pub fn with_metadata(mut self, author: [u8; 32], description: String) -> Self {
        self.metadata.author = author;
        self.metadata.description = description;
        self
    }

    pub fn add_patch(&mut self, patch: Patch) {
        self.patches.push(patch);
    }

    pub fn compute_target_id(&mut self) -> [u8; 32] {
        self.target_id = *blake3::hash(&bincode::serialize(self).unwrap()).as_bytes();
        self.target_id
    }

    pub fn target_id(&self) -> [u8; 32] {
        self.target_id
    }

    pub fn apply(&self, base: &super::RPack) -> Option<super::RPack> {
        let mut result = base.clone();

        for patch in &self.patches {
            match patch.operation {
                PatchOperation::Add(_) => {}
                PatchOperation::Remove => {}
                PatchOperation::Modify(_) => {}
            }
        }

        result.header.scene_ir_hash = self.target_id;
        Some(result)
    }

    pub fn is_applicable(&self, base: &super::RPack) -> bool {
        base.header.scene_ir_hash == self.base_id
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn patch_count(&self) -> usize {
        self.patches.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_creation() {
        let delta = RPackDelta::new([1u8; 32]);
        assert_eq!(delta.base_id, [1u8; 32]);
        assert_eq!(delta.version, 1);
        assert!(delta.patches.is_empty());
    }

    #[test]
    fn test_delta_add_patch() {
        let mut delta = RPackDelta::new([1u8; 32]);

        let patch = Patch {
            patch_type: PatchType::Property,
            path: "/scene/nodes/0/transform".to_string(),
            operation: PatchOperation::Modify(vec![1, 2, 3]),
        };

        delta.add_patch(patch);

        assert_eq!(delta.patch_count(), 1);
    }

    #[test]
    fn test_patch_type_priority() {
        assert!(PatchType::Object.priority() > PatchType::Asset.priority());
        assert!(PatchType::Material.priority() == PatchType::Property.priority());
    }

    #[test]
    fn test_patch_set() {
        let mut set = PatchSet::new();

        let patch = Patch {
            patch_type: PatchType::Object,
            path: "/scene/nodes/0".to_string(),
            operation: PatchOperation::Add(vec![]),
        };

        set.add(patch.clone());

        assert_eq!(set.len(), 1);
        assert!(!set.is_empty());

        assert!(set.get("/scene/nodes/0").is_some());

        set.remove("/scene/nodes/0");

        assert!(set.is_empty());
    }

    #[test]
    fn test_delta_target_id_stability() {
        // Create fresh delta for each call to ensure determinism
        let create_delta = || {
            let mut delta = RPackDelta::new([1u8; 32]);
            delta.compute_target_id();
            delta
        };

        let id1 = create_delta().target_id();
        let id2 = create_delta().target_id();

        assert_eq!(id1, id2);
    }

    #[test]
    fn test_patch_operation_data() {
        let add = PatchOperation::Add(vec![1, 2, 3]);
        let modify = PatchOperation::Modify(vec![4, 5, 6]);
        let remove = PatchOperation::Remove;

        assert!(add.data().is_some());
        assert!(modify.data().is_some());
        assert!(remove.data().is_none());
    }
}
