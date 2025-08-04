mod core;
mod util;

use core::peer::peer_id::get_peer_id;
use core::torrent::torrent::TorrentFile;

use std::env;
use std::path::Path;

use crate::core::tracker::tracker_manager::TrackerManager;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let file_path = args[1].clone();
    let torrent_file = TorrentFile::from_file(&Path::new(&file_path)).unwrap();
    let peer_id = &get_peer_id();
    let mut tracker_manager = TrackerManager::new(
        torrent_file.torrent.announce,
        torrent_file.torrent.announce_list.as_ref(),
        &torrent_file.torrent.info_hash,
        peer_id,
        100,
        1234,
    )
    .await;
    tracker_manager.poll_all_trackers().await;
    println!("{:?}", tracker_manager.get_all_peers().await);
}
