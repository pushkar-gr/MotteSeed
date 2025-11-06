//! Peer manager.
//!
//! Manages multiple peer connections for a torrent, coordinating download/upload
//! activities across all connected peers. Uses async channels for inter-task communication
//! and spawns independent tasks for each peer connection.

use crate::core::peer::peer_connection::{ManagerCommand, PeerConnection, PeerEvent};

use bytes::Bytes;
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Manages all peer connections for a torrent.
///
/// This structure maintains a collection of active peer connections,
/// coordinating piece requests and data transfer across multiple peers.
/// Each peer is represented by its 6-byte address (4 bytes IP + 2 bytes port).
///
/// The PeerManager uses async channels to communicate with individual peer connections:
/// - Maintains a map of peer senders to send commands to each peer
/// - Receives events from peers through a shared event channel
/// - Spawns independent tokio tasks for each peer connection
///
/// # Example
///
/// ```no_run
/// # use MotteSeed::core::peer::peer_manager::PeerManager;
/// # use tokio::sync::mpsc;
/// # async fn example() {
/// let (download_tx, mut download_rx) = mpsc::channel(100);
/// let mut manager = PeerManager::new([0u8; 20], [0u8; 20], download_tx);
///
/// // Add a peer connection
/// manager.add_peer([192, 168, 1, 1, 0x18, 0x89], None);
///
/// // Send a command to the peer
/// let peer_addr = [192, 168, 1, 1, 0x18, 0x89];
/// manager.send_to_peer(&peer_addr, ManagerCommand::Interested).await;
/// # }
/// ```
#[derive(Debug)]
pub struct PeerManager {
    /// Map of peer addresses (6 bytes: 4 IP + 2 port) to their command channels.
    /// Used to send commands to specific peers.
    peer_senders: HashMap<[u8; 6], mpsc::Sender<ManagerCommand>>,

    /// Channel sender for download events.
    /// All peer connections send their events through this channel,
    /// tagged with the peer address for identification.
    download_event_tx: mpsc::Sender<(PeerEvent, [u8; 6])>,

    /// Our 20-byte peer ID in Azureus format.
    /// Used during handshake with peers to identify this client.
    peer_id: [u8; 20],

    /// SHA-1 hash of the torrent info dictionary.
    /// Used during handshake to verify we're downloading the same torrent.
    info_hash: [u8; 20],
}

impl PeerManager {
    /// Creates a new peer manager.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - Our 20-byte peer ID used in handshakes
    /// * `info_hash` - The 20-byte SHA-1 hash of the torrent info dictionary
    /// * `download_event_tx` - Channel sender for peer events with peer addresses
    ///
    /// # Returns
    ///
    /// Returns a new PeerManager instance with no connected peers.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::peer::peer_manager::PeerManager;
    /// # use tokio::sync::mpsc;
    /// let (tx, _rx) = mpsc::channel(100);
    /// let manager = PeerManager::new([0u8; 20], [0u8; 20], tx);
    /// ```
    pub fn new(
        peer_id: [u8; 20],
        info_hash: [u8; 20],
        download_event_tx: mpsc::Sender<(PeerEvent, [u8; 6])>,
    ) -> Self {
        Self {
            peer_senders: HashMap::new(),
            download_event_tx,
            peer_id,
            info_hash,
        }
    }

    /// Adds a new peer connection.
    ///
    /// Establishes a connection to a peer, performs the BitTorrent handshake,
    /// and spawns a background task to handle peer communication.
    ///
    /// The peer connection is managed independently in a spawned tokio task:
    /// - Performs handshake to authenticate and exchange peer information
    /// - Sends optional bitfield on connection (indicating which pieces we have)
    /// - Listens for incoming messages from the peer
    /// - Forwards peer events to the manager through the download event channel
    ///
    /// # Arguments
    ///
    /// * `peer_ip` - 6-byte peer address (4 bytes IPv4 + 2 bytes port in big-endian)
    /// * `bitfield` - Optional bitfield indicating which pieces we have.
    ///               If provided, this will be sent immediately after connection.
    ///               Format: Each bit represents a piece (1 = have, 0 = don't have)
    ///
    /// # Behavior on Failure
    ///
    /// If the handshake or connection fails, an error is logged but the peer is not
    /// tracked by the manager. Failed connections are silently dropped without affecting
    /// other peer connections.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::peer::peer_manager::PeerManager;
    /// # use bytes::Bytes;
    /// # use tokio::sync::mpsc;
    /// # async fn example() {
    /// # let (tx, _rx) = mpsc::channel(100);
    /// # let mut manager = PeerManager::new([0u8; 20], [0u8; 20], tx);
    /// // Add peer without bitfield
    /// manager.add_peer([192, 168, 1, 1, 0x18, 0x89], None);
    ///
    /// // Add peer with bitfield
    /// let bitfield = Bytes::from(vec![0xFF, 0x00]); // Have pieces 0-7, don't have 8-15
    /// manager.add_peer([192, 168, 1, 2, 0x18, 0x89], Some(bitfield));
    /// # }
    /// ```
    pub fn add_peer(&mut self, peer_ip: [u8; 6], bitfield: Option<Bytes>) {
        // Create channels for bidirectional communication with the peer
        let (to_manager_tx, mut to_manager_rx) = mpsc::channel::<PeerEvent>(100);
        let (from_manager_tx, from_manager_rx) = mpsc::channel::<ManagerCommand>(100);

        self.peer_senders.insert(peer_ip, from_manager_tx);

        // Clone shared data for the peer task
        let download_tx = self.download_event_tx.clone();
        let peer_id = self.peer_id;
        let info_hash = self.info_hash;

        // Spawn independent task for this peer connection
        tokio::spawn(async move {
            // Attempt to establish connection and perform handshake
            match PeerConnection::new(
                &peer_ip,
                to_manager_tx,
                from_manager_rx,
                &peer_id,
                &info_hash,
            )
            .await
            {
                Ok(mut conn) => {
                    // Spawn task to run the peer connection loop
                    tokio::spawn(async move {
                        let _ = conn.run(bitfield).await;
                    });
                }
                Err(e) => {
                    // Log connection failure and exit
                    eprintln!("Failed to connect to peer {:?}: {}", peer_ip, e);
                    return;
                }
            }

            // Event forwarding loop: relay all peer events to the manager
            // This loop continues until the peer disconnects or the manager drops the channel
            while let Some(event) = to_manager_rx.recv().await {
                // Forward event with peer address for identification
                if download_tx.send((event, peer_ip)).await.is_err() {
                    // Manager has dropped the receiver, exit the loop
                    break;
                }
            }
        });
    }

    /// Gets the command sender for a specific peer.
    ///
    /// Retrieves the async channel sender to send commands to a specific peer. Can be used by other components to directly send commands to the peer.
    ///
    /// # Arguments
    ///
    /// * `peer_ip` - The 6-byte peer address to get the sender for
    ///
    /// # Returns
    ///
    /// Returns `Some(sender)` if the peer is connected, `None` if the peer
    /// is not found or has disconnected.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::peer::peer_manager::PeerManager;
    /// # use MotteSeed::core::peer::peer_connection::ManagerCommand;
    /// # use tokio::sync::mpsc;
    /// # async fn example(manager: &PeerManager) {
    /// let peer_addr = [192, 168, 1, 1, 0x18, 0x89];
    /// if let Some(sender) = manager.get_peer_sender(&peer_addr) {
    ///     let _ = sender.send(ManagerCommand::Interested).await;
    /// }
    /// # }
    /// ```
    pub fn get_peer_sender(&self, peer_ip: &[u8; 6]) -> Option<mpsc::Sender<ManagerCommand>> {
        self.peer_senders.get(peer_ip).cloned()
    }

    /// Sends a command to a specific peer.
    ///
    /// Asynchronously sends a command to the peer connection for it to handle.
    /// The command is queued in the peer's channel and processed by the peer task.
    ///
    /// Common commands include:
    /// - `Interested`: Tell peer we want their pieces
    /// - `RequestBlock`: Request a specific block of data
    /// - `Unchoke`: Tell peer we're ready to upload to them
    /// - `Choke`: Tell peer we're not uploading to them
    ///
    /// # Arguments
    ///
    /// * `peer_ip` - The 6-byte address of the target peer
    /// * `command` - The command to send to the peer
    ///
    /// # Behavior on Failure
    ///
    /// If the peer is not found or the channel is closed, the command is silently dropped.
    /// This prevents panics if a peer disconnects during command sending.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::peer::peer_manager::PeerManager;
    /// # use MotteSeed::core::peer::peer_connection::ManagerCommand;
    /// # use tokio::sync::mpsc;
    /// # async fn example(manager: &PeerManager) {
    /// let peer_addr = [192, 168, 1, 1, 0x18, 0x89];
    /// manager.send_to_peer(&peer_addr, ManagerCommand::Interested).await;
    /// manager.send_to_peer(&peer_addr, ManagerCommand::RequestBlock {
    ///     index: 0,
    ///     begin: 0,
    ///     length: 16384,
    /// }).await;
    /// # }
    /// ```
    pub async fn send_to_peer(&self, peer_ip: &[u8; 6], command: ManagerCommand) {
        if let Some(sender) = self.peer_senders.get(peer_ip) {
            let _ = sender.send(command).await;
        }
    }
}
