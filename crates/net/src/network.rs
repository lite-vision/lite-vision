use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::protocol::{MessageType, ProtocolMessage};
use crate::p2p::{P2PNetwork, Peer, PeerStatus, PeerInfo, P2PConfig};
use crate::message::NetworkMessage;

const _PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub node_id: [u8; 32],
    pub listen_address: String,
    pub listen_port: u16,
    pub truth_plane_enabled: bool,
    pub intelligence_plane_enabled: bool,
    pub rpc_port: u16,
    pub max_peers: usize,
    pub bootnodes: Vec<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            node_id: [0u8; 32],
            listen_address: "0.0.0.0".to_string(),
            listen_port: 9100,
            truth_plane_enabled: true,
            intelligence_plane_enabled: true,
            rpc_port: 8080,
            max_peers: 128,
            bootnodes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NetworkMode {
    Full,
    TruthOnly,
    IntelligenceOnly,
    LightClient,
}

impl Default for NetworkMode {
    fn default() -> Self {
        NetworkMode::Full
    }
}

pub struct NetworkNode {
    pub config: NetworkConfig,
    pub p2p: P2PNetwork,
    pub status: NodeStatus,
    pub mode: NetworkMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Initializing,
    Starting,
    Running,
    Stopping,
    Stopped,
    Error,
}

impl NetworkNode {
    pub fn new(config: NetworkConfig) -> Self {
        Self {
            config: config.clone(),
            p2p: P2PNetwork::new(config.node_id),
            status: NodeStatus::Initializing,
            mode: if config.truth_plane_enabled && config.intelligence_plane_enabled {
                NetworkMode::Full
            } else if config.truth_plane_enabled {
                NetworkMode::TruthOnly
            } else {
                NetworkMode::IntelligenceOnly
            },
        }
    }

    pub async fn start(&mut self) -> Result<(), NetworkError> {
        self.status = NodeStatus::Starting;

        if self.config.truth_plane_enabled {
            println!("Starting Truth Plane...");
        }

        if self.config.intelligence_plane_enabled {
            println!("Starting Intelligence Plane...");
        }

        self.status = NodeStatus::Running;
        println!("Network node started in {:?} mode", self.mode);
        println!("P2P listen: {}:{}", self.config.listen_address, self.config.listen_port);
        println!("RPC: {}:{}", self.config.listen_address, self.config.rpc_port);

        Ok(())
    }

    pub async fn stop(&mut self) {
        self.status = NodeStatus::Stopping;
        self.status = NodeStatus::Stopped;
    }

    pub fn is_running(&self) -> bool {
        self.status == NodeStatus::Running
    }

    pub async fn connect_to_bootnode(&self, peer_id: [u8; 32], address: String, port: u16) -> Result<(), NetworkError> {
        let peer = Peer::new(peer_id, address, port);
        self.p2p.add_peer(peer).await;
        Ok(())
    }

    pub async fn get_connected_peer_count(&self) -> usize {
        self.p2p.get_peer_count().await
    }

    pub async fn broadcast_block(&self, block_data: Vec<u8>) -> Result<(), NetworkError> {
        let msg = ProtocolMessage::new(
            MessageType::Block,
            self.config.node_id,
            block_data,
        );
        self.p2p.broadcast(msg).await;
        Ok(())
    }

    pub async fn broadcast_transaction(&self, tx_data: Vec<u8>) -> Result<(), NetworkError> {
        let msg = ProtocolMessage::new(
            MessageType::Transaction,
            self.config.node_id,
            tx_data,
        );
        self.p2p.broadcast(msg).await;
        Ok(())
    }

    pub async fn broadcast_intelligence(&self, data: Vec<u8>) -> Result<(), NetworkError> {
        let msg = ProtocolMessage::new(
            MessageType::Intelligence,
            self.config.node_id,
            data,
        );
        self.p2p.broadcast(msg).await;
        Ok(())
    }
}

#[derive(Debug)]
pub struct NetworkError(String);

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for NetworkError {}

impl From<std::io::Error> for NetworkError {
    fn from(e: std::io::Error) -> Self {
        NetworkError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert_eq!(config.max_peers, 128);
        assert!(config.truth_plane_enabled);
        assert!(config.intelligence_plane_enabled);
    }

    #[test]
    fn test_network_mode_detection() {
        let config_truth = NetworkConfig {
            truth_plane_enabled: true,
            intelligence_plane_enabled: false,
            ..Default::default()
        };
        let node = NetworkNode::new(config_truth);
        assert_eq!(node.mode, NetworkMode::TruthOnly);

        let config_intel = NetworkConfig {
            truth_plane_enabled: false,
            intelligence_plane_enabled: true,
            ..Default::default()
        };
        let node = NetworkNode::new(config_intel);
        assert_eq!(node.mode, NetworkMode::IntelligenceOnly);

        let config_full = NetworkConfig::default();
        let node = NetworkNode::new(config_full);
        assert_eq!(node.mode, NetworkMode::Full);
    }

    #[tokio::test]
    async fn test_node_lifecycle() {
        let config = NetworkConfig {
            node_id: [1u8; 32],
            ..Default::default()
        };
        let mut node = NetworkNode::new(config);

        assert_eq!(node.status, NodeStatus::Initializing);

        node.start().await.unwrap();
        assert!(node.is_running());

        node.stop().await;
        assert_eq!(node.status, NodeStatus::Stopped);
    }
}