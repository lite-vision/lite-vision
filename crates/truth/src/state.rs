use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::block::Transaction;
use crate::settlement::{OperatorRegistrationStatus, OperatorState, SettlementState};
use crate::transaction::TransactionType;

pub const STATE_VERSION: u64 = 2; // Version 2 - Settlement-based

/// State - AI World Model Edition
/// Keeps: Block height, state root, validator set
/// Replaces: Account model with settlement primitives
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub version: u64,
    pub height: u64,
    pub state_root: [u8; 32],
    pub validator_set_root: [u8; 32],

    // Settlement Layer - Economic primitives for AI compute
    pub settlement: SettlementState,

    // Legacy receipts (for compatibility)
    pub intelligence_receipts: HashMap<[u8; 32], Receipt>,
}

/// Legacy Receipt - kept for compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Receipt {
    pub id: [u8; 32],
    pub job_id: [u8; 32],
    pub operator_id: [u8; 32],
    pub output_hash: [u8; 32],
    pub compute_used: u64,
    pub fee: u64,
    pub settled: bool,
}

impl Receipt {
    pub fn new(
        job_id: [u8; 32],
        operator_id: [u8; 32],
        output_hash: [u8; 32],
        compute_used: u64,
        fee: u64,
    ) -> Self {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&job_id);
        hasher.update(&operator_id);
        let id = *hasher.finalize().as_bytes();

        Self {
            id,
            job_id,
            operator_id,
            output_hash,
            compute_used,
            fee,
            settled: false,
        }
    }

    pub fn settle(&mut self) {
        self.settled = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StateError {
    AccountNotFound,     // Legacy
    InsufficientBalance, // Legacy
    InvalidNonce,        // Legacy
    InvalidTransaction,
    InvalidReceiver, // Legacy
    InvalidSender,   // Legacy
    OperatorNotFound,
    OperatorNotActive,
    EscrowNotFound,
    EscrowNotActive,
    InsufficientStake,
    ReceiptNotFound,
    OutsideChallengeWindow,
}

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::AccountNotFound => write!(f, "Account not found"),
            StateError::InsufficientBalance => write!(f, "Insufficient balance"),
            StateError::InvalidNonce => write!(f, "Invalid nonce"),
            StateError::InvalidTransaction => write!(f, "Invalid transaction"),
            StateError::InvalidReceiver => write!(f, "Invalid receiver"),
            StateError::InvalidSender => write!(f, "Invalid sender"),
            StateError::OperatorNotFound => write!(f, "Operator not found"),
            StateError::OperatorNotActive => write!(f, "Operator not active"),
            StateError::EscrowNotFound => write!(f, "Escrow not found"),
            StateError::EscrowNotActive => write!(f, "Escrow not active"),
            StateError::InsufficientStake => write!(f, "Insufficient stake"),
            StateError::ReceiptNotFound => write!(f, "Receipt not found"),
            StateError::OutsideChallengeWindow => write!(f, "Outside challenge window"),
        }
    }
}

impl std::error::Error for StateError {}

/// State Transition for block commits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub prev_state_hash: [u8; 32],
    pub block_hash: [u8; 32],
    pub transactions: Vec<[u8; 32]>,
    pub receipts: Vec<[u8; 32]>,
    pub state_hash: [u8; 32],
}

impl State {
    pub fn new() -> Self {
        Self {
            version: STATE_VERSION,
            height: 0,
            state_root: [0u8; 32],
            validator_set_root: [0u8; 32],
            settlement: SettlementState::new(),
            intelligence_receipts: HashMap::new(),
        }
    }

    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<Option<Receipt>, StateError> {
        // First verify the transaction format
        tx.verify().map_err(|_| StateError::InvalidTransaction)?;

        // For transactions that modify state (not just reads), verify authorization
        // Skip authorization for GovernanceVote and legacy Transfer types
        if !matches!(
            tx.tx_type,
            TransactionType::GovernanceVote | TransactionType::Transfer
        ) {
            // Get the sender's public key from the operator registry if they exist
            let sender_pubkey = self
                .settlement
                .operator_registry
                .get(&tx.sender)
                .map(|op| op.pubkey)
                .unwrap_or([0u8; 32]); // Default to zero if not registered

            // For transactions that require authorization (non-zero signature)
            // We verify the signature if provided
            if !tx.signature.is_empty() {
                // If sender is registered, verify their signature
                if sender_pubkey != [0u8; 32] {
                    tx.verify_authorization(&sender_pubkey)
                        .map_err(|_| StateError::InvalidTransaction)?;
                }
            }
        }

        match &tx.tx_type {
            // Job lifecycle
            TransactionType::JobSubmit => {
                self.apply_job_submit(tx)?;
            }
            TransactionType::JobSettle => {
                self.apply_job_settle(tx)?;
            }

            // Artifact management
            TransactionType::ArtifactCommit => {
                self.apply_artifact_commit(tx)?;
            }

            // Operator management
            TransactionType::OperatorRegister => {
                self.apply_operator_register(tx)?;
            }
            TransactionType::OperatorUpdate => {
                self.apply_operator_update(tx)?;
            }
            TransactionType::ReputationUpdate => {
                self.apply_reputation_update(tx)?;
            }

            // Escrow management
            TransactionType::EscrowDeposit => {
                self.apply_escrow_deposit(tx)?;
            }
            TransactionType::EscrowRelease => {
                self.apply_escrow_release(tx)?;
            }
            TransactionType::EscrowRefund => {
                self.apply_escrow_refund(tx)?;
            }

            // Governance (limited)
            TransactionType::GovernanceVote => {
                // Simplified governance - just validate
            }

            // Legacy types (for compatibility)
            TransactionType::Transfer => {
                // Skip - no longer used
            }
            TransactionType::ContractDeploy | TransactionType::ContractCall => {
                // Skip - no smart contracts
            }
            TransactionType::IntelligenceSubmit => {
                self.apply_intelligence_submit(tx)?;
            }
            TransactionType::IntelligenceSettle => {
                self.apply_intelligence_settle(tx)?;
            }
        }

        Ok(None)
    }

    fn apply_job_submit(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // Parse: job_id(32), kernel_id(32), budget(8), deadline(8)
        if tx.payload.len() < 80 {
            return Err(StateError::InvalidTransaction);
        }

        let job_id: [u8; 32] = tx.payload[0..32].try_into().unwrap();
        let budget = u64::from_le_bytes(tx.payload[64..72].try_into().unwrap());

        self.settlement.create_escrow(
            job_id,
            tx.sender,
            budget,
            self.height,
            500, // deadline blocks
        );

        Ok(())
    }

    fn apply_job_settle(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // Parse: receipt_id(32), output_hash(32)
        if tx.payload.len() < 64 {
            return Err(StateError::InvalidTransaction);
        }

        let job_id: [u8; 32] = tx.sender; // operator_id becomes job_id for linking

        // Release escrow to operator
        if let Some(escrow) = self.settlement.get_escrow_mut(&job_id) {
            escrow.release(tx.sender);
        }

        // Anchor receipt for verification
        let output_hash: [u8; 32] = tx.payload[32..64].try_into().unwrap();
        let receipt = crate::settlement::ReceiptAnchor::new(
            job_id,
            tx.sender,
            output_hash,
            [0u8; 32], // input_hash
            0,         // compute_used
            self.height,
        );
        self.settlement.anchor_receipt(receipt);

        Ok(())
    }

    fn apply_artifact_commit(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // Parse: content_hash(32), size_bytes(8)
        if tx.payload.len() < 40 {
            return Err(StateError::InvalidTransaction);
        }

        let _content_hash: [u8; 32] = tx.payload[0..32].try_into().unwrap();

        // Artifact commitment happens via state - the hash is stored
        // Actual content stored in separate storage layer

        Ok(())
    }

    fn apply_operator_register(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // Parse: operator_id(32), pubkey(32), stake(8), region(variable)
        if tx.payload.len() < 72 {
            return Err(StateError::InvalidTransaction);
        }

        let operator_id: [u8; 32] = tx.payload[0..32].try_into().unwrap();
        let pubkey: [u8; 32] = tx.payload[32..64].try_into().unwrap();
        let stake = u64::from_le_bytes(tx.payload[64..72].try_into().unwrap());

        // Fee serves as initial stake
        let actual_stake = tx.fee.max(stake);

        let operator = OperatorState {
            id: operator_id,
            pubkey,
            stake: actual_stake,
            reputation: 1000, // Initial reputation
            status: OperatorRegistrationStatus::Active,
            region: "global".to_string(),
            capabilities: vec![],
            slashed_count: 0,
            total_jobs_completed: 0,
            total_compute_units: 0,
            last_update_block: self.height,
        };

        self.settlement.operator_registry.register(operator);

        Ok(())
    }

    fn apply_operator_update(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // Parse: operator_id(32)
        if tx.payload.len() < 32 {
            return Err(StateError::InvalidTransaction);
        }

        let operator_id: [u8; 32] = tx.payload[0..32].try_into().unwrap();

        if !self.settlement.operator_registry.is_active(&operator_id) {
            return Err(StateError::OperatorNotActive);
        }

        Ok(())
    }

    fn apply_reputation_update(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // Parse: target_operator(32), delta(8)
        if tx.payload.len() < 40 {
            return Err(StateError::InvalidTransaction);
        }

        let target_operator: [u8; 32] = tx.payload[0..32].try_into().unwrap();
        let delta = i64::from_le_bytes(tx.payload[32..40].try_into().unwrap());

        if delta >= 0 {
            self.settlement
                .operator_registry
                .increase_reputation(&target_operator, delta as u64);
        } else {
            self.settlement
                .operator_registry
                .decrease_reputation(&target_operator, (-delta) as u64);
        }

        Ok(())
    }

    fn apply_escrow_deposit(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // Parse: job_id(32), amount(8)
        if tx.payload.len() < 40 {
            return Err(StateError::InvalidTransaction);
        }

        let job_id: [u8; 32] = tx.payload[0..32].try_into().unwrap();
        let amount = u64::from_le_bytes(tx.payload[32..40].try_into().unwrap());

        self.settlement
            .create_escrow(job_id, tx.sender, amount, self.height, 500);

        Ok(())
    }

    fn apply_escrow_release(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // Parse: job_id(32), recipient(32)
        if tx.payload.len() < 64 {
            return Err(StateError::InvalidTransaction);
        }

        let job_id: [u8; 32] = tx.payload[0..32].try_into().unwrap();
        let recipient: [u8; 32] = tx.payload[32..64].try_into().unwrap();

        self.settlement.release_escrow(&job_id, recipient)?;

        Ok(())
    }

    fn apply_escrow_refund(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // Parse: job_id(32)
        if tx.payload.len() < 32 {
            return Err(StateError::InvalidTransaction);
        }

        let job_id: [u8; 32] = tx.payload[0..32].try_into().unwrap();

        self.settlement.refund_escrow(&job_id)?;

        Ok(())
    }

    fn apply_intelligence_submit(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // Legacy compatibility
        let job_id = tx.sender;
        let operator_id: [u8; 32] = tx
            .payload
            .get(0..32)
            .map(|p| {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(p);
                arr
            })
            .unwrap_or([0u8; 32]);

        let compute_used: u64 = tx
            .payload
            .get(32..40)
            .map(|p| u64::from_le_bytes(p.try_into().unwrap()))
            .unwrap_or(0);

        let output_hash: [u8; 32] = tx
            .payload
            .get(40..72)
            .map(|p| {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(p);
                arr
            })
            .unwrap_or([0u8; 32]);

        let receipt = Receipt::new(job_id, operator_id, output_hash, compute_used, tx.fee);
        let receipt_id = receipt.id;

        self.intelligence_receipts.insert(receipt_id, receipt);

        Ok(())
    }

    fn apply_intelligence_settle(&mut self, tx: &Transaction) -> Result<(), StateError> {
        // Legacy compatibility
        let receipt_id: [u8; 32] = tx
            .payload
            .get(0..32)
            .map(|p| {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(p);
                arr
            })
            .ok_or(StateError::InvalidTransaction)?;

        let receipt = self
            .intelligence_receipts
            .get_mut(&receipt_id)
            .ok_or(StateError::InvalidTransaction)?;

        if !receipt.settled {
            receipt.settle();
        }

        Ok(())
    }

    pub fn apply_block_transactions(
        &mut self,
        txs: &[Transaction],
    ) -> Result<Vec<Option<Receipt>>, StateError> {
        let mut results = Vec::new();

        for tx in txs {
            match self.apply_transaction(tx) {
                Ok(receipt) => results.push(receipt),
                Err(e) => return Err(e),
            }
        }

        Ok(results)
    }

    pub fn root_hash(&self) -> [u8; 32] {
        use blake3::Hasher;

        let mut hasher = Hasher::new();
        hasher.update(&self.version.to_le_bytes());
        hasher.update(&self.height.to_le_bytes());
        hasher.update(&self.validator_set_root);

        // Include settlement state
        hasher.update(&(self.settlement.job_escrows.len() as u64).to_le_bytes());
        hasher.update(&(self.settlement.operator_registry.operators.len() as u64).to_le_bytes());
        hasher.update(&(self.settlement.receipt_anchors.len() as u64).to_le_bytes());

        // Hash escrow keys
        let mut escrow_keys: Vec<_> = self.settlement.job_escrows.keys().collect();
        escrow_keys.sort();
        for key in escrow_keys {
            hasher.update(key);
        }

        // Hash operator IDs
        let mut operator_ids: Vec<_> = self.settlement.operator_registry.operators.keys().collect();
        operator_ids.sort();
        for id in operator_ids {
            hasher.update(id);
        }

        *hasher.finalize().as_bytes()
    }

    /// Compute root hash and update state_root
    pub fn compute_state_root(&mut self) -> [u8; 32] {
        let hash = self.root_hash();
        self.state_root = hash;
        hash
    }

    pub fn increment_height(&mut self) {
        self.height += 1;
    }

    pub fn set_validator_set_root(&mut self, root: [u8; 32]) {
        self.validator_set_root = root;
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_submit() {
        let mut state = State::new();

        let tx = Transaction::new_job_submit([1u8; 32], [2u8; 32], 1000, 100);

        // Apply the transaction
        state.apply_transaction(&tx).unwrap();

        // Verify escrow was created - there should be 1 escrow
        assert_eq!(state.settlement.job_escrows.len(), 1);

        // Get the escrow and verify the amount
        let escrow = state.settlement.job_escrows.values().next().unwrap();
        assert_eq!(escrow.amount, 1000);
    }

    #[test]
    fn test_operator_register() {
        let mut state = State::new();

        let tx =
            Transaction::new_operator_register([1u8; 32], [2u8; 32], 5000, "us-east".to_string());

        state.apply_transaction(&tx).unwrap();

        assert!(state.settlement.operator_registry.is_active(&[1u8; 32]));
    }

    #[test]
    fn test_reputation_update() {
        let mut state = State::new();

        // Register operator first
        let reg_tx =
            Transaction::new_operator_register([1u8; 32], [2u8; 32], 5000, "us-east".to_string());
        state.apply_transaction(&reg_tx).unwrap();

        // Update reputation
        let rep_tx = Transaction::new_reputation_update([1u8; 32], 500);
        state.apply_transaction(&rep_tx).unwrap();

        let op = state.settlement.operator_registry.get(&[1u8; 32]).unwrap();
        assert_eq!(op.reputation, 1500);
    }

    #[test]
    fn test_escrow_deposit() {
        let mut state = State::new();

        let tx = Transaction::new_escrow_deposit([1u8; 32], 1000);
        state.apply_transaction(&tx).unwrap();

        let escrow = state.settlement.get_escrow(&[1u8; 32]).unwrap();
        assert_eq!(escrow.amount, 1000);
    }

    #[test]
    fn test_escrow_release() {
        let mut state = State::new();

        // Create escrow
        let deposit_tx = Transaction::new_escrow_deposit([1u8; 32], 1000);
        state.apply_transaction(&deposit_tx).unwrap();

        // Release to operator
        let release_tx = Transaction::new_escrow_release([1u8; 32], [2u8; 32]);
        state.apply_transaction(&release_tx).unwrap();

        let escrow = state.settlement.get_escrow(&[1u8; 32]).unwrap();
        assert_eq!(escrow.status, EscrowStatus::Released);
    }

    #[test]
    fn test_escrow_refund() {
        let mut state = State::new();

        // Create escrow
        let deposit_tx = Transaction::new_escrow_deposit([1u8; 32], 1000);
        state.apply_transaction(&deposit_tx).unwrap();

        // Refund
        let refund_tx = Transaction::new_escrow_refund([1u8; 32]);
        state.apply_transaction(&refund_tx).unwrap();

        let escrow = state.settlement.get_escrow(&[1u8; 32]).unwrap();
        assert_eq!(escrow.status, EscrowStatus::Refunded);
    }

    #[test]
    fn test_height_increment() {
        let mut state = State::new();
        assert_eq!(state.height, 0);

        state.increment_height();
        assert_eq!(state.height, 1);

        state.increment_height();
        assert_eq!(state.height, 2);
    }

    #[test]
    fn test_state_root_hash() {
        let mut state = State::new();

        let hash1 = state.root_hash();
        let hash2 = state.root_hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_operator_slashing() {
        let mut state = State::new();

        // Register with stake
        let tx =
            Transaction::new_operator_register([1u8; 32], [2u8; 32], 10000, "us-east".to_string());
        state.apply_transaction(&tx).unwrap();

        // Slash operator
        let slashed = state.settlement.operator_registry.slash(&[1u8; 32], 5000);
        assert!(slashed.is_ok());
    }
}
