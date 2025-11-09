//! Block management within pieces.
//!
//! Represents individual blocks of data within a piece, tracking their state,
//! requests, and timeouts. Blocks are the smallest unit of data transfer in BitTorrent.

use bytes::Bytes;
use std::time::{Duration, Instant};

/// Represents a block within a piece.
///
/// Blocks are typically 16 KiB in size (except possibly the last block in a piece).
/// Each block tracks its download state and the peer from which it was requested.
#[derive(Debug)]
pub struct Block {
    /// Index of the piece this block belongs to.
    pub index: u32,
    /// Byte offset of this block within the piece.
    pub offset: u64,
    /// Length of the block data in bytes.
    pub length: u32,
    /// Current state of the block.
    pub state: BlockState,
}

impl Block {
    /// Timeout duration for block requests (2 minutes).
    const TIMEOUT: Duration = Duration::from_secs(120);

    /// Creates a new block in the Missing state.
    ///
    /// # Arguments
    ///
    /// * `index` - The piece index this block belongs to
    /// * `offset` - The byte offset within the piece
    /// * `length` - The block length in bytes
    ///
    /// # Returns
    ///
    /// A newly initialized Block in Missing state.
    pub fn new(index: u32, offset: u64, length: u32) -> Self {
        Self {
            index,
            offset,
            length,
            state: BlockState::Missing,
        }
    }

    /// Marks the block as requested from a peer.
    ///
    /// Changes the block state to Requested, recording which peer and when.
    ///
    /// # Arguments
    ///
    /// * `peer_ip` - The 6-byte peer address (4 bytes IP + 2 bytes port)
    pub fn request_from_peer(&mut self, peer_ip: [u8; 6]) {
        self.state = BlockState::Requested {
            peer_ip,
            instant: Instant::now(),
        }
    }

    /// Cancels the request for this block.
    ///
    /// Reverts the block state to Missing, typically called when a request times out
    /// or when piece verification fails.
    pub fn cancle(&mut self) {
        self.state = BlockState::Missing;
    }

    /// Resets the block if the request has timed out.
    ///
    /// Checks if the block is in Requested state and if the timeout has elapsed.
    /// If so, cancels the request and returns the peer IP.
    ///
    /// # Returns
    ///
    /// Returns `Some(peer_ip)` if the request timed out and was cancelled,
    /// otherwise `None`.
    pub fn reset(&mut self) -> Option<[u8; 6]> {
        if let BlockState::Requested { peer_ip, instant } = self.state
            && instant.elapsed() > Self::TIMEOUT
        {
            self.cancle();
            return Some(peer_ip);
        }
        None
    }

    /// Receives data for this block.
    ///
    /// Stores the received data if its length matches the expected block length.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The block data received from a peer
    pub fn receive_data(&mut self, bytes: Bytes) {
        if bytes.len() as u32 == self.length {
            self.state = BlockState::Received(bytes);
        }
    }

    /// Marks the block as written to disk.
    ///
    /// Changes the block state to Written, indicating the data has been
    /// successfully persisted.
    pub fn mask_written(&mut self) {
        self.state = BlockState::Written;
    }

    /// Checks if the block is complete.
    ///
    /// # Returns
    ///
    /// Returns `true` if the block has been received or written, `false` otherwise.
    pub fn is_complete(&self) -> bool {
        matches!(self.state, BlockState::Received(_) | BlockState::Written)
    }
}

/// Represents the state of a block.
#[derive(Debug)]
pub enum BlockState {
    /// Block needs to be downloaded.
    Missing,
    /// Block has been requested from a peer.
    Requested {
        /// The peer from which the block was requested.
        peer_ip: [u8; 6],
        /// When the request was sent.
        instant: Instant,
    },
    /// Block data has been received from a peer.
    Received(Bytes),
    /// Block has been written to disk.
    Written,
}
