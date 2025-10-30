//! Common tracker traits.

use crate::core::torrent_stats::TorrentStats;
use crate::core::tracker::tracker_error::TrackerError;

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Trait for tracker implementations.
#[async_trait]
pub trait Tracker: Send + Sync {
    /// Gets peers from the tracker.
    async fn get_peers(&mut self) -> Result<&Vec<[u8; 6]>, TrackerError>;
}

/// Trait for constructing trackers.
pub trait TrackerConstructor<'a>: Sized {
    /// Creates a new tracker and sends an initial request.
    async fn new(
        announce_url: &'a [u8],
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        stats: Arc<RwLock<TorrentStats>>,
        port: u16,
    ) -> Result<Self, TrackerError>;
}
