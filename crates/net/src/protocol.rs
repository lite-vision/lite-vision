use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageType {
    Handshake,
    Block,
    Transaction,
    Vote,
    Intelligence,
    Ping,
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub msg_type: MessageType,
    pub sender_id: [u8; 32],
    pub payload: Vec<u8>,
    pub timestamp: u64,
    pub signature: Option<Vec<u8>>,
}

impl ProtocolMessage {
    pub fn new(msg_type: MessageType, sender_id: [u8; 32], payload: Vec<u8>) -> Self {
        Self {
            msg_type,
            sender_id,
            payload,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: None,
        }
    }

    pub fn sign(&mut self, signature: Vec<u8>) {
        self.signature = Some(signature);
    }
}
