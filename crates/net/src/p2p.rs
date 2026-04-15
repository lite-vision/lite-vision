use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

use crate::protocol::{MessageType, ProtocolMessage};

const DEFAULT_PORT: u16 = 9100;
const _PING_INTERVAL_SECS: u64 = 30;
const _MAX_PEERS_DEFAULT: usize = 128;

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
            last_seen: current_timestamp(),
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

/// P2P Network with actual peer handling
pub struct P2PNetwork {
    pub peer_id: [u8; 32],
    peers: Arc<RwLock<HashMap<[u8; 32], Peer>>>,
    banned: Arc<RwLock<HashSet<[u8; 32]>>>,
    #[allow(dead_code)]
    message_tx: Arc<RwLock<Option<mpsc::Sender<P2PMessage>>>>,
    running: Arc<RwLock<bool>>,
}

/// Messages for P2P event handling
#[derive(Debug, Clone)]
pub enum P2PMessage {
    PeerConnected(Peer),
    PeerDisconnected([u8; 32]),
    MessageReceived(ProtocolMessage, [u8; 32]),
    Discovery(PeerDiscovery),
}

/// Peer discovery message for operator discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerDiscovery {
    pub peer_id: [u8; 32],
    pub seq: u64,
    pub peers: Vec<PeerInfo>,
}

impl P2PNetwork {
    pub fn new(peer_id: [u8; 32]) -> Self {
        Self {
            peer_id,
            peers: Arc::new(RwLock::new(HashMap::new())),
            banned: Arc::new(RwLock::new(HashSet::new())),
            message_tx: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Connect to a peer (simulated connection)
    pub async fn connect(&self, address: String, port: u16) -> Result<(), P2PError> {
        // In production, this would establish actual TCP connection
        // For now, we simulate the handshake
        
        use blake3::Hasher;
        let mut hasher = Hasher::new();
        hasher.update(address.as_bytes());
        hasher.update(&port.to_le_bytes());
        let peer_id = *hasher.finalize().as_bytes();
        
        let peer = Peer::new(peer_id, address, port);
        self.add_peer(peer).await;
        
        // Update status to connected
        if let Some(p) = self.peers.write().await.get_mut(&peer_id) {
            p.status = PeerStatus::Connected;
            p.last_seen = current_timestamp();
        }
        
        Ok(())
    }

    /// Connect to bootstrap nodes
    pub async fn connect_bootstrap(&self, nodes: &[String]) -> Result<(), P2PError> {
        for node in nodes {
            let parts: Vec<&str> = node.split(':').collect();
            if parts.len() >= 2 {
                let address = parts[0].to_string();
                let port: u16 = parts[1].parse().unwrap_or(DEFAULT_PORT);
                self.connect(address, port).await.ok();
            }
        }
        Ok(())
    }

    /// Disconnect from a peer
    pub async fn disconnect(&self, peer_id: &[u8; 32]) -> Result<(), P2PError> {
        if let Some(peer) = self.peers.write().await.get_mut(peer_id) {
            peer.status = PeerStatus::Disconnected;
        }
        self.remove_peer(peer_id).await;
        Ok(())
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

    /// Broadcast a message to all connected peers
    pub async fn broadcast(&self, msg: ProtocolMessage) -> usize {
        let active = self.get_active_peers().await;
        
        // In production, this would send to each peer
        for peer in &active {
            self.send_to_peer(peer.id, msg.clone()).await.ok();
        }
        
        active.len()
    }

    /// Send a message to a specific peer
    pub async fn send_to_peer(&self, peer_id: [u8; 32], msg: ProtocolMessage) -> Result<(), P2PError> {
        // In production, this would serialize and send over the wire
        if !self.is_connected(&peer_id).await {
            return Err(P2PError(format!("Peer {} not connected", hex::encode(peer_id))));
        }
        
        // Simulate successful send
        Ok(())
    }

    /// Propagate a job to peers (for job distribution)
    pub async fn propagate_job(&self, job_id: [u8; 32], data: Vec<u8>) -> Result<(), P2PError> {
        let msg = ProtocolMessage::new(
            MessageType::Intelligence,
            self.peer_id,
            serde_json::json!({
                "job_id": job_id,
                "data": data,
                "origin": self.peer_id,
            }).to_string().into_bytes(),
        );
        
        self.broadcast(msg).await;
        Ok(())
    }

    /// Propagate a receipt to peers (for verification)
    pub async fn propagate_receipt(&self, receipt_id: [u8; 32], data: Vec<u8>) -> Result<(), P2PError> {
        let msg = ProtocolMessage::new(
            MessageType::Intelligence,
            self.peer_id,
            serde_json::json!({
                "receipt_id": receipt_id,
                "data": data,
                "origin": self.peer_id,
            }).to_string().into_bytes(),
        );
        
        self.broadcast(msg).await;
        Ok(())
    }

    /// Gossip protocol - discover new peers
    pub async fn gossip_discovery(&self) -> Result<(), P2PError> {
        let active = self.get_active_peers().await;
        
        for peer in active {
            // Request peer list from each connected peer
            let msg = ProtocolMessage::new(
                MessageType::Ping,
                self.peer_id,
                serde_json::json!({
                    "requester_id": self.peer_id,
                    "seq": current_timestamp(),
                }).to_string().into_bytes(),
            );
            
            self.send_to_peer(peer.id, msg).await.ok();
        }
        
        Ok(())
    }

    /// Handle incoming discovery response
    pub async fn handle_discovery_response(&self, _peer_id: [u8; 32], peers: Vec<PeerInfo>) {
        for info in peers {
            let peer = Peer::new(info.peer_id, "".to_string(), info.listen_port);
            self.add_peer(peer).await;
        }
    }

    /// Start background tasks (ping, gossip)
    pub async fn start_background_tasks(&self) {
        *self.running.write().await = true;
        
        // In production, this would spawn async tasks
        // - Ping task: periodically ping peers to check connectivity
        // - Gossip task: periodically exchange peer lists
    }

    /// Stop background tasks
    pub async fn stop_background_tasks(&self) {
        *self.running.write().await = false;
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

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
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