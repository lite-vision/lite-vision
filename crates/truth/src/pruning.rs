use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

const DEFAULT_RETENTION_LIMIT: u64 = 10000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruningConfig {
    pub enabled: bool,
    pub retention_blocks: u64,
    pub min_height_to_prune: u64,
    pub batch_size: usize,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_blocks: DEFAULT_RETENTION_LIMIT,
            min_height_to_prune: 100,
            batch_size: 1000,
        }
    }
}

impl PruningConfig {
    pub fn with_retention(mut self, blocks: u64) -> Self {
        self.retention_blocks = blocks;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockArchive {
    pub height: u64,
    pub hash: [u8; 32],
    pub state_root: [u8; 32],
    pub block_size_bytes: u64,
    pub tx_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveStats {
    pub total_blocks: u64,
    pub total_size_bytes: u64,
    pub earliest_height: u64,
    pub latest_height: u64,
}

pub struct StatePruner {
    config: PruningConfig,
    blocks: HashMap<u64, BlockArchive>,
    heights: VecDeque<u64>,
}

impl StatePruner {
    pub fn new(config: PruningConfig) -> Self {
        Self {
            config,
            blocks: HashMap::new(),
            heights: VecDeque::new(),
        }
    }

    pub fn archive_block(
        &mut self,
        height: u64,
        hash: [u8; 32],
        state_root: [u8; 32],
        block_size: u64,
        tx_count: u32,
    ) {
        let archive = BlockArchive {
            height,
            hash,
            state_root,
            block_size_bytes: block_size,
            tx_count,
        };

        self.blocks.insert(height, archive);

        if !self.heights.contains(&height) {
            self.heights.push_back(height);
        }
    }

    pub fn should_prune(&self, current_height: u64) -> bool {
        if !self.config.enabled {
            return false;
        }

        let min_height = current_height.saturating_sub(self.config.retention_blocks);
        min_height > self.config.min_height_to_prune
    }

    pub fn get_blocks_to_prune(&self, current_height: u64) -> Vec<u64> {
        if !self.should_prune(current_height) {
            return Vec::new();
        }

        let cutoff = current_height.saturating_sub(self.config.retention_blocks);

        self.heights
            .iter()
            .filter(|&&h| h < cutoff)
            .cloned()
            .collect()
    }

    pub fn prune(&mut self, heights: &[u64]) -> usize {
        let mut count = 0;

        for &height in heights {
            if self.blocks.remove(&height).is_some() {
                if let Some(pos) = self.heights.iter().position(|&h| h == height) {
                    self.heights.remove(pos);
                }
                count += 1;
            }
        }

        count
    }

    pub fn get_archive(&self, height: u64) -> Option<&BlockArchive> {
        self.blocks.get(&height)
    }

    pub fn get_stats(&self) -> ArchiveStats {
        let total_blocks = self.blocks.len() as u64;
        let total_size: u64 = self.blocks.values().map(|b| b.block_size_bytes).sum();

        let (earliest, latest) =
            if let (Some(&earliest), Some(&latest)) = (self.heights.front(), self.heights.back()) {
                (earliest, latest)
            } else {
                (0, 0)
            };

        ArchiveStats {
            total_blocks,
            total_size_bytes: total_size,
            earliest_height: earliest,
            latest_height: latest,
        }
    }

    pub fn verify_archive(&self, height: u64, expected_hash: [u8; 32]) -> bool {
        self.blocks
            .get(&height)
            .map(|a| a.hash == expected_hash)
            .unwrap_or(false)
    }

    pub fn retention_height(&self, current_height: u64) -> u64 {
        current_height.saturating_sub(self.config.retention_blocks)
    }
}

impl Default for StatePruner {
    fn default() -> Self {
        Self::new(PruningConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pruner_disabled() {
        let config = PruningConfig::default().disabled();
        let pruner = StatePruner::new(config);

        assert!(!pruner.should_prune(1000));
    }

    #[test]
    fn test_archive_block() {
        let mut pruner = StatePruner::new(PruningConfig::default());

        pruner.archive_block(100, [1u8; 32], [2u8; 32], 1000, 5);

        let archive = pruner.get_archive(100).unwrap();
        assert_eq!(archive.height, 100);
    }

    #[test]
    fn test_should_prune() {
        let config = PruningConfig::default().with_retention(100);
        let mut pruner = StatePruner::new(config);

        // Archive some blocks to have something to prune
        for height in 1..=50 {
            pruner.archive_block(height, [height as u8; 32], [0u8; 32], 100, 1);
        }

        assert!(!pruner.should_prune(50));
        // At height 201, with retention 100: min_height = 201-100 = 101 > 100, so should prune
        assert!(pruner.should_prune(201));
    }

    #[test]
    fn test_get_blocks_to_prune() {
        let config = PruningConfig::default().with_retention(10);
        let mut pruner = StatePruner::new(config);

        for i in 1..=20 {
            pruner.archive_block(i, [i as u8; 32], [0u8; 32], 100, 1);
        }

        let to_prune = pruner.get_blocks_to_prune(20);

        for h in to_prune {
            assert!(h < 10);
        }
    }

    #[test]
    fn test_prune() {
        let config = PruningConfig::default().with_retention(10);
        let mut pruner = StatePruner::new(config);

        pruner.archive_block(1, [1u8; 32], [0u8; 32], 100, 1);
        pruner.archive_block(2, [2u8; 32], [0u8; 32], 100, 1);

        let pruned = pruner.prune(&[1, 2]);

        assert_eq!(pruned, 2);
        assert!(pruner.get_archive(1).is_none());
    }

    #[test]
    fn test_stats() {
        let mut pruner = StatePruner::new(PruningConfig::default());

        pruner.archive_block(1, [1u8; 32], [0u8; 32], 100, 1);
        pruner.archive_block(2, [2u8; 32], [0u8; 32], 200, 2);

        let stats = pruner.get_stats();

        assert_eq!(stats.total_blocks, 2);
        assert_eq!(stats.total_size_bytes, 300);
    }

    #[test]
    fn test_retention_height() {
        let config = PruningConfig::default().with_retention(100);
        let pruner = StatePruner::new(config);

        assert_eq!(pruner.retention_height(200), 100);
    }
}
