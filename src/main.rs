mod core;
mod util;

use core::peer_id::get_peer_id;
use core::torrent::torrent::TorrentFile;

use core::tracker::tracker::{Tracker, TrackerRequest};
use std::env;
use std::path::Path;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let file_path = args[1].clone();
    let torrent_file = TorrentFile::from_file(&Path::new(&file_path)).unwrap();
    let peer_id = &get_peer_id();
    let tracker_request = TrackerRequest::new(
        torrent_file.torrent.announce,
        &torrent_file.torrent.info_hash,
        peer_id,
        6881,
        0,
        0,
        0,
        true,
    )
    .unwrap();
    let tracker = Tracker::new(&tracker_request).await.unwrap();
    println!("{:?}", tracker);
}
