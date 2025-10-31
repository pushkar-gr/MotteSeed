//! Piece manager module.
//!
//! Manages the state and selection of pieces for downloading in a torrent.
//! Coordinates piece downloads and tracks completion status.

use super::piece::{Piece, PieceState};
use crate::core::torrent::torrent::Torrent;

use std::collections::HashMap;

/// Manages all pieces for a torrent.
///
/// Tracks the state of each piece in the torrent and provides methods to select
/// pieces for download, retrieve completed pieces, and manage the overall download progress.
#[derive(Debug)]
pub struct PieceManager<'a> {
    /// Collection of all pieces in the torrent, keyed by piece index.
    pub pieces: HashMap<u32, Piece<'a>>,
    /// Reference to the torrent metadata.
    torrent: &'a Torrent<'a>,
    /// Size of each block within a piece (typically 16 KiB).
    block_size: u32,
}

impl<'a> PieceManager<'a> {
    /// Creates a new PieceManager for the torrent.
    ///
    /// Initializes all pieces based on the torrent's piece hashes and total length.
    /// Each piece is created with the appropriate hash and divided into blocks.
    ///
    /// # Arguments
    ///
    /// * `torrent` - The torrent metadata containing piece information
    /// * `block_size` - The size of each block in bytes (typically 16384)
    ///
    /// # Returns
    ///
    /// Returns a new PieceManager instance with all pieces initialized.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::peer::piece::piece_manager::PieceManager;
    /// # use MotteSeed::core::torrent::torrent::Torrent;
    /// # let torrent: Torrent = todo!();
    /// let manager = PieceManager::new(&torrent, 16384);
    /// ```
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

    /// Selects the next piece to download from available pieces.
    ///
    /// Implements a simple piece selection strategy: returns the first piece
    /// that is either Missing or Incomplete from the list of available pieces.
    ///
    /// # Arguments
    ///
    /// * `available_pieces` - A slice of piece indices that peers have available
    ///
    /// # Returns
    ///
    /// Returns `Some(piece_index)` for the next piece to download, or `None`
    /// if no suitable piece is found (all available pieces are already complete).
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::peer::piece::piece_manager::PieceManager;
    /// # let manager: PieceManager = todo!();
    /// let available = vec![0, 1, 2, 3];
    /// if let Some(piece_index) = manager.get_next_piece_to_download(&available) {
    ///     println!("Download piece {}", piece_index);
    /// }
    /// ```
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

    /// Returns a list of completed piece indices.
    ///
    /// Collects all pieces that have been fully downloaded and verified.
    ///
    /// # Returns
    ///
    /// Returns a vector of piece indices that are in Complete or Written state.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::peer::piece::piece_manager::PieceManager;
    /// # let manager: PieceManager = todo!();
    /// let completed = manager.get_completed_pieces();
    /// println!("Completed {} pieces", completed.len());
    /// ```
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
