//! Download manager.
//!
//! Coordinates downloads across connected peers for a single torrent.
//!
//! The DownloadManager listens for peer events (bitfields, have messages,
//! unchokes, piece blocks, disconnects) and drives piece/block selection
//! through the PieceManager. It also exposes methods to register peers and
//! a DiskIO instance for persisting verified pieces.
//!
//! Design notes:
//! - Peers are identified by a compact 6-byte representation: 4 bytes IPv4 + 2 bytes port (big-endian).
//! - Communication with peers is performed through Tokio MPSC channels using the
//!   ManagerCommand/PeerEvent types defined by the peer module.
//! - The DownloadManager does minimal I/O itself; it delegates piece state to
//!   PieceManager and disk operations to DiskIO to keep separation of concerns.

use super::{peer_connection::ManagerCommand, piece::piece_manager::PieceManager};
use crate::core::{
    disk_io::{self, DiskIO},
    peer::peer_connection::PeerEvent,
    torrent::torrent::Torrent,
};

use bytes::Bytes;
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Manages piece selection and peer coordination for a torrent.
#[derive(Debug)]
pub struct DownloadManager<'a> {
    /// In-memory piece state and selection logic.
    piece_manager: PieceManager<'a>,

    /// Registered peers mapped to their manager command sender and the
    /// last-seen bitfield (if any).
    peers: HashMap<[u8; 6], (mpsc::Sender<ManagerCommand>, Option<Bytes>)>,

    /// Receiver for events produced by peer connections. Each event is tagged
    /// with the peer address that generated it.
    peer_event_rx: mpsc::Receiver<(PeerEvent, [u8; 6])>,

    /// Optional DiskIO handle used to persist verified pieces to disk.
    disk_io: Option<&'a DiskIO<'a>>,
}

impl<'a> DownloadManager<'a> {
    /// Creates a new DownloadManager.
    ///
    /// # Arguments
    ///
    /// * `torrent` - Reference to the parsed torrent metadata.
    /// * `block_size` - Size of transfer blocks (typically 16 KiB).
    /// * `peer_event_rx` - Receiver channel for peer events.
    ///
    /// # Returns
    ///
    /// A DownloadManager ready to accept peers (not yet running).
    pub fn new(
        torrent: &'a Torrent<'a>,
        block_size: u32,
        peer_event_rx: mpsc::Receiver<(PeerEvent, [u8; 6])>,
    ) -> Self {
        Self {
            piece_manager: PieceManager::new(torrent, block_size),
            peers: HashMap::new(),
            peer_event_rx,
            disk_io: None,
        }
    }

    /// Sets the DiskIO instance used to write completed pieces to disk.
    ///
    /// This must be set before the manager can persist verified pieces.
    pub fn set_disk_io(&mut self, disk_io: &'a DiskIO<'a>) {
        self.disk_io = Some(disk_io);
    }

    /// Registers a peer with the download manager.
    ///
    /// The `sender` is the channel used to send ManagerCommand values to the peer
    /// connection task. The optional bitfield will be stored when received from the peer.
    ///
    /// # Arguments
    ///
    /// * `peer_ip` - 6-byte peer address (4 bytes IPv4 + 2 bytes port big-endian).
    /// * `sender` - Channel sender to forward ManagerCommand messages to the peer task.
    pub fn add_peer(&mut self, peer_ip: [u8; 6], sender: mpsc::Sender<ManagerCommand>) {
        self.peers.insert(peer_ip, (sender, None));
    }

    /// Removes a peer from the manager (e.g., on disconnect).
    ///
    /// # Arguments
    ///
    /// * `peer_ip` - 6-byte peer address to remove.
    pub fn remove_peer(&mut self, peer_ip: &[u8; 6]) {
        self.peers.remove(peer_ip);
    }

    /// Runs the main event loop of the DownloadManager.
    ///
    /// Listens for peer events and updates piece state, issues requests,
    /// and persists verified pieces. This method runs until the download
    /// completes (all pieces verified/written) or the peer event channel
    /// is closed.
    pub async fn run(&mut self) {
        loop {
            if let Some((event, peer_ip)) = self.peer_event_rx.recv().await {
                match event {
                    PeerEvent::ReceivedBitfield(bitfield) => {
                        self.handle_bitfield(peer_ip, bitfield).await;
                    }
                    PeerEvent::ReceivedHave(piece_index) => {
                        self.handle_have(peer_ip, piece_index).await;
                    }
                    PeerEvent::PeerUnchoked => {
                        self.handle_unchoke(peer_ip).await;
                    }
                    PeerEvent::ReceivedPiece {
                        index,
                        begin,
                        block,
                    } => {
                        self.handle_recieve_piece(peer_ip, index, begin as u64, block)
                            .await;
                    }
                    PeerEvent::PeerDisconnected => {
                        self.remove_peer(&peer_ip);
                    }
                    _ => {}
                }
            }

            // Check for completion after processing each event batch.
            if self.piece_manager.is_download_complete() {
                println!("Download compelte!");
                break;
            }
        }
    }

    /// Returns the list of piece indices a peer has, parsed from its stored bitfield.
    ///
    /// If no bitfield has been received for the peer, returns an empty Vec.
    fn get_peer_available_pieces(&self, peer_ip: [u8; 6]) -> Vec<u32> {
        if let Some((_, Some(bitfield))) = self.peers.get(&peer_ip) {
            let mut pieces = Vec::new();
            for (byte_index, byte) in bitfield.iter().enumerate() {
                for bit_index in 0..8 {
                    if byte & (1 << (7 - bit_index)) != 0 {
                        let piece_index = (byte_index * 8 + bit_index) as u32;
                        pieces.push(piece_index);
                    }
                }
            }
            pieces
        } else {
            Vec::new()
        }
    }

    /// Returns `true` if the peer has any pieces we still need.
    fn peer_has_needed_pieces(&self, peer_ip: [u8; 6]) -> bool {
        let available = self.get_peer_available_pieces(peer_ip);
        self.piece_manager
            .get_next_piece_to_download(&available)
            .is_some()
    }

    /// Handles an incoming Bitfield message from a peer.
    ///
    /// Updates the stored bitfield and sends an Interested command if the peer
    /// has pieces we need.
    async fn handle_bitfield(&mut self, peer_ip: [u8; 6], bitfield: Bytes) {
        if let Some((_, peer_bitfield)) = self.peers.get_mut(&peer_ip) {
            *peer_bitfield = Some(bitfield.clone());
        }

        if self.peer_has_needed_pieces(peer_ip) {
            if let Some((sender, _)) = self.peers.get(&peer_ip) {
                let _ = sender.send(ManagerCommand::Interested).await;
            }
        }
    }

    /// Handles a peer's Have message (announcing availability of a piece).
    ///
    /// Updates the stored bitfield for the peer (if present) to mark the piece as available.
    ///
    /// # Arguments
    ///
    /// * `peer_ip` - Peer address that sent the have message.
    /// * `piece_index` - Index of the piece the peer reported having.
    pub async fn handle_have(&mut self, peer_ip: [u8; 6], piece_index: u32) {
        if let Some((_, bitfield)) = self.peers.get_mut(&peer_ip) {
            if let Some(bf) = bitfield {
                let byte_index = piece_index / 8;
                let bit_index = 7 - (piece_index % 8);
                if byte_index < bf.len() as u32 {
                    let mut bf_vec = bf.to_vec();
                    bf_vec[byte_index as usize] |= 1 << bit_index;
                    *bitfield = Some(Bytes::from(bf_vec));
                }
            }
        }
    }

    /// Requests the next missing block for `piece_index` from `peer_ip` if available.
    ///
    /// This marks the block as requested in PieceManager and sends a RequestBlock
    /// ManagerCommand to the peer.
    async fn request_next_block_for_piece(&mut self, peer_ip: [u8; 6], piece_index: u32) {
        if let Some((offset, length)) = self.piece_manager.get_next_block_for_piece(piece_index) {
            self.piece_manager
                .request_block(piece_index, offset, peer_ip);

            if let Some((sender, _)) = self.peers.get(&peer_ip) {
                let _ = sender
                    .send(ManagerCommand::RequestBlock {
                        index: piece_index,
                        begin: offset as u32,
                        length,
                    })
                    .await;
            }
        }
    }

    /// Selects a piece available from the peer and requests its next block.
    async fn request_pieces_from_peer(&mut self, peer_ip: [u8; 6]) {
        let available_pieces = self.get_peer_available_pieces(peer_ip);

        if let Some(piece_index) = self
            .piece_manager
            .get_next_piece_to_download(&available_pieces)
        {
            self.request_next_block_for_piece(peer_ip, piece_index)
                .await;
        }
    }

    /// Called when a peer unchokes us; attempt to request work from them.
    ///
    /// # Arguments
    ///
    /// * `peer_ip` - Peer address that unchoked us.
    pub async fn handle_unchoke(&mut self, peer_ip: [u8; 6]) {
        self.request_pieces_from_peer(peer_ip).await;
    }

    /// Broadcast a Have message to all connected peers.
    ///
    /// Used after successfully persisting a piece to signal other peers we have it.
    async fn broadcast_have(&self, piece_index: u32) {
        for (sender, _) in self.peers.values() {
            let _ = sender.send(ManagerCommand::HavePiece(piece_index)).await;
        }
    }

    /// Handles a ReceivedPiece event from a peer.
    ///
    /// Stores the block in PieceManager and, if the piece becomes complete,
    /// verifies and writes it to disk using DiskIO (if configured). On successful
    /// disk write the piece is marked written and a Have message is broadcast.
    ///
    /// # Arguments
    ///
    /// * `peer_ip` - The peer that sent the block.
    /// * `piece_index` - Piece index.
    /// * `begin` - Byte offset within the piece.
    /// * `block` - Block data provided by the peer.
    pub async fn handle_recieve_piece(
        &mut self,
        peer_ip: [u8; 6],
        piece_index: u32,
        begin: u64,
        block: Bytes,
    ) {
        let piece_complete = self.piece_manager.receive_block(piece_index, begin, block);

        if piece_complete {
            if let Some(disk_io) = self.disk_io {
                if let Some(data) = self.piece_manager.get_piece_data(piece_index) {
                    if let Ok(_) = disk_io.write_piece(piece_index, &data).await {
                        self.piece_manager.mark_piece_written(piece_index);

                        self.broadcast_have(piece_index).await;
                    }
                }
            }
        }
    }
}
