use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::transaction::{Transaction, TransactionType};

pub const MIN_FEE: u64 = 1;
pub const STATE_VERSION: u64 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub version: u64,
    pub height: u64,
    pub accounts: HashMap<[u8; 32], Account>,
    pub validator_set_root: [u8; 32],
    pub partitions: HashMap<u32, PartitionState>,
    pub intelligence_receipts: HashMap<[u8; 32], Receipt>,
    pub fee_pool: u64,
    pub slashing_pool: u64,
    pub total_stake: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub balance: u64,
    pub nonce: u64,
    pub code: Option<Vec<u8>>,
    pub storage: HashMap<Vec<u8>, Vec<u8>>,
}

impl Account {
    pub fn new(balance: u64) -> Self {
        Self {
            balance,
            nonce: 0,
            code: None,
            storage: HashMap::new(),
        }
    }

    pub fn with_code(mut self, code: Vec<u8>) -> Self {
        self.code = Some(code);
        self
    }

    pub fn can_pay(&self, amount: u64, fee: u64) -> bool {
        self.balance >= amount + fee
    }

    pub fn transfer(&mut self, amount: u64) -> Result<(), StateError> {
        if self.balance < amount {
            return Err(StateError::InsufficientBalance);
        }
        self.balance -= amount;
        Ok(())
    }

    pub fn deposit(&mut self, amount: u64) {
        self.balance += amount;
    }

    pub fn withdraw(&mut self, amount: u64) -> Result<(), StateError> {
        if self.balance < amount {
            return Err(StateError::InsufficientBalance);
        }
        self.balance -= amount;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionState {
    pub id: u32,
    pub root_hash: [u8; 32],
    pub size: u64,
    pub artifact_count: u64,
}

impl PartitionState {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            root_hash: [0u8; 32],
            size: 0,
            artifact_count: 0,
        }
    }
}

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
    AccountNotFound,
    InsufficientBalance,
    InvalidNonce,
    InvalidTransaction,
    InvalidReceiver,
    InvalidSender,
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
        }
    }
}

impl std::error::Error for StateError {}

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
            accounts: HashMap::new(),
            validator_set_root: [0u8; 32],
            partitions: HashMap::new(),
            intelligence_receipts: HashMap::new(),
            fee_pool: 0,
            slashing_pool: 0,
            total_stake: 0,
        }
    }

    pub fn with_account(mut self, id: [u8; 32], balance: u64) -> Self {
        self.accounts.insert(id, Account::new(balance));
        self
    }

    pub fn create_account(&mut self, id: [u8; 32], balance: u64) {
        self.accounts.insert(id, Account::new(balance));
    }

    pub fn get_account(&self, id: &[u8; 32]) -> Option<&Account> {
        self.accounts.get(id)
    }

    pub fn get_account_mut(&mut self, id: &[u8; 32]) -> Option<&mut Account> {
        self.accounts.get_mut(id)
    }

    pub fn account_exists(&self, id: &[u8; 32]) -> bool {
        self.accounts.contains_key(id)
    }

    pub fn apply_transaction(&mut self, tx: &Transaction) -> Result<Option<Receipt>, StateError> {
        tx.verify().map_err(|_| StateError::InvalidTransaction)?;

        if tx.fee < MIN_FEE {
            return Err(StateError::InvalidTransaction);
        }

        let sender = self
            .accounts
            .get_mut(&tx.sender)
            .ok_or(StateError::AccountNotFound)?;

        if sender.nonce + 1 != tx.nonce {
            return Err(StateError::InvalidNonce);
        }

        if !sender.can_pay(0, tx.fee) {
            return Err(StateError::InsufficientBalance);
        }

        sender.nonce += 1;
        sender.balance -= tx.fee;
        self.fee_pool += tx.fee;

        match tx.tx_type {
            TransactionType::Transfer => {
                self.apply_transfer(tx)?;
            }
            TransactionType::ContractDeploy => {
                self.apply_contract_deploy(tx)?;
            }
            TransactionType::ContractCall => {
                self.apply_contract_call(tx)?;
            }
            TransactionType::IntelligenceSubmit => {
                let receipt = self.apply_intelligence_submit(tx)?;
                return Ok(Some(receipt));
            }
            TransactionType::IntelligenceSettle => {
                self.apply_intelligence_settle(tx)?;
            }
            TransactionType::GovernanceVote => {
                self.apply_governance_vote(tx)?;
            }
        }

        Ok(None)
    }

    fn apply_transfer(&mut self, tx: &Transaction) -> Result<(), StateError> {
        let receiver_id: [u8; 32] = tx
            .payload
            .get(0..32)
            .map(|p| {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(p);
                arr
            })
            .ok_or(StateError::InvalidReceiver)?;

        let amount: u64 = tx
            .payload
            .get(32..40)
            .map(|p| u64::from_le_bytes(p.try_into().unwrap()))
            .unwrap_or(0);

        let sender = self
            .accounts
            .get_mut(&tx.sender)
            .ok_or(StateError::AccountNotFound)?;
        sender.transfer(amount)?;

        self.accounts
            .entry(receiver_id)
            .or_insert_with(|| Account::new(0))
            .deposit(amount);

        Ok(())
    }

    fn apply_contract_deploy(&mut self, tx: &Transaction) -> Result<(), StateError> {
        let sender = self
            .accounts
            .get_mut(&tx.sender)
            .ok_or(StateError::AccountNotFound)?;

        sender.code = Some(tx.payload.clone());

        Ok(())
    }

    fn apply_contract_call(&mut self, tx: &Transaction) -> Result<(), StateError> {
        let contract_id: [u8; 32] = tx
            .payload
            .get(0..32)
            .map(|p| {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(p);
                arr
            })
            .ok_or(StateError::InvalidReceiver)?;

        let contract = self
            .accounts
            .get(&contract_id)
            .ok_or(StateError::AccountNotFound)?;

        if contract.code.is_none() {
            return Err(StateError::InvalidReceiver);
        }

        Ok(())
    }

    fn apply_intelligence_submit(&mut self, tx: &Transaction) -> Result<Receipt, StateError> {
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

        let fee = tx.fee;

        let receipt = Receipt::new(job_id, operator_id, output_hash, compute_used, fee);
        let receipt_id = receipt.id;

        self.intelligence_receipts.insert(receipt_id, receipt);

        Ok(self.intelligence_receipts.get(&receipt_id).unwrap().clone())
    }

    fn apply_intelligence_settle(&mut self, tx: &Transaction) -> Result<(), StateError> {
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
            let operator = self
                .accounts
                .get_mut(&receipt.operator_id)
                .ok_or(StateError::AccountNotFound)?;
            operator.deposit(receipt.fee);
            receipt.settle();
        }

        Ok(())
    }

    fn apply_governance_vote(&mut self, _tx: &Transaction) -> Result<(), StateError> {
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
        hasher.update(&self.fee_pool.to_le_bytes());
        hasher.update(&self.slashing_pool.to_le_bytes());
        hasher.update(&self.total_stake.to_le_bytes());

        let mut account_keys: Vec<_> = self.accounts.keys().collect();
        account_keys.sort();
        for key in account_keys {
            if let Some(account) = self.accounts.get(key) {
                hasher.update(key);
                hasher.update(&account.balance.to_le_bytes());
                hasher.update(&account.nonce.to_le_bytes());
            }
        }

        *hasher.finalize().as_bytes()
    }

    pub fn generate_merkle_proof(&self, account_id: &[u8; 32]) -> Option<MerkleProof> {
        if !self.account_exists(account_id) {
            return None;
        }

        let account = self.get_account(account_id)?;
        let root_hash = self.root_hash();

        let proof = MerkleProof {
            leaf_hash: Self::hash_account(account_id, account),
            path: Vec::new(),
            root_hash,
        };

        Some(proof)
    }

    fn hash_account(id: &[u8; 32], account: &Account) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(id);
        hasher.update(&account.balance.to_le_bytes());
        hasher.update(&account.nonce.to_le_bytes());
        *hasher.finalize().as_bytes()
    }

    fn combine_hashes(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(&left);
        hasher.update(&right);
        *hasher.finalize().as_bytes()
    }

    pub fn verify_merkle_proof(&self, account_id: &[u8; 32], proof: &MerkleProof) -> bool {
        if proof.path.is_empty() {
            return proof.leaf_hash == proof.root_hash;
        }

        let account = match self.get_account(account_id) {
            Some(a) => a,
            None => return false,
        };

        let mut current_hash = Self::hash_account(account_id, account);

        for (sibling_hash, is_left) in &proof.path {
            current_hash = if *is_left {
                Self::combine_hashes(*sibling_hash, current_hash)
            } else {
                Self::combine_hashes(current_hash, *sibling_hash)
            };
        }

        current_hash == proof.root_hash
    }

    pub fn add_partition(&mut self, partition_id: u32) {
        self.partitions
            .insert(partition_id, PartitionState::new(partition_id));
    }

    pub fn remove_partition(&mut self, partition_id: u32) -> Option<PartitionState> {
        self.partitions.remove(&partition_id)
    }

    pub fn update_partition_root(
        &mut self,
        partition_id: u32,
        root_hash: [u8; 32],
    ) -> Result<(), StateError> {
        let partition = self
            .partitions
            .get_mut(&partition_id)
            .ok_or(StateError::InvalidTransaction)?;
        partition.root_hash = root_hash;
        partition.size += 1;
        Ok(())
    }

    pub fn increment_height(&mut self) {
        self.height += 1;
    }

    pub fn set_validator_set_root(&mut self, root: [u8; 32]) {
        self.validator_set_root = root;
    }

    pub fn set_total_stake(&mut self, stake: u64) {
        self.total_stake = stake;
    }

    pub fn distribute_fees(
        &mut self,
        validators: &[[u8; 32]],
        amount: u64,
    ) -> Result<HashMap<[u8; 32], u64>, StateError> {
        if self.fee_pool < amount {
            return Err(StateError::InsufficientBalance);
        }

        self.fee_pool -= amount;

        if validators.is_empty() {
            return Ok(HashMap::new());
        }

        let per_validator = amount / validators.len() as u64;
        let mut distribution = HashMap::new();

        for validator_id in validators {
            if let Some(account) = self.accounts.get_mut(validator_id) {
                account.deposit(per_validator);
            }
            distribution.insert(*validator_id, per_validator);
        }

        Ok(distribution)
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    pub leaf_hash: [u8; 32],
    pub path: Vec<([u8; 32], bool)>,
    pub root_hash: [u8; 32],
}

impl MerkleProof {
    pub fn new(leaf_hash: [u8; 32], root_hash: [u8; 32]) -> Self {
        Self {
            leaf_hash,
            path: Vec::new(),
            root_hash,
        }
    }

    pub fn verify(&self, expected_root: [u8; 32]) -> bool {
        if self.path.is_empty() {
            return self.leaf_hash == expected_root;
        }

        let mut current_hash = self.leaf_hash;

        for (sibling_hash, is_left) in &self.path {
            current_hash = if *is_left {
                State::combine_hashes(*sibling_hash, current_hash)
            } else {
                State::combine_hashes(current_hash, *sibling_hash)
            };
        }

        current_hash == expected_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_tx(
        sender: [u8; 32],
        tx_type: TransactionType,
        payload: Vec<u8>,
        nonce: u64,
        fee: u64,
    ) -> Transaction {
        Transaction {
            id: [0u8; 32],
            sender,
            tx_type,
            payload,
            nonce,
            fee,
            signature: vec![0u8; 64],
        }
    }

    #[test]
    fn test_state_creation() {
        let state = State::new();
        assert_eq!(state.height, 0);
        assert_eq!(state.version, STATE_VERSION);
    }

    #[test]
    fn test_create_account() {
        let mut state = State::new();
        let id = [1u8; 32];
        state.create_account(id, 1000);

        let account = state.get_account(&id).unwrap();
        assert_eq!(account.balance, 1000);
        assert_eq!(account.nonce, 0);
    }

    #[test]
    fn test_apply_transfer() {
        let mut state = State::new();
        let sender = [1u8; 32];
        let receiver = [2u8; 32];

        state.create_account(sender, 1000);
        state.create_account(receiver, 0);

        let mut payload = Vec::new();
        payload.extend_from_slice(&receiver);
        payload.extend_from_slice(&500u64.to_le_bytes());

        let tx = create_tx(sender, TransactionType::Transfer, payload, 1, 10);
        state.apply_transaction(&tx).unwrap();

        let sender_acc = state.get_account(&sender).unwrap();
        let receiver_acc = state.get_account(&receiver).unwrap();

        assert_eq!(sender_acc.balance, 490);
        assert_eq!(receiver_acc.balance, 500);
    }

    #[test]
    fn test_apply_transfer_insufficient_balance() {
        let mut state = State::new();
        let sender = [1u8; 32];
        let receiver = [2u8; 32];

        state.create_account(sender, 100);
        state.create_account(receiver, 0);

        let mut payload = Vec::new();
        payload.extend_from_slice(&receiver);
        payload.extend_from_slice(&500u64.to_le_bytes());

        let tx = create_tx(sender, TransactionType::Transfer, payload, 1, 10);
        let result = state.apply_transaction(&tx);

        assert!(result.is_err());
    }

    #[test]
    fn test_apply_transfer_wrong_nonce() {
        let mut state = State::new();
        let sender = [1u8; 32];
        let receiver = [2u8; 32];

        state.create_account(sender, 1000);

        let mut payload = Vec::new();
        payload.extend_from_slice(&receiver);
        payload.extend_from_slice(&500u64.to_le_bytes());

        let tx = create_tx(sender, TransactionType::Transfer, payload, 5, 10);
        let result = state.apply_transaction(&tx);

        assert!(result.is_err());
    }

    #[test]
    fn test_apply_contract_deploy() {
        let mut state = State::new();
        let sender = [1u8; 32];

        state.create_account(sender, 1000);

        let code = vec![0x60, 0x00, 0x60, 0x00, 0x52];
        let tx = create_tx(sender, TransactionType::ContractDeploy, code, 1, 10);
        state.apply_transaction(&tx).unwrap();

        let account = state.get_account(&sender).unwrap();
        assert!(account.code.is_some());
    }

    #[test]
    fn test_state_root_hash() {
        let mut state = State::new();
        state.create_account([1u8; 32], 1000);
        state.create_account([2u8; 32], 2000);

        let hash1 = state.root_hash();
        let hash2 = state.root_hash();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_merkle_proof_generation() {
        let mut state = State::new();
        state.create_account([1u8; 32], 1000);

        let root = state.root_hash();
        let proof = state.generate_merkle_proof(&[1u8; 32]).unwrap();

        assert_eq!(proof.root_hash, root);
    }

    #[test]
    fn test_merkle_proof_invalid_account() {
        let mut state = State::new();
        state.create_account([1u8; 32], 1000);

        let proof = state.generate_merkle_proof(&[9u8; 32]);
        assert!(proof.is_none());
    }

    #[test]
    fn test_fee_distribution() {
        let mut state = State::new();
        state.create_account([1u8; 32], 1000);
        state.create_account([2u8; 32], 1000);
        state.fee_pool = 100;

        let validators = [[1u8; 32], [2u8; 32]];
        let distribution = state.distribute_fees(&validators, 100).unwrap();

        assert_eq!(distribution.len(), 2);
        assert_eq!(distribution.get(&[1u8; 32]), Some(&50));
    }

    #[test]
    fn test_intelligence_submit_creates_receipt() {
        let mut state = State::new();
        let sender = [1u8; 32];

        state.create_account(sender, 1000);

        let mut payload = Vec::new();
        payload.extend_from_slice(&[2u8; 32]);
        payload.extend_from_slice(&1000u64.to_le_bytes());
        payload.extend_from_slice(&[3u8; 32].repeat(8));

        let tx = create_tx(sender, TransactionType::IntelligenceSubmit, payload, 1, 10);
        let result = state.apply_transaction(&tx);

        assert!(result.is_ok());
        let receipt = result.unwrap();
        assert!(receipt.is_some());
    }

    #[test]
    fn test_partition_management() {
        let mut state = State::new();

        state.add_partition(1);
        assert!(state.partitions.contains_key(&1));

        state.remove_partition(1);
        assert!(!state.partitions.contains_key(&1));
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
    fn test_apply_multiple_transactions() {
        let mut state = State::new();
        let sender = [1u8; 32];
        let receiver = [2u8; 32];

        state.create_account(sender, 10000);
        state.create_account(receiver, 0);

        let txs = vec![
            create_tx(
                sender,
                TransactionType::Transfer,
                vec![2u8; 32]
                    .into_iter()
                    .chain(100u64.to_le_bytes().into_iter())
                    .collect(),
                1,
                10,
            ),
            create_tx(
                sender,
                TransactionType::Transfer,
                vec![2u8; 32]
                    .into_iter()
                    .chain(200u64.to_le_bytes().into_iter())
                    .collect(),
                2,
                10,
            ),
        ];

        let results = state.apply_block_transactions(&txs).unwrap();
        assert_eq!(results.len(), 2);

        let receiver_acc = state.get_account(&receiver).unwrap();
        assert_eq!(receiver_acc.balance, 300);
    }

    #[test]
    fn test_apply_transaction_updates_fee_pool() {
        let mut state = State::new();
        let sender = [1u8; 32];

        state.create_account(sender, 1000);
        assert_eq!(state.fee_pool, 0);

        let tx = create_tx(
            sender,
            TransactionType::Transfer,
            vec![2u8; 32]
                .into_iter()
                .chain(100u64.to_le_bytes().into_iter())
                .collect(),
            1,
            10,
        );
        state.apply_transaction(&tx).unwrap();

        assert_eq!(state.fee_pool, 10);
    }
}
