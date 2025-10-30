//! Peer state management.
//!
//! Tracks the state of peer connection, including choking, intrest, bitfield, download/upload
//! states, and rates.

use bytes::Bytes;
use std::time::Instant;

/// Struct to represent peer state.
#[derive(Debug)]
pub struct PeerState {
    //choking state
    pub am_choking: bool,
    pub am_interested: bool,
    pub peer_choking: bool,
    pub peer_interested: bool,

    //bitfield
    pub bitfield: Option<Bytes>,

    //request tracking
    pub requested_blocks: Vec<(u32, u32, u32)>, //(index, begin, length)

    //stats
    pub downloaded: u64,             //total bytes downloaded from the peer
    pub uploaded: u64,               //total bytes uploaded to the peer
    pub download_rate: f64,          //download rate in bytes/second
    pub upload_rate: f64,            //upload rate in bytes/second
    pub last_download_calc: Instant, //last time download rate was calculated
    pub last_upload_calc: Instant,   //last time upload rate was calculated
    pub prev_downloaded: u64,        //previously downloaded bytes
    pub prev_uploaded: u64,          //previously uploaded bytes
}

impl PeerState {
    /// Creates a new peer state with default values.
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

    /// Checks if peer has a specific piece.
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
