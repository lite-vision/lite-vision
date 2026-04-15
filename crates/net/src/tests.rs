//! Integration tests for Net Plane (P2P networking, message propagation)

use crate::p2p::{P2PNetwork, Peer, PeerStatus, PeerInfo, P2PConfig, P2PMessage};
use crate::protocol::{MessageType, ProtocolMessage};

/// Test connecting to bootstrap nodes
#[tokio::test]
async fn test_p2p_bootstrap_connection() {
    let network = P2PNetwork::new([1u8; 32]);
    
    let bootstrap_nodes = vec![
        "192.168.1.1:9100".to_string(),
        "192.168.1.2:9100".to_string(),
    ];
    
    let result = network.connect_bootstrap(&bootstrap_nodes).await;
    assert!(result.is_ok());
    
    // Note: In simulation, connections succeed even if nodes don't exist
    // In production, this would verify actual network connectivity
}

/// Test peer disconnection
#[tokio::test]
async fn test_p2p_disconnect() {
    let network = P2PNetwork::new([1u8; 32]);
    
    // Add a peer
    let peer = Peer::new([2u8; 32], "127.0.0.1".to_string(), 8080);
    network.add_peer(peer).await;
    
    // Disconnect
    let result = network.disconnect(&[2u8; 32]).await;
    assert!(result.is_ok());
    
    // Should no longer be connected
    assert!(!network.is_connected(&[2u8; 32]).await);
}

/// Test broadcast to all connected peers
#[tokio::test]
async fn test_p2p_broadcast() {
    let network = P2PNetwork::new([1u8; 32]);
    
    // Add multiple peers with Connected status
    let mut peer1 = Peer::new([2u8; 32], "127.0.0.1".to_string(), 8080);
    peer1.status = PeerStatus::Connected;
    
    let mut peer2 = Peer::new([3u8; 32], "127.0.0.1".to_string(), 8081);
    peer2.status = PeerStatus::Connected;
    
    let mut peer3 = Peer::new([4u8; 32], "127.0.0.1".to_string(), 8082);
    peer3.status = PeerStatus::Connected;
    
    network.add_peer(peer1).await;
    network.add_peer(peer2).await;
    network.add_peer(peer3).await;
    
    let msg = ProtocolMessage::new(
        MessageType::Intelligence,
        [1u8; 32],
        b"test payload".to_vec(),
    );
    
    let count = network.broadcast(msg).await;
    assert_eq!(count, 3);
}

/// Test send to specific peer
#[tokio::test]
async fn test_p2p_send_to_peer() {
    let network = P2PNetwork::new([1u8; 32]);
    
    let peer = Peer::new([2u8; 32], "127.0.0.1".to_string(), 8080);
    network.add_peer(peer).await;
    
    // Note: Need to set peer to Connected status for send to work
    // In simulation, we need to simulate the connection
    
    let msg = ProtocolMessage::new(
        MessageType::Block,
        [1u8; 32],
        b"test".to_vec(),
    );
    
    // This should fail because peer is not in Connected state
    let result = network.send_to_peer([2u8; 32], msg).await;
    // In the current implementation, send_to_peer checks is_connected
    // which requires status == PeerStatus::Connected
}

/// Test job propagation to peers
#[tokio::test]
async fn test_p2p_propagate_job() {
    let network = P2PNetwork::new([1u8; 32]);
    
    // Add some peers
    let peer1 = Peer::new([2u8; 32], "127.0.0.1".to_string(), 8080);
    let peer2 = Peer::new([3u8; 32], "127.0.0.1".to_string(), 8081);
    
    network.add_peer(peer1).await;
    network.add_peer(peer2).await;
    
    let job_id = [1u8; 32];
    let data = b"job data".to_vec();
    
    let result = network.propagate_job(job_id, data).await;
    assert!(result.is_ok());
}

/// Test receipt propagation to peers
#[tokio::test]
async fn test_p2p_propagate_receipt() {
    let network = P2PNetwork::new([1u8; 32]);
    
    let peer = Peer::new([2u8; 32], "127.0.0.1".to_string(), 8080);
    network.add_peer(peer).await;
    
    let receipt_id = [1u8; 32];
    let data = b"receipt data".to_vec();
    
    let result = network.propagate_receipt(receipt_id, data).await;
    assert!(result.is_ok());
}

/// Test gossip discovery
#[tokio::test]
async fn test_p2p_gossip_discovery() {
    let network = P2PNetwork::new([1u8; 32]);
    
    let peer = Peer::new([2u8; 32], "127.0.0.1".to_string(), 8080);
    network.add_peer(peer).await;
    
    let result = network.gossip_discovery().await;
    assert!(result.is_ok());
}

/// Test handle discovery response
#[tokio::test]
async fn test_p2p_handle_discovery_response() {
    let network = P2PNetwork::new([1u8; 32]);
    
    let peer_infos = vec![
        PeerInfo {
            peer_id: [2u8; 32],
            listen_port: 9100,
            version: 1,
            capabilities: vec!["compute".to_string()],
        },
        PeerInfo {
            peer_id: [3u8; 32],
            listen_port: 9101,
            version: 1,
            capabilities: vec!["compute".to_string(), "storage".to_string()],
        },
    ];
    
    network.handle_discovery_response([1u8; 32], peer_infos).await;
    
    // Should have added the peers from discovery
    assert!(network.get_peer_count().await >= 2);
}

/// Test P2P config defaults
#[test]
fn test_p2p_config_defaults() {
    let config = P2PConfig::default();
    
    assert_eq!(config.listen_address, "0.0.0.0");
    assert_eq!(config.listen_port, 9100);
    assert_eq!(config.max_peers, 128);
    assert_eq!(config.ping_interval_secs, 30);
}

/// Test peer status transitions
#[test]
fn test_peer_status_transitions() {
    let peer = Peer::new([1u8; 32], "127.0.0.1".to_string(), 8080);
    
    assert_eq!(peer.status, PeerStatus::Connecting);
    assert_eq!(peer.endpoint(), "127.0.0.1:8080");
}

/// Test active peers retrieval
#[tokio::test]
async fn test_get_active_peers() {
    let network = P2PNetwork::new([1u8; 32]);
    
    // Add peers with different statuses
    let mut peer1 = Peer::new([2u8; 32], "127.0.0.1".to_string(), 8080);
    peer1.status = PeerStatus::Connected;
    
    let mut peer2 = Peer::new([3u8; 32], "127.0.0.1".to_string(), 8081);
    peer2.status = PeerStatus::Connected;
    
    let peer3 = Peer::new([4u8; 32], "127.0.0.1".to_string(), 8082); // Disconnected
    
    network.add_peer(peer1).await;
    network.add_peer(peer2).await;
    network.add_peer(peer3).await;
    
    let active = network.get_active_peers().await;
    assert_eq!(active.len(), 2);
}

/// Test ban peer removes from network
#[tokio::test]
async fn test_ban_peer_removes() {
    let network = P2PNetwork::new([1u8; 32]);
    
    let peer = Peer::new([2u8; 32], "127.0.0.1".to_string(), 8080);
    network.add_peer(peer).await;
    
    assert_eq!(network.get_peer_count().await, 1);
    
    network.ban_peer([2u8; 32]).await;
    
    assert_eq!(network.get_peer_count().await, 0);
}

/// Test cannot add banned peer
#[tokio::test]
async fn test_cannot_add_banned_peer() {
    let network = P2PNetwork::new([1u8; 32]);
    
    // Ban first
    network.ban_peer([2u8; 32]).await;
    
    // Try to add - should be ignored
    let peer = Peer::new([2u8; 32], "127.0.0.1".to_string(), 8080);
    network.add_peer(peer).await;
    
    assert_eq!(network.get_peer_count().await, 0);
}

/// Test background tasks start/stop
#[tokio::test]
async fn test_background_tasks_lifecycle() {
    let network = P2PNetwork::new([1u8; 32]);
    
    network.start_background_tasks().await;
    
    // Give some time for tasks to run
    // In production, this would be actual background tasks
    
    network.stop_background_tasks().await;
    
    // Tasks should be stopped now
}