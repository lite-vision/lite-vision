use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Storage {
    pub state: super::state::State,
    pub blocks: HashMap<u64, super::block::Block>,
    pub snapshots: Vec<Snapshot>,
    pub archival_height: u64,
    pub prune_height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub height: u64,
    pub state_root: [u8; 32],
    pub block_hash: [u8; 32],
}

impl Storage {
    pub fn new() -> Self {
        Self {
            state: super::state::State::new(),
            blocks: HashMap::new(),
            snapshots: Vec::new(),
            archival_height: 10000,
            prune_height: 1000,
        }
    }

    pub fn store_block(&mut self, block: super::block::Block) {
        self.blocks.insert(block.header.height, block);
    }

    pub fn get_block(&self, height: u64) -> Option<&super::block::Block> {
        self.blocks.get(&height)
    }

    pub fn create_snapshot(&mut self, height: u64, state_root: [u8; 32], block_hash: [u8; 32]) {
        let snapshot = Snapshot {
            height,
            state_root,
            block_hash,
        };
        self.snapshots.push(snapshot);

        if self.snapshots.len() > 100 {
            self.snapshots.remove(0);
        }
    }

    pub fn prune(&mut self, keep_height: u64) {
        self.blocks.retain(|h, _| *h >= keep_height);
    }

    pub fn can_prune(&self, height: u64) -> bool {
        let latest_snapshot = self.snapshots.last();
        match latest_snapshot {
            Some(s) => height < s.height && height < (s.height - self.prune_height),
            None => false,
        }
    }
}

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}
