//! Tracker factory for creating HTTP or UDP trackers.

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

/// Tracker tyes.
#[warn(clippy::upper_case_acronyms)]
pub enum TrackerType {
    HTTP,
    UDP,
}

/// Factory for creating trackers.
pub struct TrackerFactory;

impl TrackerFactory {
    /// Determines tracker type from URL.
    pub fn determine_type(url: &[u8]) -> TrackerType {
        if url.starts_with(b"udp://") {
            TrackerType::UDP
        } else {
            //default
            TrackerType::HTTP
        }
    }

    /// Creates a tracker based on URL.
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
