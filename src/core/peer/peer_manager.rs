//! Peer manager.
//!
//! Manages multiple peer connection.
use crate::core::peer::peer_connection::PeerConnection;

use std::collections::HashSet;

/// Manages peer connections.
#[derive(Debug)]
pub struct PeerManager {
    peer_connections: HashSet<[u8; 6], PeerConnection>,
}
