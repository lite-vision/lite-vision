use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub id: [u8; 32],
    pub job_id: [u8; 32],
    pub operator_id: [u8; 32],
    pub kernel_id: [u8; 32],
    pub input_hash: [u8; 32],
    pub output_hash: [u8; 32],
    pub compute_used: u64,
    pub fee: u64,
    pub timestamp: u64,
    pub signature: Vec<u8>,
    pub status: ReceiptStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReceiptStatus {
    Pending,
    Verified,
    Challenged,
    Slashed,
}

impl Receipt {
    pub fn new(
        job_id: [u8; 32],
        operator_id: [u8; 32],
        kernel_id: [u8; 32],
        input_hash: [u8; 32],
        output_hash: [u8; 32],
        compute_used: u64,
        fee: u64,
    ) -> Self {
        Self {
            id: blake3::hash(&bincode::serialize(&(job_id, operator_id)).unwrap())
                .as_bytes()
                .clone(),
            job_id,
            operator_id,
            kernel_id,
            input_hash,
            output_hash,
            compute_used,
            fee,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: Vec::new(),
            status: ReceiptStatus::Pending,
        }
    }

    pub fn sign(&mut self, signature: Vec<u8>) {
        self.signature = signature;
    }

    pub fn verify(&self) -> bool {
        !self.signature.iter().all(|&x| x == 0)
    }
}
