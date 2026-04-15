use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::block::Block;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    pub start_height: u64,
    pub end_height: u64,
    pub peer_id: [u8; 32],
    pub request_type: SyncRequestType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncRequestType {
    Headers,
    Blocks,
    State,
    All,
}

impl SyncRequest {
    pub fn new(
        start_height: u64,
        end_height: u64,
        peer_id: [u8; 32],
        request_type: SyncRequestType,
    ) -> Self {
        Self {
            start_height,
            end_height,
            peer_id,
            request_type,
        }
    }

    pub fn request_headers(height: u64, peer_id: [u8; 32]) -> Self {
        Self {
            start_height: height,
            end_height: height + 100,
            peer_id,
            request_type: SyncRequestType::Headers,
        }
    }

    pub fn request_state(height: u64, peer_id: [u8; 32]) -> Self {
        Self {
            start_height: height,
            end_height: height,
            peer_id,
            request_type: SyncRequestType::State,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResponse {
    pub request: SyncRequest,
    pub headers: Vec<SyncHeader>,
    pub blocks: Vec<Block>,
    pub state_summary: StateSummary,
    pub complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncHeader {
    pub height: u64,
    pub hash: [u8; 32],
    pub parent_hash: [u8; 32],
    pub timestamp: u64,
    pub validator_set_hash: [u8; 32],
}

impl SyncHeader {
    pub fn from_block(block: &Block) -> Self {
        Self {
            height: block.header.height,
            hash: block.hash(),
            parent_hash: block.header.parent_hash,
            timestamp: block.header.timestamp,
            validator_set_hash: block.header.validator_set_hash,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StateSummary {
    pub height: u64,
    pub root_hash: [u8; 32],
    pub num_accounts: u64,
    pub num_receipts: u64,
    pub total_stake: u64,
}

pub struct StateSync {
    pending_requests: HashMap<u64, SyncRequest>,
    received_headers: Vec<SyncHeader>,
    received_blocks: Vec<Block>,
}

impl StateSync {
    pub fn new() -> Self {
        Self {
            pending_requests: HashMap::new(),
            received_headers: Vec::new(),
            received_blocks: Vec::new(),
        }
    }

    pub fn request_sync(&mut self, height: u64, peer_id: [u8; 32]) -> SyncRequest {
        let request = SyncRequest::request_headers(height, peer_id);
        self.pending_requests.insert(height, request.clone());
        request
    }

    pub fn process_headers(&mut self, headers: Vec<SyncHeader>) -> Vec<u64> {
        let mut heights = Vec::new();

        for header in headers {
            if !self
                .received_headers
                .iter()
                .any(|h| h.height == header.height)
            {
                self.received_headers.push(header.clone());
                heights.push(header.height);
            }
        }

        heights.sort();
        heights
    }

    pub fn process_blocks(&mut self, blocks: Vec<Block>) -> Vec<u64> {
        let mut heights = Vec::new();

        for block in blocks {
            if !self
                .received_blocks
                .iter()
                .any(|b| b.header.height == block.header.height)
            {
                self.received_blocks.push(block.clone());
                heights.push(block.header.height);
            }
        }

        heights.sort();
        heights
    }

    pub fn get_missing_heights(&self, from_height: u64, to_height: u64) -> Vec<u64> {
        let received: std::collections::HashSet<u64> =
            self.received_headers.iter().map(|h| h.height).collect();

        (from_height..=to_height)
            .filter(|h| !received.contains(h))
            .collect()
    }

    pub fn is_synced(&self, target_height: u64) -> bool {
        let max_received = self
            .received_headers
            .iter()
            .map(|h| h.height)
            .max()
            .unwrap_or(0);

        max_received >= target_height
    }

    pub fn clear(&mut self) {
        self.received_headers.clear();
        self.received_blocks.clear();
        self.pending_requests.clear();
    }
}

impl Default for StateSync {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LightState {
    pub height: u64,
    pub hash: [u8; 32],
    pub state_root: [u8; 32],
    pub validator_set_hash: [u8; 32],
    pub app_hash: [u8; 32],
}

impl LightState {
    pub fn new(height: u64, block: &Block, app_hash: [u8; 32]) -> Self {
        Self {
            height: height,
            hash: block.hash(),
            state_root: block.header.state_root,
            validator_set_hash: block.header.validator_set_hash,
            app_hash,
        }
    }

    pub fn verify(&self, other: &LightState) -> bool {
        self.height == other.height
            && self.hash == other.hash
            && self.state_root == other.state_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_request() {
        let request = SyncRequest::request_headers(100, [1u8; 32]);

        assert_eq!(request.start_height, 100);
        assert_eq!(request.request_type, SyncRequestType::Headers);
    }

    #[test]
    fn test_state_sync() {
        let mut sync = StateSync::new();

        let request = sync.request_sync(50, [1u8; 32]);
        assert!(sync.pending_requests.contains_key(&50));

        let headers = vec![SyncHeader {
            height: 50,
            hash: [1u8; 32],
            parent_hash: [0u8; 32],
            timestamp: 100,
            validator_set_hash: [2u8; 32],
        }];

        let heights = sync.process_headers(headers);
        assert_eq!(heights.len(), 1);
    }

    #[test]
    fn test_missing_heights() {
        let mut sync = StateSync::new();

        sync.received_headers.push(SyncHeader {
            height: 100,
            hash: [1u8; 32],
            parent_hash: [0u8; 32],
            timestamp: 100,
            validator_set_hash: [2u8; 32],
        });

        let missing = sync.get_missing_heights(50, 150);

        assert!(missing.contains(&50));
        assert!(missing.contains(&75));
        assert!(!missing.contains(&100));
    }

    #[test]
    fn test_is_synced() {
        let mut sync = StateSync::new();

        sync.received_headers.push(SyncHeader {
            height: 100,
            hash: [1u8; 32],
            parent_hash: [0u8; 32],
            timestamp: 100,
            validator_set_hash: [2u8; 32],
        });

        assert!(sync.is_synced(50));
        assert!(!sync.is_synced(150));
    }

    #[test]
    fn test_light_state() {
        let header = crate::block::BlockHeader {
            height: 1,
            timestamp: 100,
            parent_hash: [0u8; 32],
            state_root: [1u8; 32],
            receipts_root: [2u8; 32],
            validator_set_hash: [3u8; 32],
        };

        let block = crate::block::Block::new(header, vec![]);
        let light = LightState::new(1, &block, [4u8; 32]);

        assert_eq!(light.height, 1);
    }
}
