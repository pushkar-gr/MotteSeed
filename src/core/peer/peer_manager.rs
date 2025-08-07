use crate::core::peer::peer_connection::PeerConnection;

use std::collections::HashSet;

#[derive(Debug)]
pub struct PeerManager {
    peer_connections: HashSet<[u8; 6], PeerConnection>,
}
