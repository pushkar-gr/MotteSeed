//! Manager for multiple trackers.
//!
//! Aggregates peer information from multiple trackers, supporting both
//! primary and backup tracker lists.

use crate::core::tracker::tracker_factory::TrackerFactory;
use crate::core::{torrent_stats::TorrentStats, tracker::tracker::Tracker};

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages multiple tracker connections for a torrent.
///
/// This manager initializes trackers from the torrent's announce URL and
/// announce-list, polls them for peers, and maintains a deduplicated pool
/// of available peers.
pub struct TrackerManager<'a> {
    /// List of active tracker instances.
    trackers: Vec<Box<dyn Tracker + 'a>>,
    /// Shared torrent statistics for all trackers.
    stats: Arc<RwLock<TorrentStats>>,
    /// Deduplicated set of peers (6 bytes: 4 for IP, 2 for port).
    peer_pool: HashSet<[u8; 6]>,
    /// Info hash of the torrent.
    info_hash: &'a [u8; 20],
    /// Client's peer ID.
    peer_id: &'a [u8; 20],
    /// Port the client is listening on.
    port: u16,
}

impl<'a> TrackerManager<'a> {
    /// Creates a new tracker manager.
    ///
    /// Initializes trackers from the primary announce URL and optional backup
    /// announce list. Silently ignores trackers that fail to initialize.
    ///
    /// # Arguments
    ///
    /// * `announce_url` - Primary tracker URL
    /// * `announce_url_list` - Optional list of backup tracker URLs organized in tiers
    /// * `info_hash` - The 20-byte torrent info hash
    /// * `peer_id` - The 20-byte client peer ID
    /// * `total_size` - Total size of the torrent in bytes
    /// * `port` - Port number the client is listening on
    ///
    /// # Returns
    ///
    /// Returns a new TrackerManager instance with initialized trackers.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::tracker::tracker_manager::TrackerManager;
    /// # async fn example() {
    /// let manager = TrackerManager::new(
    ///     b"http://tracker.example.com/announce",
    ///     None,
    ///     &[0u8; 20], // info_hash
    ///     &[0u8; 20], // peer_id
    ///     1024 * 1024, // 1 MB
    ///     6881,
    /// ).await;
    /// # }
    /// ```
    pub async fn new(
        announce_url: &'a [u8],
        announce_url_list: Option<&Vec<Vec<&'a [u8]>>>,
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        total_size: u64,
        port: u16,
    ) -> Self {
        let stats = Arc::new(RwLock::new(TorrentStats::new(total_size)));
        let mut trackers = Vec::new();
        //create tracker from announce_url
        if let Ok(tracker) = TrackerFactory::create_tracker(
            announce_url,
            info_hash,
            peer_id,
            Arc::clone(&stats),
            port,
        )
        .await
        {
            trackers.push(tracker);
        }
        //create trackers from announce_urls_list
        if let Some(announce_url_list) = announce_url_list {
            for announce_urls in announce_url_list {
                for announce_url in announce_urls {
                    if let Ok(tracker) = TrackerFactory::create_tracker(
                        announce_url,
                        info_hash,
                        peer_id,
                        Arc::clone(&stats),
                        port,
                    )
                    .await
                    {
                        trackers.push(tracker);
                    }
                }
            }
        }
        Self {
            trackers,
            stats,
            peer_pool: HashSet::new(),
            info_hash,
            peer_id,
            port,
        }
    }

    /// Polls all trackers for peers.
    ///
    /// Sends announce requests to all initialized trackers and collects
    /// peers into the peer pool. Silently ignores trackers that fail to respond.
    ///
    /// The peer pool automatically deduplicates peers across different trackers.
    pub async fn poll_all_trackers(&mut self) {
        for tracker in &mut self.trackers {
            if let Ok(peers) = tracker.get_peers().await {
                for peer in peers {
                    self.peer_pool.insert(*peer);
                }
            }
        }
    }

    /// Gets all discovered peers.
    ///
    /// # Returns
    ///
    /// Returns a vector of 6-byte peer addresses (4 bytes IP + 2 bytes port).
    /// Each peer appears only once even if discovered by multiple trackers.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::tracker::tracker_manager::TrackerManager;
    /// # async fn example(mut manager: TrackerManager<'_>) {
    /// manager.poll_all_trackers().await;
    /// let peers = manager.get_all_peers_vec().await;
    /// println!("Found {} unique peers", peers.len());
    /// # }
    /// ```
    pub async fn get_all_peers_vec(&self) -> Vec<[u8; 6]> {
        self.peer_pool.iter().copied().collect()
    }

    /// Gets all discovered peers.
    ///
    /// # Returns
    ///
    /// Returns a reference HashSet of 6-byte peer addresses (4 bytes IP + 2 bytes port).
    /// Each peer appears only once even if discovered by multiple trackers.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::tracker::tracker_manager::TrackerManager;
    /// # async fn example(mut manager: TrackerManager<'_>) {
    /// manager.poll_all_trackers().await;
    /// let peers = manager.get_all_peers_hash().await;
    /// println!("Found {} unique peers", peers.len());
    /// # }
    /// ```
    pub async fn get_all_peers_hash(&self) -> &HashSet<[u8; 6]> {
        &self.peer_pool
    }
}
