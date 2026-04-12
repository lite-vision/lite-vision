use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::state::State;
use crate::block::Block;
use crate::consensus::ConsensusEngine;
use crate::transaction::Transaction;

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
    GetValidatorSet { validators: Vec<crate::validator::Validator> },
    #[serde(rename = "subscribe")]
    Subscribe { subscription_id: u64 },
    #[serde(rename = "error")]
    RpcError { message: String },
}

pub struct RpcServer {
    state: Arc<RwLock<State>>,
    consensus: Arc<ConsensusEngine>,
    mempool: Arc<RwLock<Vec<Transaction>>>,
}

impl RpcServer {
    pub fn new(
        state: Arc<RwLock<State>>,
        consensus: Arc<ConsensusEngine>,
        mempool: Arc<RwLock<Vec<Transaction>>>,
    ) -> Self {
        Self {
            state,
            consensus,
            mempool,
        }
    }

    pub async fn handle_request(&self, request: RpcRequest) -> RpcResponse {
        match request {
            RpcRequest::SubmitTransaction { tx } => {
                use blake3::Hasher;
                let mut hasher = Hasher::new();
                hasher.update(&bincode::serialize(&tx).unwrap());
                let tx_hash = *hasher.finalize().as_bytes();
                let mut mempool = self.mempool.write().await;
                mempool.push(tx);
                RpcResponse::SubmitTransaction {
                    tx_hash,
                    accepted: true,
                }
            }
            RpcRequest::GetBlock { height: _ } => {
                RpcResponse::GetBlock { block: None }
            }
            RpcRequest::GetState {} => {
                let state = self.state.read().await;
                RpcResponse::GetState {
                    state: state.clone(),
                }
            }
            RpcRequest::GetTransaction { tx_hash: _ } => {
                RpcResponse::GetTransaction { tx: None }
            }
            RpcRequest::GetValidatorSet {} => {
                RpcResponse::GetValidatorSet {
                    validators: Vec::new(),
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

            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, state, consensus, mempool).await {
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
) -> Result<(), RpcError> {
    let mut buf = [0u8; 4096];

    loop {
        let n = socket.read(&mut buf).await
            .map_err(|e| RpcError(e.to_string()))?;

        if n == 0 {
            break;
        }

        let request: RpcRequest = serde_json::from_slice(&buf[..n])
            .map_err(|e| RpcError(e.to_string()))?;

        let response = handle_rpc_request(request, &state, &mempool).await;
        let response_bytes = serde_json::to_vec(&response)
            .map_err(|e| RpcError(e.to_string()))?;

        socket.write_all(&response_bytes).await
            .map_err(|e| RpcError(e.to_string()))?;
    }

    Ok(())
}

async fn handle_rpc_request(
    request: RpcRequest,
    state: &Arc<RwLock<State>>,
    mempool: &Arc<RwLock<Vec<Transaction>>>,
) -> RpcResponse {
    match request {
        RpcRequest::SubmitTransaction { tx } => {
            use blake3::Hasher;
            let mut hasher = Hasher::new();
            hasher.update(&bincode::serialize(&tx).unwrap());
            let tx_hash = *hasher.finalize().as_bytes();
            let mut mempool = mempool.write().await;
            mempool.push(tx);
            RpcResponse::SubmitTransaction {
                tx_hash,
                accepted: true,
            }
        }
        RpcRequest::GetBlock { height: _ } => {
            RpcResponse::GetBlock { block: None }
        }
        RpcRequest::GetState {} => {
            let state = state.read().await;
            RpcResponse::GetState {
                state: state.clone(),
            }
        }
        RpcRequest::GetTransaction { tx_hash: _ } => {
            RpcResponse::GetTransaction { tx: None }
        }
        RpcRequest::GetValidatorSet {} => {
            RpcResponse::GetValidatorSet {
                validators: Vec::new(),
            }
        }
        RpcRequest::Subscribe { event_type: _ } => {
            RpcResponse::Subscribe {
                subscription_id: 0,
            }
        }
    }
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