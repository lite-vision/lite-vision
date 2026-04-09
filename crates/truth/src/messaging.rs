use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Block(BlockMessage),
    Vote(VoteMessage),
    Transaction(TxMessage),
    Intelligence(IntelligenceMessage),
    CrossPlane(CrossPlaneMessage),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockMessage {
    pub block: super::block::Block,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoteMessage {
    pub vote: super::consensus::Vote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxMessage {
    pub transaction: super::transaction::Transaction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IntelligenceMessage {
    JobSubmit(JobSubmit),
    JobResult(JobResult),
    ReceiptSubmit(ReceiptSubmit),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSubmit {
    pub job_id: [u8; 32],
    pub kernel_id: [u8; 32],
    pub input_hash: [u8; 32],
    pub budget: u64,
    pub deadline: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub job_id: [u8; 32],
    pub output_hash: [u8; 32],
    pub compute_used: u64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptSubmit {
    pub receipt: super::state::Receipt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrossPlaneMessage {
    Commitment(Commitment),
    Verification(Verification),
    Challenge(Challenge),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commitment {
    pub partition_id: u32,
    pub artifact_id: [u8; 32],
    pub commitment: [u8; 32],
    pub height: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verification {
    pub artifact_id: [u8; 32],
    pub verified: bool,
    pub verifier_id: [u8; 32],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Challenge {
    pub challenge_id: [u8; 32],
    pub artifact_id: [u8; 32],
    pub challenger_id: [u8; 32],
    pub bond: u64,
    pub reason: String,
}
