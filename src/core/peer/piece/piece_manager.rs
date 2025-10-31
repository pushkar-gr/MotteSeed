//! Piece manager module.
//!
//! Manages the state and selection and pieces for downloading in a torrent.

use super::piece::{Piece, PieceState};
use crate::core::torrent::torrent::Torrent;

use std::collections::HashMap;

/// Manages pieces for the torrent.
///
/// Tracks the state of each pieces and provides methods to select pieces for download and retrieve
/// completed pieces.
#[derive(Debug)]
pub struct PieceManager<'a> {
    /// The collection of pieces in the torrent, keyed by piece index.
    pub pieces: HashMap<u32, Piece<'a>>,
    /// References to the torrent metadata.
    torrent: &'a Torrent<'a>,
    /// The size of each block within a piece.
    block_size: u32,
}

impl<'a> PieceManager<'a> {
    /// Creates a new `PieceManager` for the torrent.
    ///
    /// Initializes pieces based on the torrent's piece hashes and length
    ///
    /// # Arguments
    ///
    /// * `torrent` - The torrent metadata.
    /// * `block_size` - The size of each block in bytes.
    ///
    ///  # Returns
    ///
    ///  A new `PieceManager` instance.
    pub fn new(torrent: &'a Torrent<'a>, block_size: u32) -> Self {
        let mut pieces = HashMap::new();

        for i in 0..torrent.info.raw_pieces.len() / 20 {
            if let Some(piece_hash) = torrent.info.piece_hash(i) {
                pieces.insert(
                    i as u32,
                    Piece::new(i as u32, torrent.info.piece_length, piece_hash, block_size),
                );
            }
        }

        Self {
            pieces,
            torrent,
            block_size,
        }
    }

    /// Returns the next piece index to download from the available pieces.
    ///
    /// Selects the first piece that is either missing or incompelte.
    ///
    /// # Arguments
    ///
    /// * `available_pieces` - A list of piece indices available from peers.
    ///
    /// # Returns
    ///
    /// The piece index to download, or `None` if no suitable piece is found.
    pub fn get_next_piece_to_download(&self, available_pieces: &[u32]) -> Option<u32> {
        for piece_index in available_pieces {
            if let Some(piece) = self.pieces.get(piece_index) {
                if matches!(piece.state, PieceState::Missing | PieceState::Incomplete) {
                    return Some(*piece_index);
                }
            }
        }
        None
    }

    /// Returns a list of completed piece indeces.
    ///
    /// # Returns
    ///
    /// A vector of piece indices that are fully downloaded and verified.
    pub fn get_completed_pieces(&self) -> Vec<u32> {
        self.pieces
            .iter()
            .filter_map(|(index, piece)| {
                if piece.is_complete() {
                    Some(*index)
                } else {
                    None
                }
            })
            .collect()
    }
}
