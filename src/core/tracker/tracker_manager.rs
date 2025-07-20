use crate::core::tracker::tracker_error::TrackerError;
use crate::core::tracker::tracker_factory::TrackerFactory;
use crate::core::{torrent_stats::TorrentStats, tracker::tracker::Tracker};

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

//manages trackers
pub struct TrackerManager<'a> {
    trackers: Vec<Box<dyn Tracker + 'a>>, //vector of trackers
    stats: Arc<RwLock<TorrentStats>>,     //tracker stats
    peer_pool: HashSet<[u8; 6]>,          //hashlist of peers
    info_hash: &'a [u8; 20],              //info hash of torrent
    peer_id: &'a [u8; 20],                //peer id of client
    port: u16,                            //connection port
}

impl<'a> TrackerManager<'a> {
    //create a new tracker manager
    pub async fn new(
        announce_url: &'a [u8],
        announce_url_list: Option<&Vec<Vec<&'a [u8]>>>,
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        total_size: u64,
        port: u16,
    ) -> Result<Self, TrackerError> {
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
        Ok(Self {
            trackers: trackers,
            stats: stats,
            peer_pool: HashSet::new(),
            info_hash,
            peer_id,
            port,
        })
    }

    //get peers from all trackers
    pub async fn poll_all_trackers(&mut self) -> Result<(), TrackerError> {
        for tracker in &mut self.trackers {
            if let Ok(peers) = tracker.get_peers().await {
                for peer in peers {
                    self.peer_pool.insert(*peer);
                }
            }
        }
        Ok(())
    }

    //get all peers
    pub async fn get_all_peers(&self) -> Vec<[u8; 6]> {
        self.peer_pool.iter().copied().collect()
    }
}
