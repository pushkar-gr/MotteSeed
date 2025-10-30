//! Piece structure and management.
//!
//! Represents a piece of torrent, with blocks and verification.

use crate::core::peer::piece::block::{Block, BlockState};

use bytes::{BufMut, Bytes, BytesMut};
use sha1::{Digest, Sha1};
use std::collections::HashMap;

/// Structure to represent a piece.
#[derive(Debug)]
pub struct Piece<'a> {
    pub index: u32,                      //piece index
    pub length: u32,                     //piece length
    pub hash: &'a [u8; 20],              //SHA1 of piece
    pub blocks: HashMap<u32, Block<'a>>, //blocks
    pub state: PieceState,               //piece state
    pub block_size: u32,                 //block size
    pub priority: u8,                    //piece priority
}

impl<'a> Piece<'a> {
    /// Creates a new piece with blocks.
    pub fn new(index: u32, length: u32, hash: &'a [u8; 20], block_size: u32) -> Self {
        let mut blocks = HashMap::new();
        let mut offset = 0;

        //create blocks for the piece
        while offset < length {
            let block_length = std::cmp::min(block_size, length - offset);
            blocks.insert(offset, Block::new(index, offset, block_length));
            offset += block_length;
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

    /// Requests a block from a peer.
    pub fn request_from_peer(&mut self, offset: u32, peer_ip: &'a [u8; 6]) {
        if let Some(block) = self.blocks.get_mut(&offset) {
            block.request_from_peer(peer_ip);
        }
    }

    /// Check if all blocks are downloaded.
    pub fn is_fully_downloaded(&self) -> bool {
        self.blocks.values().all(|block| block.is_complete())
    }

    /// Verifies the piece with SHA1 hash.
    pub fn verify(&mut self) -> bool {
        let mut buf = BytesMut::with_capacity(self.length as usize);

        //combine blocks to get piece data
        let mut offset = 0;
        while offset < self.length {
            if let Some(block) = self.blocks.get(&offset) {
                if let BlockState::Received(bytes) = &block.state {
                    buf.put(bytes.clone());
                    offset += self.block_size;
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

    /// Receives a block from peer.
    pub fn receive_block(&mut self, offset: u32, data: Bytes) -> bool {
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

    /// Marks the  piece as written.
    pub fn mask_written(&mut self) {
        self.state = PieceState::Written;
    }

    /// Gets the next missing block offset.
    pub fn get_next_missing_block(&self) -> Option<u32> {
        for (offset, block) in &self.blocks {
            if !block.is_complete() {
                return Some(*offset);
            }
        }
        None
    }

    /// Resets timeout requests, returning reset IPs.
    pub fn reset_timeout_requests(&mut self) -> HashMap<u32, &'a [u8; 6]> {
        let mut reset_ip = HashMap::new();

        for (offset, block) in &mut self.blocks {
            if let Some(ip) = block.reset() {
                reset_ip.insert(*offset, ip);
            }
        }

        reset_ip
    }

    /// Checks if piece is compelte.
    pub fn is_complete(&self) -> bool {
        matches!(self.state, PieceState::Complete | PieceState::Written)
    }
}

/// Represents piece state.
#[derive(Debug, Clone, PartialEq)]
pub enum PieceState {
    Missing,    //piece needs to be downloaded
    Incomplete, //some blocks downloaded but not complete
    Complete,   //all blocks downloaded and verified
    Written,    //piece written to disk
}
