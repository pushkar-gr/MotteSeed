mod core;
mod util;

use core::peer::peer_id::get_peer_id;
use core::torrent::torrent::TorrentFile;
use core::torrent_stats::TorrentStats;
use core::tracker::tracker::{Tracker, TrackerConstructor};
use core::tracker::tracker_udp::TrackerUDP;

use std::env;
use std::path::Path;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let file_path = args[1].clone();
    let torrent_file = TorrentFile::from_file(&Path::new(&file_path)).unwrap();
    let peer_id = &get_peer_id();
    let torrent_status = TorrentStats::new(1000);
    let mut tracker = TrackerUDP::new(
        torrent_file.torrent.announce,
        &torrent_file.torrent.info_hash,
        peer_id,
        torrent_status.downloaded,
        torrent_status.left,
        torrent_status.uploaded,
        6881,
    )
    .await
    .unwrap();
    println!("{:?}", tracker.get_peers().await.unwrap());
}
