use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMessage {
    pub topic: String,
    pub data: Vec<u8>,
    pub sender: [u8; 32],
    pub sequence: u64,
}

impl NetworkMessage {
    pub fn new(topic: String, data: Vec<u8>, sender: [u8; 32]) -> Self {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self {
            topic,
            data,
            sender,
            sequence: SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }
}

/// Job broadcast message for P2P job propagation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobBroadcastMessage {
    pub job_id: [u8; 32],
    pub data: Vec<u8>,
    pub origin: [u8; 32],
}

/// Receipt broadcast message for P2P receipt propagation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptBroadcastMessage {
    pub receipt_id: [u8; 32],
    pub data: Vec<u8>,
    pub origin: [u8; 32],
}

/// Discovery request message for peer discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryRequestMessage {
    pub requester_id: [u8; 32],
    pub seq: u64,
}

/// Discovery response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResponseMessage {
    pub responder_id: [u8; 32],
    pub seq: u64,
    pub peers: Vec<crate::p2p::PeerInfo>,
}

pub struct MessageQueue {
    topics: std::collections::HashMap<String, Vec<NetworkMessage>>,
}

impl MessageQueue {
    pub fn new() -> Self {
        Self {
            topics: std::collections::HashMap::new(),
        }
    }

    pub fn publish(&mut self, topic: String, message: NetworkMessage) {
        self.topics
            .entry(topic)
            .or_insert_with(Vec::new)
            .push(message);
    }

    pub fn subscribe(&self, topic: &str) -> Option<&Vec<NetworkMessage>> {
        self.topics.get(topic)
    }

    pub fn clear(&mut self) {
        self.topics.clear();
    }
}

impl Default for MessageQueue {
    fn default() -> Self {
        Self::new()
    }
}
