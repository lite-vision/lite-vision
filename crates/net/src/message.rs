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
