//! Torrent statistics management.
//!
//! Tracks download/upload progress and remaining bytes for torrent transfers.

/// Manages torrent transfer statistics.
///
/// This structure tracks the number of bytes downloaded, uploaded, and remaining
/// for a torrent. It's used to report progress to trackers and monitor transfer status.
#[derive(Debug)]
pub struct TorrentStats {
    /// Total number of bytes downloaded.
    pub downloaded: u64,
    /// Total number of bytes uploaded.
    pub uploaded: u64,
    /// Number of bytes left to download.
    pub left: u64,
}

impl TorrentStats {
    /// Creates a new TorrentStats instance.
    ///
    /// # Arguments
    ///
    /// * `total_size` - The total size of the torrent in bytes
    ///
    /// # Returns
    ///
    /// Returns a new TorrentStats with downloaded and uploaded set to 0,
    /// and left set to the total size.
    ///
    /// # Example
    ///
    /// ```
    /// use MotteSeed::core::torrent_stats::TorrentStats;
    ///
    /// let stats = TorrentStats::new(1024 * 1024); // 1 MB torrent
    /// assert_eq!(stats.downloaded, 0);
    /// assert_eq!(stats.left, 1024 * 1024);
    /// ```
    pub fn new(total_size: u64) -> Self {
        Self {
            downloaded: 0,
            uploaded: 0,
            left: total_size,
        }
    }

    /// Updates the number of downloaded bytes.
    ///
    /// Increments the downloaded counter and decrements the left counter by the same amount.
    /// Uses saturating subtraction to prevent underflow.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Number of bytes that were downloaded
    pub async fn update_downloaded(&mut self, bytes: u64) {
        self.downloaded += bytes;
        self.left = self.left.saturating_sub(bytes);
    }

    /// Updates the number of uploaded bytes.
    ///
    /// Increments the uploaded counter.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Number of bytes that were uploaded
    pub async fn update_uploaded(&mut self, bytes: u64) {
        self.uploaded += bytes;
    }

    /// Updates the number of bytes left to download.
    ///
    /// Increments the left counter, typically used when marking pieces as incomplete.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Number of bytes to add to the left counter
    pub async fn updated_left(&mut self, bytes: u64) {
        self.left += bytes;
    }
}
