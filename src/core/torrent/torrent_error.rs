//! Error types for torrent operations.

use crate::util::{
    bencode::bencode_decodable_error::BencodeDecodableError, errors::BStreamingError,
};

use thiserror::Error;

/// Custom error enum for torrent file reading and parsing operations.
///
/// This enum represents all possible errors that can occur while reading
/// and parsing torrent files.
#[derive(Error, Debug)]
pub enum ReadTorrentError {
    /// Error from the bencode streaming parser.
    ///
    /// Occurs when the bencode data is malformed or cannot be parsed.
    #[error("Streaming error: {0}")]
    Streaming(#[from] BStreamingError),

    /// Error from bencode decoding operations.
    ///
    /// Occurs when a required key is missing or a value has an unexpected type.
    #[error("Key not found: {0}")]
    BencodeDecodable(#[from] BencodeDecodableError),

    /// I/O error while reading the torrent file.
    ///
    /// Occurs when the file cannot be read from disk.
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
}
