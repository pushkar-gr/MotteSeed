//! Torrent statistics management.
//!
//! Tracks download/upload progress and rages.

/// Manages torrent status.
#[derive(Debug)]
pub struct TorrentStats {
    pub downloaded: u64,
    pub uploaded: u64,
    pub left: u64,
}

impl TorrentStats {
    /// Creates a new object.
    pub fn new(total_size: u64) -> Self {
        Self {
            downloaded: 0,
            uploaded: 0,
            left: total_size,
        }
    }

    /// Updates downloaded bytes.
    pub async fn update_downloaded(&mut self, bytes: u64) {
        self.downloaded += bytes;
        self.left = self.left.saturating_sub(bytes);
    }

    /// Updates uploaded bytes.
    pub async fn update_uploaded(&mut self, bytes: u64) {
        self.uploaded += bytes;
    }

    /// Updates left bytes.
    pub async fn updated_left(&mut self, bytes: u64) {
        self.left += bytes;
    }
}
