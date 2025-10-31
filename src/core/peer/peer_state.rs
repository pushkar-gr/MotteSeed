//! Peer state management.
//!
//! Tracks the connection state with a peer, including choking/interest status,
//! available pieces (bitfield), pending requests, and transfer statistics.

use bytes::Bytes;
use std::time::Instant;

/// Represents the state of a peer connection.
///
/// Maintains all information about the current state of communication with a peer,
/// including choking status, piece availability, pending requests, and transfer rates.
#[derive(Debug)]
pub struct PeerState {
    /// Whether we are choking this peer (not sending data to them).
    pub am_choking: bool,
    /// Whether we are interested in this peer's pieces.
    pub am_interested: bool,
    /// Whether the peer is choking us (not sending data to us).
    pub peer_choking: bool,
    /// Whether the peer is interested in our pieces.
    pub peer_interested: bool,

    /// Bitfield indicating which pieces the peer has.
    /// Each bit represents one piece (1 = has piece, 0 = doesn't have piece).
    pub bitfield: Option<Bytes>,

    /// List of block requests we've sent to this peer.
    /// Each tuple contains (piece_index, begin_offset, length).
    pub requested_blocks: Vec<(u32, u32, u32)>,

    /// Total bytes downloaded from this peer.
    pub downloaded: u64,
    /// Total bytes uploaded to this peer.
    pub uploaded: u64,
    /// Current download rate in bytes per second.
    pub download_rate: f64,
    /// Current upload rate in bytes per second.
    pub upload_rate: f64,
    /// Last time the download rate was calculated.
    pub last_download_calc: Instant,
    /// Last time the upload rate was calculated.
    pub last_upload_calc: Instant,
    /// Previously recorded downloaded bytes (for rate calculation).
    pub prev_downloaded: u64,
    /// Previously recorded uploaded bytes (for rate calculation).
    pub prev_uploaded: u64,
}

impl PeerState {
    /// Creates a new peer state with default values.
    ///
    /// Initializes the peer as choking us, with us not interested,
    /// and zero transfer statistics.
    ///
    /// # Arguments
    ///
    /// * `bitfield` - Optional bitfield indicating which pieces the peer has
    ///
    /// # Returns
    ///
    /// Returns a new PeerState instance.
    ///
    /// # Example
    ///
    /// ```
    /// use MotteSeed::core::peer::peer_state::PeerState;
    ///
    /// let state = PeerState::new(None);
    /// assert!(state.peer_choking);
    /// assert!(!state.am_interested);
    /// ```
    pub fn new(bitfield: Option<Bytes>) -> Self {
        Self {
            am_choking: true,
            am_interested: false,
            peer_choking: true,
            peer_interested: false,
            bitfield,
            requested_blocks: Vec::new(),
            downloaded: 0,
            uploaded: 0,
            download_rate: 0.0,
            upload_rate: 0.0,
            last_download_calc: Instant::now(),
            last_upload_calc: Instant::now(),
            prev_downloaded: 0,
            prev_uploaded: 0,
        }
    }

    /// Updates the download rate based on elapsed time.
    ///
    /// Recalculates the download rate if at least 1 second has elapsed since
    /// the last calculation. The rate is computed as bytes per second.
    pub fn update_download_rate(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_download_calc).as_secs_f64();

        //update only if time has passed
        if elapsed >= 1.0 {
            let bytes_diff = self.downloaded - self.prev_downloaded;
            self.download_rate = bytes_diff as f64 / elapsed;

            self.prev_downloaded = self.downloaded;
            self.last_download_calc = now;
        }
    }

    /// Updates the upload rate based on elapsed time.
    ///
    /// Recalculates the upload rate if at least 1 second has elapsed since
    /// the last calculation. The rate is computed as bytes per second.
    pub fn update_upload_rate(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_upload_calc).as_secs_f64();

        //update only if time has passed
        if elapsed >= 1.0 {
            let bytes_diff = self.uploaded - self.prev_uploaded;
            self.upload_rate = bytes_diff as f64 / elapsed;

            self.prev_uploaded = self.uploaded;
            self.last_upload_calc = now;
        }
    }

    /// Checks if the peer has a specific piece.
    ///
    /// Examines the peer's bitfield to determine if they have the piece at the given index.
    ///
    /// # Arguments
    ///
    /// * `index` - The zero-based piece index to check
    ///
    /// # Returns
    ///
    /// Returns `true` if the peer has the piece, `false` if they don't or if
    /// we haven't received a bitfield from them yet.
    ///
    /// # Example
    ///
    /// ```
    /// use MotteSeed::core::peer::peer_state::PeerState;
    /// use bytes::Bytes;
    ///
    /// let bitfield = Bytes::from(vec![0b10000000]); // Has piece 0 only
    /// let state = PeerState::new(Some(bitfield));
    /// assert!(state.has_piece(0));
    /// assert!(!state.has_piece(1));
    /// ```
    pub fn has_piece(&self, index: u32) -> bool {
        if let Some(bitfield) = &self.bitfield {
            let byte_index = (index / 8) as usize;
            let bit_index = 7 - (index % 8) as usize;

            byte_index < bitfield.len() && (bitfield[byte_index] & (1 << bit_index)) != 0
        } else {
            false
        }
    }
}
