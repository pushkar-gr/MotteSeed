//! Tracker factory for creating HTTP or UDP trackers.
//!
//! Provides factory methods to create the appropriate tracker implementation
//! based on the announce URL scheme.

use crate::core::{
    torrent_stats::TorrentStats,
    tracker::{
        tracker::{Tracker, TrackerConstructor},
        tracker_error::TrackerError,
        tracker_http::TrackerHTTP,
        tracker_udp::TrackerUDP,
    },
};

use std::sync::Arc;
use tokio::sync::RwLock;

/// Tracker protocol types.
///
/// Identifies the protocol used by a tracker based on its URL scheme.
#[warn(clippy::upper_case_acronyms)]
pub enum TrackerType {
    /// HTTP or HTTPS tracker protocol.
    HTTP,
    /// UDP tracker protocol.
    UDP,
}

/// Factory for creating tracker instances.
///
/// This factory analyzes tracker URLs and creates the appropriate
/// tracker implementation (HTTP or UDP).
pub struct TrackerFactory;

impl TrackerFactory {
    /// Determines the tracker type from the announce URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The tracker's announce URL as bytes
    ///
    /// # Returns
    ///
    /// Returns `TrackerType::UDP` if the URL starts with "udp://",
    /// otherwise returns `TrackerType::HTTP` (the default).
    ///
    /// # Example
    ///
    /// ```
    /// use MotteSeed::core::tracker::tracker_factory::TrackerFactory;
    ///
    /// let http_type = TrackerFactory::determine_type(b"http://tracker.example.com/announce");
    /// let udp_type = TrackerFactory::determine_type(b"udp://tracker.example.com:8000");
    /// ```
    pub fn determine_type(url: &[u8]) -> TrackerType {
        if url.starts_with(b"udp://") {
            TrackerType::UDP
        } else {
            //default
            TrackerType::HTTP
        }
    }

    /// Creates a tracker instance based on the announce URL.
    ///
    /// Automatically detects the tracker type and creates the appropriate
    /// implementation (HTTP or UDP).
    ///
    /// # Arguments
    ///
    /// * `announce_url` - The tracker's announce URL
    /// * `info_hash` - The 20-byte SHA-1 hash of the torrent
    /// * `peer_id` - The 20-byte client peer ID
    /// * `stats` - Shared torrent statistics
    /// * `port` - The port the client is listening on
    ///
    /// # Returns
    ///
    /// Returns a boxed trait object implementing the `Tracker` trait.
    ///
    /// # Errors
    ///
    /// Returns `TrackerError` if tracker creation or initialization fails.
    pub async fn create_tracker<'a>(
        announce_url: &'a [u8],
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        stats: Arc<RwLock<TorrentStats>>,
        port: u16,
    ) -> Result<Box<dyn Tracker + 'a>, TrackerError> {
        match Self::determine_type(announce_url) {
            TrackerType::HTTP => Ok(Box::new(
                TrackerHTTP::new(announce_url, info_hash, peer_id, stats, port).await?,
            )),
            TrackerType::UDP => Ok(Box::new(
                TrackerUDP::new(announce_url, info_hash, peer_id, stats, port).await?,
            )),
        }
    }
}
