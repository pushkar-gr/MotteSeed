//! Piece structure and management.piece.rs
//!
//! Represents a piece of a torrent, consisting of multiple blocks, and handles
//! piece verification using SHA-1 hashes.

use super::block::{Block, BlockState};

use bytes::{BufMut, Bytes, BytesMut};
use sha1::{Digest, Sha1};
use std::collections::HashMap;

/// Represents a piece in a torrent.
///
/// A piece is a fixed-size chunk of the torrent data (typically 256 KiB or larger).
/// Each piece is divided into blocks for transfer and has a SHA-1 hash for verification.
#[derive(Debug)]
pub struct Piece<'a> {
    /// Zero-based index of this piece in the torrent.
    pub index: u32,
    /// Length of the piece in bytes.
    pub length: u64,
    /// Expected SHA-1 hash of the complete piece data.
    pub hash: &'a [u8; 20],
    /// Map of block offset to Block, containing all blocks in this piece.
    pub blocks: HashMap<u64, Block>,
    /// Current state of the piece.
    pub state: PieceState,
    /// Size of each block in bytes (except possibly the last block).
    pub block_size: u32,
    /// Download priority for this piece (higher is more urgent).
    pub priority: u8,
}

impl<'a> Piece<'a> {
    /// Creates a new piece with blocks.
    ///
    /// Divides the piece into blocks of the specified size and initializes them
    /// in the Missing state.
    ///
    /// # Arguments
    ///
    /// * `index` - The piece index in the torrent
    /// * `length` - The total length of the piece in bytes
    /// * `hash` - The expected SHA-1 hash for verification
    /// * `block_size` - The size of each block (typically 16 KiB)
    ///
    /// # Returns
    ///
    /// Returns a new Piece instance with all blocks initialized.
    pub fn new(index: u32, length: u64, hash: &'a [u8; 20], block_size: u32) -> Self {
        let mut blocks = HashMap::new();
        let mut offset: u64 = 0;

        //create blocks for the piece
        while offset < length {
            let block_length = std::cmp::min(block_size, (length - offset) as u32);
            blocks.insert(offset, Block::new(index, offset, block_length));
            offset += block_length as u64;
        }

        Self {
            index,
            length,
            hash,
            blocks,
            state: PieceState::Missing,
            block_size,
            priority: 1, //default priority
        }
    }

    /// Marks a block as requested from a peer.
    ///
    /// # Arguments
    ///
    /// * `offset` - The byte offset of the block within the piece
    /// * `peer_ip` - The peer from which the block is being requested
    pub fn request_from_peer(&mut self, offset: u64, peer_ip: [u8; 6]) {
        if let Some(block) = self.blocks.get_mut(&offset) {
            block.request_from_peer(peer_ip);
        }
    }

    /// Checks if all blocks in the piece have been downloaded.
    ///
    /// # Returns
    ///
    /// Returns `true` if all blocks are complete, `false` otherwise.
    pub fn is_fully_downloaded(&self) -> bool {
        self.blocks.values().all(|block| block.is_complete())
    }

    /// Verifies the piece against its expected SHA-1 hash.
    ///
    /// Combines all block data, computes the SHA-1 hash, and compares it
    /// to the expected hash. If verification fails, all blocks are reset to Missing.
    ///
    /// # Returns
    ///
    /// Returns `true` if the piece hash matches, `false` otherwise.
    pub fn verify(&mut self) -> bool {
        let mut buf = BytesMut::with_capacity(self.length as usize);

        //combine blocks to get piece data
        let mut offset = 0;
        while offset < self.length {
            if let Some(block) = self.blocks.get(&offset) {
                if let BlockState::Received(bytes) = &block.state {
                    buf.put(bytes.clone());
                    offset += self.block_size as u64;
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }

        //calculate hash
        let mut hasher = Sha1::new();
        hasher.update(&buf);
        let calculated_hash = hasher.finalize();

        //check hash
        if calculated_hash.as_slice() == self.hash {
            self.state = PieceState::Complete;
            true
        } else {
            //mark all block as missing
            for block in self.blocks.values_mut() {
                block.cancle();
            }
            self.state = PieceState::Missing;
            false
        }
    }

    /// Receives a block of data from a peer.
    ///
    /// Stores the block data and triggers piece verification if all blocks
    /// have been received.
    ///
    /// # Arguments
    ///
    /// * `offset` - The byte offset of the block within the piece
    /// * `data` - The block data received
    ///
    /// # Returns
    ///
    /// Returns `true` if the block was successfully stored, `false` if the offset is invalid.
    pub fn receive_block(&mut self, offset: u64, data: Bytes) -> bool {
        if let Some(block) = self.blocks.get_mut(&offset) {
            block.receive_data(data);

            if self.is_fully_downloaded() {
                self.verify();
            }

            true
        } else {
            false
        }
    }

    /// Marks the piece as written to disk.
    ///
    /// Changes the piece state to Written, indicating the verified data
    /// has been persisted.
    pub fn mask_written(&mut self) {
        self.state = PieceState::Written;
    }

    /// Gets the offset of the next missing block.
    ///
    /// Searches for a block that hasn't been completed yet.
    ///
    /// # Returns
    ///
    /// Returns `Some(offset)` if there's a missing block, `None` if all blocks are complete.
    pub fn get_next_missing_block(&self) -> Option<u64> {
        for (offset, block) in &self.blocks {
            if !block.is_complete() {
                return Some(*offset);
            }
        }
        None
    }

    /// Resets timed-out block requests.
    ///
    /// Checks all blocks for timeouts and resets them to Missing state.
    ///
    /// # Returns
    ///
    /// Returns a map of block offsets to peer IPs for requests that timed out.
    pub fn reset_timeout_requests(&mut self) -> HashMap<u64, [u8; 6]> {
        let mut reset_ip = HashMap::new();

        for (offset, block) in &mut self.blocks {
            if let Some(ip) = block.reset() {
                reset_ip.insert(*offset, ip);
            }
        }

        reset_ip
    }

    /// Checks if the piece is complete.
    ///
    /// # Returns
    ///
    /// Returns `true` if the piece is in Complete or Written state, `false` otherwise.
    pub fn is_complete(&self) -> bool {
        matches!(self.state, PieceState::Complete | PieceState::Written)
    }

    /// Gets the complete piece data.
    ///
    /// Combines all block data into a single Bytes object.
    ///
    /// # Returns
    ///
    /// Returns `Some(Bytes)` containing the complete piece data if the piece
    /// is in Complete state, `None` otherwise.
    pub fn get_piece_data(&self) -> Option<Bytes> {
        if self.state == PieceState::Complete {
            let mut data = Vec::with_capacity(self.length as usize);
            let mut offset = 0;
            while offset < self.length {
                if let Some(block) = self.blocks.get(&offset) {
                    if let BlockState::Received(bytes) = &block.state {
                        data.extend_from_slice(&bytes);
                        offset += block.length as u64;
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }

            return Some(Bytes::from(data));
        }
        None
    }
}

/// Represents the state of a piece.
#[derive(Debug, Clone, PartialEq)]
pub enum PieceState {
    /// Piece needs to be downloaded (no blocks received yet).
    Missing,
    /// Some blocks have been downloaded but the piece is not complete.
    Incomplete,
    /// All blocks have been downloaded and hash verified successfully.
    Complete,
    /// Piece has been written to disk.
    Written,
}
