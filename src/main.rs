//! Main entry point for the MotteSeed BitTorrent client.
//!
//! This module initializes the client, parses command-line arguments, loads the torrent file, and
//! starts communication with trackers and peers.

mod core;
mod util;

use core::disk_io::DiskIO;
use core::peer::peer_id::get_peer_id;
use core::torrent::torrent::FileDetails;
use core::torrent::torrent::TorrentFile;
use core::tracker::tracker_manager::{self, TrackerManager};

use std::env;
use std::path::Path;
use std::path::PathBuf;

use tokio::sync::mpsc;

use crate::core::peer::download_manager;
use crate::core::peer::download_manager::DownloadManager;
use crate::core::peer::peer_connection::PeerEvent;
use crate::core::peer::peer_manager::PeerManager;

/// Main function that runs the BitTorrent client.
///
/// Parses the torrent file path from command-line arguments, loads the torrent, generates a peer
/// ID, initializes the tracker manager, polls trackers for peers, and prints the list of peers.
///
/// # Panics
///
/// Panics if the torrent file cannot be loaded of if tracker operations fail.
#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <torrent_file>", args[0]);
        eprintln!("Example: {} ubuntu.torrent", args[0]);
        std::process::exit(1);
    }

    let file_path = &args[1];
    println!("Loading torrent file: {}", file_path);

    let torrent_file = match TorrentFile::from_file(Path::new(&file_path)) {
        Ok(tf) => tf,
        Err(e) => {
            eprintln!("Failed to load torrent file: {}", e);
            std::process::exit(1);
        }
    };

    let torrent = &torrent_file.torrent;
    let info = &torrent.info;

    println!("Torrent info:");
    println!("Name: {}", info.name);
    println!("Piece length: {} bytes", info.piece_length);

    let num_pieces = info.raw_pieces.len() / 20;
    println!("Number of pieces: {}", num_pieces);

    let total_size = match &info.file_details {
        FileDetails::SingleFile { length } => {
            println!("Type: Single file");
            println!(
                "Size: {} bytes ({:2} MB)",
                length,
                *length as f64 / 1_048_576.0
            );
            *length
        }
        FileDetails::MultiFile { files } => {
            println!("Type: Multi-file");
            let total: u64 = files.iter().map(|f| f.length).sum();
            println!("Files: {}", files.len());
            println!(
                "Total size: {} bytes ({:2} MB)",
                total,
                total as f64 / 1_048_576.0
            );
            total
        }
    };

    let peer_id = *get_peer_id();
    println!("Client info");
    println!("Peer ID: {:?}", String::from_utf8_lossy(&peer_id[..]));

    println!("Connecting to tracker");
    let mut tracker_manager = TrackerManager::new(
        torrent.announce,
        None,
        &torrent.info_hash,
        &peer_id,
        total_size,
        6881,
    )
    .await;

    println!("Pooling trackers for peers...");
    tracker_manager.poll_all_trackers().await;
    let peers = tracker_manager.get_all_peers_vec().await;
    println!("Found {} peers", peers.len());

    if peers.is_empty() {
        println!("No peers found. Cannot download torrent.");
        return;
    }

    println!("Peer list:");
    for (i, peer) in peers.iter().take(5).enumerate() {
        println!(
            " {}. {}.{}.{}.{}:{}",
            i + 1,
            peer[0],
            peer[1],
            peer[2],
            peer[3],
            u16::from_be_bytes([peer[4], peer[5]])
        );
    }
    println!("...");

    let download_path = PathBuf::from("./downloads");
    println!("Setting up download");
    println!("Download directory: {}", download_path.display());

    let disk_io = match DiskIO::new(download_path, &info.file_details, info.piece_length as u32) {
        Ok(dio) => dio,
        Err(e) => {
            eprintln!("Failed to create disk I/O handler: {}", e);
            std::process::exit(1);
        }
    };

    println!("Disk I/O handler created successfully");

    let (event_tx, event_rx) = mpsc::channel::<(PeerEvent, [u8; 6])>(1000);
    let block_size = 16384;
    let mut download_manager = DownloadManager::new(&torrent, block_size, event_rx);

    download_manager.set_disk_io(&disk_io);

    let mut peer_manager = PeerManager::new(peer_id, torrent.info_hash, event_tx);

    println!("Starting downloading");
    let max_peers = std::cmp::min(peers.len(), 10);
    println!("Connecting to {} peers...", max_peers);

    for peer_ip in peers.iter().take(max_peers) {
        peer_manager.add_peer(*peer_ip, None);
    }

    println!("Download started!");

    download_manager.run().await;

    let completed = download_manager.piece_manager.get_completed_pieces().len();
    println!("Download complte");
    println!("Downloaded {} pieces", completed);

    if download_manager.piece_manager.is_download_complete() {
        println!("All pieces downloaded successfully");
    } else {
        println!("Download incomplete: {} pieces", completed);
    }
}
