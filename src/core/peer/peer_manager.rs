//! Peer manager.
//!
//! Manages multiple peer connections for a torrent, coordinating download/upload
//! activities across all connected peers.

use crate::core::peer::peer_connection::PeerConnection;

use std::collections::HashSet;

/// Manages all peer connections for a torrent.
///
/// This structure maintains a collection of active peer connections,
/// coordinating piece requests and data transfer across multiple peers.
#[derive(Debug)]
pub struct PeerManager {
    /// Map of peer addresses (6 bytes: 4 IP + 2 port) to their connection handlers.
    peer_connections: HashSet<[u8; 6], PeerConnection>,
}
