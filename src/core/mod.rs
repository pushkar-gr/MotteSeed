//! Core modules for the BitTorrent client.
//!
//! This module contains the main components of the client, including peer management, torrent
//! handling, tracker communication, and disk I/O.

pub mod disk_io;
pub mod peer;
pub mod torrent;
pub mod torrent_stats;
pub mod tracker;
