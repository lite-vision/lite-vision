use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id: [u8; 32],
    pub address: String,
    pub port: u16,
    pub status: PeerStatus,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PeerStatus {
    Connected,
    Disconnected,
    Banned,
}

pub struct P2PNetwork {
    pub peer_id: [u8; 32],
    pub peers: Vec<Peer>,
    pub banned: HashSet<[u8; 32]>,
}

impl P2PNetwork {
    pub fn new(peer_id: [u8; 32]) -> Self {
        Self {
            peer_id,
            peers: Vec::new(),
            banned: HashSet::new(),
        }
    }

    pub fn add_peer(&mut self, peer: Peer) {
        if !self.banned.contains(&peer.id) {
            self.peers.push(peer);
        }
    }

    pub fn remove_peer(&mut self, peer_id: &[u8; 32]) {
        self.peers.retain(|p| p.id != *peer_id);
    }

    pub fn ban_peer(&mut self, peer_id: [u8; 32]) {
        self.banned.insert(peer_id);
        self.remove_peer(&peer_id);
    }

    pub fn get_active_peers(&self) -> Vec<&Peer> {
        self.peers
            .iter()
            .filter(|p| matches!(p.status, PeerStatus::Connected))
            .collect()
    }

    pub fn broadcast(&self, _message: &[u8]) -> usize {
        self.get_active_peers().len()
    }
}
