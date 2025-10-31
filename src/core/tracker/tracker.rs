//! Common tracker traits.
//!
//! Defines the core traits for tracker implementations, supporting both HTTP and UDP trackers.

use crate::core::torrent_stats::TorrentStats;
use crate::core::tracker::tracker_error::TrackerError;

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Trait for tracker implementations.
///
/// This trait defines the interface that all tracker types (HTTP, UDP) must implement
/// to fetch peer information for a torrent.
#[async_trait]
pub trait Tracker: Send + Sync {
    /// Gets the list of peers from the tracker.
    ///
    /// # Returns
    ///
    /// Returns a reference to a vector of 6-byte peer addresses (4 bytes IP + 2 bytes port).
    ///
    /// # Errors
    ///
    /// Returns `TrackerError` if the tracker request fails.
    async fn get_peers(&mut self) -> Result<&Vec<[u8; 6]>, TrackerError>;
}

/// Trait for constructing trackers.
///
/// This trait provides a factory method for creating new tracker instances
/// and initializing them with the necessary parameters.
pub trait TrackerConstructor<'a>: Sized {
    /// Creates a new tracker instance and sends an initial announce request.
    ///
    /// # Arguments
    ///
    /// * `announce_url` - The tracker's announce URL
    /// * `info_hash` - The 20-byte SHA-1 hash identifying the torrent
    /// * `peer_id` - The 20-byte client peer ID
    /// * `stats` - Shared torrent statistics (uploaded, downloaded, left)
    /// * `port` - The port number the client is listening on
    ///
    /// # Returns
    ///
    /// Returns the initialized tracker instance.
    ///
    /// # Errors
    ///
    /// Returns `TrackerError` if tracker initialization or the initial request fails.
    async fn new(
        announce_url: &'a [u8],
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        stats: Arc<RwLock<TorrentStats>>,
        port: u16,
    ) -> Result<Self, TrackerError>;
}
