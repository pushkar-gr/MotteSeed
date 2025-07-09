use crate::core::tracker::tracker_error::TrackerError;
use crate::core::tracker::tracker_factory::TrackerFactory;
use crate::core::{peer::peer::Peer, torrent_stats::TorrentStats, tracker::tracker::Tracker};

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

//manages trackers
pub struct TrackerManager<'a> {
    trackers: Vec<Box<dyn Tracker + 'a>>,      //vector of trackers
    stats: TorrentStats,                       //tracker stats
    peer_pool: Arc<RwLock<HashSet<&'a Peer>>>, //hashlist of peers
    info_hash: &'a [u8; 20],                   //info hash of torrent
    peer_id: &'a [u8; 20],                     //peer id of client
    port: u16,                                 //connection port
}

impl<'a> TrackerManager<'a> {
    //create a new tracker manager
    pub async fn new(
        announce_url: &'a [u8],
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        total_size: u64,
        port: u16,
    ) -> Result<Self, TrackerError> {
        let stats = TorrentStats::new(total_size);
        let tracker = TrackerFactory::create_tracker(
            announce_url,
            info_hash,
            peer_id,
            Arc::clone(&stats.downloaded),
            Arc::clone(&stats.left),
            Arc::clone(&stats.uploaded),
            port,
        )
        .await?;
        Ok(Self {
            trackers: vec![tracker],
            stats,
            peer_pool: Arc::new(RwLock::new(HashSet::new())),
            info_hash,
            peer_id,
            port,
        })
    }

    //get peers from all trackers
    pub async fn poll_all_trackers(&'a mut self) -> Result<(), TrackerError> {
        for tracker in &mut self.trackers {
            if let Ok(peers) = tracker.get_peers().await {
                let mut peer_pool = self.peer_pool.write().await;
                for peer in peers {
                    peer_pool.insert(peer);
                }
            }
        }
        Ok(())
    }

    //get all peers
    pub async fn get_all_peers(&self) -> Vec<&Peer> {
        let peer_pool = self.peer_pool.read().await;
        peer_pool.iter().copied().collect()
    }
}
