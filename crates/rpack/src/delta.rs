use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RPackDelta {
    pub base_id: [u8; 32],
    pub target_id: [u8; 32],
    pub version: u32,
    pub patches: Vec<Patch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    pub patch_type: PatchType,
    pub path: String,
    pub operation: PatchOperation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatchType {
    Object,
    Property,
    Animation,
    Asset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatchOperation {
    Add(Vec<u8>),
    Remove,
    Modify(Vec<u8>),
}

impl RPackDelta {
    pub fn new(base_id: [u8; 32]) -> Self {
        Self {
            base_id,
            target_id: [0u8; 32],
            version: 1,
            patches: Vec::new(),
        }
    }

    pub fn add_patch(&mut self, patch: Patch) {
        self.patches.push(patch);
    }

    pub fn apply(&self, base: &super::RPack) -> Result<super::RPack, String> {
        let mut result = base.clone();

        for patch in &self.patches {
            match patch.operation {
                PatchOperation::Add(_) | PatchOperation::Modify(_) => {}
                PatchOperation::Remove => {}
            }
        }

        Ok(result)
    }

    pub fn target_id(&self) -> [u8; 32] {
        *blake3::hash(&bincode::serialize(self).unwrap()).as_bytes()
    }
}
