use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

use crate::state::State;
use crate::block::{Block, Transaction};
use crate::consensus::ConsensusEngine;
use crate::validator_set::ValidatorSet;

/// Combined node state for RPC and consensus
pub struct NodeState {
    pub state: Arc<RwLock<State>>,
    pub blocks: Arc<RwLock<HashMap<u64, Block>>>,
    pub transactions: Arc<RwLock<HashMap<[u8; 32], Transaction>>>,
    pub mempool: Arc<RwLock<Vec<Transaction>>>,
    pub validator_set: Arc<RwLock<ValidatorSet>>,
}

impl NodeState {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(State::new())),
            blocks: Arc::new(RwLock::new(HashMap::new())),
            transactions: Arc::new(RwLock::new(HashMap::new())),
            mempool: Arc::new(RwLock::new(Vec::new())),
            validator_set: Arc::new(RwLock::new(ValidatorSet::new())),
        }
    }

    /// Store a block and update indices
    pub async fn store_block(&self, block: &Block) -> [u8; 32] {
        let block_hash = block.hash();
        let height = block.header.height;
        
        // Store block
        {
            let mut blocks = self.blocks.write().await;
            blocks.insert(height, block.clone());
        }
        
        // Index transactions - just clone the transactions
        {
            let mut txs = self.transactions.write().await;
            for tx in &block.transactions {
                let tx_hash = tx.id;
                txs.insert(tx_hash, tx.clone());
            }
        }
        
        block_hash
    }

    /// Add transaction to mempool and index
    pub async fn add_transaction(&self, tx: &super::block::Transaction) -> [u8; 32] {
        let tx_hash = tx.id;
        
        // Add to mempool
        {
            let mut mempool = self.mempool.write().await;
            mempool.push(tx.clone());
        }
        
        // Index by hash
        {
            let mut txs = self.transactions.write().await;
            txs.insert(tx_hash, tx.clone());
        }
        
        tx_hash
    }

    /// Get block by height
    pub async fn get_block(&self, height: u64) -> Option<Block> {
        let blocks = self.blocks.read().await;
        blocks.get(&height).cloned()
    }

    /// Get transaction by hash
    pub async fn get_transaction(&self, tx_hash: [u8; 32]) -> Option<Transaction> {
        let txs = self.transactions.read().await;
        txs.get(&tx_hash).cloned()
    }

    /// Get current block height
    pub async fn get_block_height(&self) -> u64 {
        let state = self.state.read().await;
        state.height
    }

    /// Get validator set
    pub async fn get_validators(&self) -> Vec<super::validator_set::Validator> {
        let vset = self.validator_set.read().await;
        vset.validators.values().cloned().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RpcMethod {
    SubmitTransaction,
    GetBlock,
    GetState,
    GetTransaction,
    GetValidatorSet,
    Subscribe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method")]
pub enum RpcRequest {
    #[serde(rename = "submit_transaction")]
    SubmitTransaction { tx: Transaction },
    #[serde(rename = "get_block")]
    GetBlock { height: u64 },
    #[serde(rename = "get_state")]
    GetState {},
    #[serde(rename = "get_transaction")]
    GetTransaction { tx_hash: [u8; 32] },
    #[serde(rename = "get_validator_set")]
    GetValidatorSet {},
    #[serde(rename = "subscribe")]
    Subscribe { event_type: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "result")]
pub enum RpcResponse {
    #[serde(rename = "submit_transaction")]
    SubmitTransaction { tx_hash: [u8; 32], accepted: bool },
    #[serde(rename = "get_block")]
    GetBlock { block: Option<Block> },
    #[serde(rename = "get_state")]
    GetState { state: State },
    #[serde(rename = "get_transaction")]
    GetTransaction { tx: Option<Transaction> },
    #[serde(rename = "get_validator_set")]
    GetValidatorSet { validators: Vec<crate::validator_set::Validator> },
    #[serde(rename = "subscribe")]
    Subscribe { subscription_id: u64 },
    #[serde(rename = "error")]
    RpcError { message: String },
}

pub struct RpcServer {
    state: Arc<RwLock<State>>,
    consensus: Arc<ConsensusEngine>,
    mempool: Arc<RwLock<Vec<Transaction>>>,
    blocks: Arc<RwLock<HashMap<u64, Block>>>,
    transactions: Arc<RwLock<HashMap<[u8; 32], Transaction>>>,
    validator_set: Arc<RwLock<ValidatorSet>>,
}

impl RpcServer {
    pub fn new(
        state: Arc<RwLock<State>>,
        consensus: Arc<ConsensusEngine>,
        mempool: Arc<RwLock<Vec<Transaction>>>,
        blocks: Arc<RwLock<HashMap<u64, Block>>>,
        transactions: Arc<RwLock<HashMap<[u8; 32], Transaction>>>,
        validator_set: Arc<RwLock<ValidatorSet>>,
    ) -> Self {
        Self {
            state,
            consensus,
            mempool,
            blocks,
            transactions,
            validator_set,
        }
    }

    pub async fn handle_request(&self, request: RpcRequest) -> RpcResponse {
        match request {
            RpcRequest::SubmitTransaction { tx } => {
                use blake3::Hasher;
                let mut hasher = Hasher::new();
                hasher.update(&bincode::serialize(&tx).unwrap());
                let tx_hash = *hasher.finalize().as_bytes();
                
                // Store transaction in transaction index
                {
                    let mut txs = self.transactions.write().await;
                    txs.insert(tx_hash, tx.clone());
                }
                
                // Add to mempool
                let mut mempool = self.mempool.write().await;
                mempool.push(tx);
                
                RpcResponse::SubmitTransaction {
                    tx_hash,
                    accepted: true,
                }
            }
            RpcRequest::GetBlock { height } => {
                let blocks = self.blocks.read().await;
                let block = blocks.get(&height).cloned();
                RpcResponse::GetBlock { block }
            }
            RpcRequest::GetState {} => {
                let state = self.state.read().await;
                RpcResponse::GetState {
                    state: state.clone(),
                }
            }
            RpcRequest::GetTransaction { tx_hash } => {
                let txs = self.transactions.read().await;
                let tx = txs.get(&tx_hash).cloned();
                RpcResponse::GetTransaction { tx }
            }
            RpcRequest::GetValidatorSet {} => {
                let vset = self.validator_set.read().await;
                let validators: Vec<super::validator_set::Validator> = vset.validators.values().cloned().collect();
                RpcResponse::GetValidatorSet {
                    validators,
                }
            }
            RpcRequest::Subscribe { event_type: _ } => {
                RpcResponse::Subscribe {
                    subscription_id: 0,
                }
            }
        }
    }

    pub async fn serve(self, addr: &str) -> Result<(), RpcError> {
        let listener = TcpListener::bind(addr).await
            .map_err(|e| RpcError(e.to_string()))?;

        println!("RPC server listening on {}", addr);

        loop {
            let (socket, _) = listener.accept().await
                .map_err(|e| RpcError(e.to_string()))?;

            let state = self.state.clone();
            let consensus = self.consensus.clone();
            let mempool = self.mempool.clone();
            let blocks = self.blocks.clone();
            let transactions = self.transactions.clone();
            let validator_set = self.validator_set.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, state, consensus, mempool, blocks, transactions, validator_set).await {
                    eprintln!("RPC connection error: {}", e);
                }
            });
        }
    }
}

async fn handle_connection(
    mut socket: TcpStream,
    state: Arc<RwLock<State>>,
    consensus: Arc<ConsensusEngine>,
    mempool: Arc<RwLock<Vec<Transaction>>>,
    blocks: Arc<RwLock<HashMap<u64, Block>>>,
    transactions: Arc<RwLock<HashMap<[u8; 32], Transaction>>>,
    validator_set: Arc<RwLock<ValidatorSet>>,) -> Result<(), RpcError> {
    let mut buf = [0u8; 4096];

    loop {
        let n = socket.read(&mut buf).await
            .map_err(|e| RpcError(e.to_string()))?;

        if n == 0 {
            break;
        }

        let request: RpcRequest = serde_json::from_slice(&buf[..n])
            .map_err(|e| RpcError(e.to_string()))?;

        // Clone the shared references for the handler
        let state = state.clone();
        let consensus = consensus.clone();
        let mempool = mempool.clone();
        let blocks = blocks.clone();
        let transactions = transactions.clone();
        let validator_set = validator_set.clone();
        
        let server = RpcServer::new(
            state,
            consensus,
            mempool,
            blocks,
            transactions,
            validator_set,
        );
        
        let response = server.handle_request(request).await;
        let response_bytes = serde_json::to_vec(&response)
            .map_err(|e| RpcError(e.to_string()))?;

        socket.write_all(&response_bytes).await
            .map_err(|e| RpcError(e.to_string()))?;
    }

    Ok(())
}

#[derive(Debug)]
pub struct RpcError(String);

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for RpcError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_request_serialize() {
        let request = RpcRequest::GetState {};
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("get_state"));
    }

    #[test]
    fn test_rpc_response_serialize() {
        let response = RpcResponse::RpcError {
            message: "test error".to_string(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("error"));
    }
}