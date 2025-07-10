use crate::core::tracker::tracker_error::TrackerError;

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

#[async_trait]
pub trait Tracker: Send + Sync {
    //get peers from tracker
    async fn get_peers(&mut self) -> Result<&Vec<[u8; 6]>, TrackerError>;
}

pub trait TrackerConstructor<'a>: Sized {
    //create a new tracker and sends an initial request
    async fn new(
        announce_url: &'a [u8],
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        downloaded: Arc<RwLock<u64>>,
        left: Arc<RwLock<u64>>,
        uploaded: Arc<RwLock<u64>>,
        port: u16,
    ) -> Result<Self, TrackerError>;
}
