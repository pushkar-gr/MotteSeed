//! Block management within pieces.
//!
//! Represents individual blocks of data in a piece, with states and timeouts.

use bytes::Bytes;
use std::time::{Duration, Instant};

/// Structure to represent a block.
#[derive(Debug)]
pub struct Block<'a> {
    pub index: u32,            //index of the piece block belongs to
    pub offset: u64,           //offset of block in the piece
    pub length: u32,           //lenght of data
    pub state: BlockState<'a>, //block state
}

impl<'a> Block<'a> {
    const TIMEOUT: Duration = Duration::from_secs(120);

    /// Creates a new block.
    pub fn new(index: u32, offset: u64, length: u32) -> Self {
        Self {
            index,
            offset,
            length,
            state: BlockState::Missing,
        }
    }

    /// Requests the block from a peer.
    pub fn request_from_peer(&mut self, peer_ip: &'a [u8; 6]) {
        self.state = BlockState::Requested {
            peer_ip,
            instant: Instant::now(),
        }
    }

    /// Calcles the request for the block.
    pub fn cancle(&mut self) {
        self.state = BlockState::Missing;
    }

    /// Resets request if timed out, returning the peer IP.
    pub fn reset(&mut self) -> Option<&'a [u8; 6]> {
        if let BlockState::Requested { peer_ip, instant } = self.state
            && instant.elapsed() > Self::TIMEOUT
        {
            self.cancle();
            return Some(peer_ip);
        }
        None
    }

    /// Receives data for the block.
    pub fn receive_data(&mut self, bytes: Bytes) {
        if bytes.len() as u32 == self.length {
            self.state = BlockState::Received(bytes);
        }
    }

    /// Marks the block as written to disk.
    pub fn mask_written(&mut self) {
        self.state = BlockState::Written;
    }

    /// Checks if the block is compelte.
    pub fn is_complete(&self) -> bool {
        matches!(self.state, BlockState::Received(_) | BlockState::Written)
    }
}

/// Represents block state.
#[derive(Debug)]
pub enum BlockState<'a> {
    Missing, //block needs to be downloaded
    Requested {
        peer_ip: &'a [u8; 6],
        instant: Instant,
    }, //block requested from a peer
    Received(Bytes), //block received from a peer
    Written, //block written to disk
}
