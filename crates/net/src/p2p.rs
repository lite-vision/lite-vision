use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;

use crate::protocol::ProtocolMessage;

const DEFAULT_PORT: u16 = 9100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id: [u8; 32],
    pub address: String,
    pub port: u16,
    pub status: PeerStatus,
    pub last_seen: u64,
    pub latency_ms: u64,
    pub version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerStatus {
    Connecting,
    Handshaking,
    Connected,
    Disconnected,
    Banned,
}

impl Peer {
    pub fn new(id: [u8; 32], address: String, port: u16) -> Self {
        Self {
            id,
            address,
            port,
            status: PeerStatus::Connecting,
            last_seen: 0,
            latency_ms: 0,
            version: 1,
        }
    }

    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: [u8; 32],
    pub listen_port: u16,
    pub version: u32,
    pub capabilities: Vec<String>,
}

pub struct P2PNetwork {
    pub peer_id: [u8; 32],
    peers: RwLock<HashMap<[u8; 32], Peer>>,
    banned: RwLock<HashSet<[u8; 32]>>,
}

impl P2PNetwork {
    pub fn new(peer_id: [u8; 32]) -> Self {
        Self {
            peer_id,
            peers: RwLock::new(HashMap::new()),
            banned: RwLock::new(HashSet::new()),
        }
    }

    pub async fn add_peer(&self, peer: Peer) {
        let banned = self.banned.read().await;
        if !banned.contains(&peer.id) {
            drop(banned);
            self.peers.write().await.insert(peer.id, peer);
        }
    }

    pub async fn remove_peer(&self, peer_id: &[u8; 32]) {
        self.peers.write().await.remove(peer_id);
    }

    pub async fn ban_peer(&self, peer_id: [u8; 32]) {
        self.banned.write().await.insert(peer_id);
        self.remove_peer(&peer_id).await;
    }

    pub async fn get_active_peers(&self) -> Vec<Peer> {
        let peers = self.peers.read().await;
        peers
            .values()
            .filter(|p| p.status == PeerStatus::Connected)
            .cloned()
            .collect()
    }

    pub async fn get_peer_count(&self) -> usize {
        self.peers.read().await.len()
    }

    pub async fn is_connected(&self, peer_id: &[u8; 32]) -> bool {
        self.peers
            .read()
            .await
            .get(peer_id)
            .map(|p| p.status == PeerStatus::Connected)
            .unwrap_or(false)
    }

    pub async fn broadcast(&self, _msg: ProtocolMessage) -> usize {
        let active = self.get_active_peers().await;
        active.len()
    }
}

#[derive(Debug)]
pub struct P2PError(String);

impl std::fmt::Display for P2PError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for P2PError {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct P2PConfig {
    pub listen_address: String,
    pub listen_port: u16,
    pub max_peers: usize,
    pub bootstrap_nodes: Vec<String>,
    pub ping_interval_secs: u64,
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            listen_address: "0.0.0.0".to_string(),
            listen_port: DEFAULT_PORT,
            max_peers: 128,
            bootstrap_nodes: Vec::new(),
            ping_interval_secs: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_endpoint() {
        let peer = Peer::new([1u8; 32], "127.0.0.1".to_string(), 8080);
        assert_eq!(peer.endpoint(), "127.0.0.1:8080");
    }

    #[tokio::test]
    async fn test_p2p_network_creation() {
        let network = P2PNetwork::new([1u8; 32]);
        assert_eq!(network.get_peer_count().await, 0);
    }

    #[tokio::test]
    async fn test_add_remove_peer() {
        let network = P2PNetwork::new([1u8; 32]);
        let peer = Peer::new([2u8; 32], "127.0.0.1".to_string(), 8080);
        
        network.add_peer(peer).await;
        assert_eq!(network.get_peer_count().await, 1);
        
        network.remove_peer(&[2u8; 32]).await;
        assert_eq!(network.get_peer_count().await, 0);
    }

    #[tokio::test]
    async fn test_ban_peer() {
        let network = P2PNetwork::new([1u8; 32]);
        let peer = Peer::new([2u8; 32], "127.0.0.1".to_string(), 8080);
        
        network.add_peer(peer).await;
        assert_eq!(network.get_peer_count().await, 1);
        
        network.ban_peer([2u8; 32]).await;
        assert_eq!(network.get_peer_count().await, 0);
    }

    #[tokio::test]
    async fn test_is_connected() {
        let network = P2PNetwork::new([1u8; 32]);
        let peer = Peer::new([2u8; 32], "127.0.0.1".to_string(), 8080);
        
        assert!(!network.is_connected(&[2u8; 32]).await);
        
        network.add_peer(peer).await;
        assert!(!network.is_connected(&[2u8; 32]).await);
    }
}