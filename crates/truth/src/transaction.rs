use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: [u8; 32],
    pub sender: [u8; 32],
    pub tx_type: TransactionType,
    pub payload: Vec<u8>,
    pub nonce: u64,
    pub fee: u64,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    Transfer,
    ContractDeploy,
    ContractCall,
    IntelligenceSubmit,
    IntelligenceSettle,
    GovernanceVote,
}

#[derive(Error, Debug)]
pub enum TxError {
    #[error("Invalid transaction format")]
    InvalidFormat,
    #[error("Insufficient fee")]
    InsufficientFee,
    #[error("Nonce too low")]
    NonceTooLow,
    #[error("Invalid signature")]
    InvalidSignature,
}

impl Transaction {
    pub fn verify(&self) -> Result<(), TxError> {
        if self.payload.is_empty() && !matches!(self.tx_type, TransactionType::Transfer) {
            return Err(TxError::InvalidFormat);
        }
        if self.fee == 0 {
            return Err(TxError::InsufficientFee);
        }
        Ok(())
    }
}
