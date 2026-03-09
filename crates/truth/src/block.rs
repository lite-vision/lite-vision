use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub signatures: Vec<ValidatorSignature>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub height: u64,
    pub timestamp: u64,
    pub parent_hash: [u8; 32],
    pub state_root: [u8; 32],
    pub receipts_root: [u8; 32],
    pub validator_set_hash: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: [u8; 32],
    pub sender: [u8; 32],
    pub payload: Vec<u8>,
    pub nonce: u64,
    pub fee: u64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSignature {
    pub validator_id: [u8; 32],
    pub signature: Vec<u8>,
}

#[derive(Error, Debug)]
pub enum BlockError {
    #[error("Invalid block height")]
    InvalidHeight,
    #[error("Invalid parent hash")]
    InvalidParentHash,
    #[error("Invalid transaction")]
    InvalidTransaction(String),
    #[error("Signature verification failed")]
    InvalidSignature,
}

impl Block {
    pub fn new(header: BlockHeader, transactions: Vec<Transaction>) -> Self {
        Self {
            header,
            transactions,
            signatures: Vec::new(),
        }
    }

    pub fn hash(&self) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&bincode::serialize(self).unwrap());
        *hasher.finalize().as_bytes()
    }
}
