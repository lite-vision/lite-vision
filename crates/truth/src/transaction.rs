use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Transaction Types - AI World Model Edition
/// Replaces blockchain transaction types with settlement-focused types
/// that don't assume EVM-style contract execution

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    // Job Lifecycle - submit, settle jobs
    JobSubmit,
    JobSettle,

    // Artifact management - content-addressed storage
    ArtifactCommit,

    // Operator management - reputation tracking
    OperatorRegister,
    OperatorUpdate,
    ReputationUpdate,

    // Settlement - economic primitives (not fees!)
    EscrowDeposit,
    EscrowRelease,
    EscrowRefund,

    // Governance (limited, not general)
    GovernanceVote,

    // DEPRECATED - kept for compatibility
    #[allow(dead_code)]
    Transfer,
    #[allow(dead_code)]
    ContractDeploy,
    #[allow(dead_code)]
    ContractCall,
    #[allow(dead_code)]
    IntelligenceSubmit,
    #[allow(dead_code)]
    IntelligenceSettle,
}

#[derive(Error, Debug)]
pub enum TxError {
    #[error("Invalid transaction format")]
    InvalidFormat,

    #[error("Transaction type not supported")]
    UnsupportedTxType,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid sender for transaction type")]
    InvalidSender,

    #[error("Invalid payload for transaction type")]
    InvalidPayload,

    #[error("Transaction expired")]
    Expired,
}

impl Transaction {
    pub fn verify(&self) -> Result<(), TxError> {
        use crate::transaction::TransactionType::*;

        match &self.tx_type {
            // AI-specific transactions don't require fees
            JobSubmit | JobSettle | ArtifactCommit | OperatorRegister | OperatorUpdate
            | ReputationUpdate | EscrowDeposit | EscrowRelease | EscrowRefund | GovernanceVote => {
                // Just validate payload is not empty
                if self.payload.is_empty() {
                    return Err(TxError::InvalidPayload);
                }
                Ok(())
            }
            // Legacy types still need some validation
            Transfer | ContractDeploy | ContractCall | IntelligenceSubmit | IntelligenceSettle => {
                // Backward compatibility
                if self.payload.is_empty() && !matches!(self.tx_type, Transfer) {
                    return Err(TxError::InvalidFormat);
                }
                Ok(())
            }
        }
    }

    pub fn is_settlement_type(&self) -> bool {
        use crate::transaction::TransactionType::*;
        matches!(
            self.tx_type,
            JobSubmit
                | JobSettle
                | ArtifactCommit
                | OperatorRegister
                | OperatorUpdate
                | ReputationUpdate
                | EscrowDeposit
                | EscrowRelease
                | EscrowRefund
        )
    }

    /// Verify the transaction signature authorizes the sender
    /// Returns Err(TxError::InvalidSignature) if verification fails
    pub fn verify_authorization(&self, pubkey: &[u8; 32]) -> Result<(), TxError> {
        // Empty signature is treated as invalid (requires authorization)
        if self.signature.is_empty() {
            return Err(TxError::InvalidSignature);
        }

        // Need at least 64 bytes for a valid signature
        if self.signature.len() != 64 {
            return Err(TxError::InvalidSignature);
        }

        // Convert signature to fixed array
        let sig_array: [u8; 64] = match self.signature.as_slice().try_into() {
            Ok(arr) => arr,
            Err(_) => return Err(TxError::InvalidSignature),
        };

        // Create message to verify: hash of (sender || tx_type || payload || nonce)
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&self.sender);
        hasher.update(&self.nonce.to_le_bytes());
        hasher.update(&self.payload);
        let message = hasher.finalize();

        // Verify signature using the provided public key
        let vk = match ed25519_dalek::VerifyingKey::from_bytes(pubkey.into()) {
            Ok(vk) => vk,
            Err(_) => return Err(TxError::InvalidSignature),
        };

        let sig = match ed25519_dalek::Signature::from_slice(&sig_array) {
            Ok(sig) => sig,
            Err(_) => return Err(TxError::InvalidSignature),
        };

        if vk.verify(message.as_bytes(), &sig).is_ok() {
            Ok(())
        } else {
            Err(TxError::InvalidSignature)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: [u8; 32],
    pub sender: [u8; 32],
    pub tx_type: TransactionType,
    pub payload: Vec<u8>,
    pub nonce: u64,
    pub fee: u64, // Kept for compatibility but not required
    pub signature: Vec<u8>,
}

impl Transaction {
    pub fn new_job_submit(
        job_id: [u8; 32],
        kernel_id: [u8; 32],
        budget: u64,
        deadline: u64,
    ) -> Self {
        let mut payload = Vec::new();
        payload.extend_from_slice(&job_id);
        payload.extend_from_slice(&kernel_id);
        payload.extend_from_slice(&budget.to_le_bytes());
        payload.extend_from_slice(&deadline.to_le_bytes());

        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let id = *hasher.finalize().as_bytes();

        Self {
            id,
            sender: [0u8; 32], // Client ID
            tx_type: TransactionType::JobSubmit,
            payload,
            nonce: 0,
            fee: 0, // No fee for settlement
            signature: Vec::new(),
        }
    }

    pub fn new_job_settle(receipt_id: [u8; 32], output_hash: [u8; 32]) -> Self {
        let mut payload = Vec::new();
        payload.extend_from_slice(&receipt_id);
        payload.extend_from_slice(&output_hash);

        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let id = *hasher.finalize().as_bytes();

        Self {
            id,
            sender: [0u8; 32], // Operator ID
            tx_type: TransactionType::JobSettle,
            payload,
            nonce: 0,
            fee: 0,
            signature: Vec::new(),
        }
    }

    pub fn new_artifact_commit(content_hash: [u8; 32], size_bytes: u64) -> Self {
        let mut payload = Vec::new();
        payload.extend_from_slice(&content_hash);
        payload.extend_from_slice(&size_bytes.to_le_bytes());

        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let id = *hasher.finalize().as_bytes();

        Self {
            id,
            sender: [0u8; 32], // Creator
            tx_type: TransactionType::ArtifactCommit,
            payload,
            nonce: 0,
            fee: 0,
            signature: Vec::new(),
        }
    }

    pub fn new_operator_register(
        operator_id: [u8; 32],
        pubkey: [u8; 32],
        stake: u64,
        region: String,
    ) -> Self {
        let mut payload = Vec::new();
        payload.extend_from_slice(&operator_id);
        payload.extend_from_slice(&pubkey);
        payload.extend_from_slice(&stake.to_le_bytes());
        payload.extend_from_slice(region.as_bytes());

        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let id = *hasher.finalize().as_bytes();

        Self {
            id,
            sender: operator_id,
            tx_type: TransactionType::OperatorRegister,
            payload,
            nonce: 0,
            fee: stake, // Stake serves as registration deposit
            signature: Vec::new(),
        }
    }

    pub fn new_reputation_update(target_operator: [u8; 32], delta: i64) -> Self {
        let mut payload = Vec::new();
        payload.extend_from_slice(&target_operator);
        payload.extend_from_slice(&delta.to_le_bytes());

        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let id = *hasher.finalize().as_bytes();

        Self {
            id,
            sender: [0u8; 32], // Verifier or system
            tx_type: TransactionType::ReputationUpdate,
            payload,
            nonce: 0,
            fee: 0,
            signature: Vec::new(),
        }
    }

    pub fn new_escrow_deposit(job_id: [u8; 32], amount: u64) -> Self {
        let mut payload = Vec::new();
        payload.extend_from_slice(&job_id);
        payload.extend_from_slice(&amount.to_le_bytes());

        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let id = *hasher.finalize().as_bytes();

        Self {
            id,
            sender: [0u8; 32], // Client
            tx_type: TransactionType::EscrowDeposit,
            payload,
            nonce: 0,
            fee: amount, // Deposit is the amount
            signature: Vec::new(),
        }
    }

    pub fn new_escrow_release(job_id: [u8; 32], recipient: [u8; 32]) -> Self {
        let mut payload = Vec::new();
        payload.extend_from_slice(&job_id);
        payload.extend_from_slice(&recipient);

        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let id = *hasher.finalize().as_bytes();

        Self {
            id,
            sender: [0u8; 32], // System/verifier
            tx_type: TransactionType::EscrowRelease,
            payload,
            nonce: 0,
            fee: 0,
            signature: Vec::new(),
        }
    }

    pub fn new_escrow_refund(job_id: [u8; 32]) -> Self {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&job_id);
        let id = *hasher.finalize().as_bytes();

        Self {
            id,
            sender: [0u8; 32],
            tx_type: TransactionType::EscrowRefund,
            payload: job_id.to_vec(),
            nonce: 0,
            fee: 0,
            signature: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_submit_transaction() {
        let tx = Transaction::new_job_submit([1u8; 32], [2u8; 32], 1000, 1000);

        assert!(matches!(tx.tx_type, TransactionType::JobSubmit));
        assert!(tx.is_settlement_type());
    }

    #[test]
    fn test_job_settle_transaction() {
        let tx = Transaction::new_job_settle([1u8; 32], [2u8; 32]);

        assert!(matches!(tx.tx_type, TransactionType::JobSettle));
    }

    #[test]
    fn test_artifact_commit_transaction() {
        let tx = Transaction::new_artifact_commit([1u8; 32], 1024);

        assert!(matches!(tx.tx_type, TransactionType::ArtifactCommit));
    }

    #[test]
    fn test_operator_register_transaction() {
        let tx =
            Transaction::new_operator_register([1u8; 32], [2u8; 32], 5000, "us-east".to_string());

        assert!(matches!(tx.tx_type, TransactionType::OperatorRegister));
        assert_eq!(tx.fee, 5000); // Fee is stake
    }

    #[test]
    fn test_reputation_update_transaction() {
        let tx = Transaction::new_reputation_update([1u8; 32], 100);

        assert!(matches!(tx.tx_type, TransactionType::ReputationUpdate));
    }

    #[test]
    fn test_escrow_deposit_transaction() {
        let tx = Transaction::new_escrow_deposit([1u8; 32], 1000);

        assert!(matches!(tx.tx_type, TransactionType::EscrowDeposit));
        assert_eq!(tx.fee, 1000); // Fee is deposit amount
    }

    #[test]
    fn test_escrow_release_transaction() {
        let tx = Transaction::new_escrow_release([1u8; 32], [2u8; 32]);

        assert!(matches!(tx.tx_type, TransactionType::EscrowRelease));
    }

    #[test]
    fn test_escrow_refund_transaction() {
        let tx = Transaction::new_escrow_refund([1u8; 32]);

        assert!(matches!(tx.tx_type, TransactionType::EscrowRefund));
    }

    #[test]
    fn test_transaction_verify_settlement() {
        let tx = Transaction::new_job_submit([1u8; 32], [2u8; 32], 1000, 1000);

        assert!(tx.verify().is_ok());
    }

    #[test]
    fn test_transaction_verify_invalid() {
        let tx = Transaction {
            id: [0u8; 32],
            sender: [0u8; 32],
            tx_type: TransactionType::JobSubmit,
            payload: Vec::new(), // Empty payload - invalid
            nonce: 0,
            fee: 0,
            signature: Vec::new(),
        };

        assert!(tx.verify().is_err());
    }
}
