//! Error types for torrent operations.

use crate::util::bencode::bencode_decodable_error::BencodeDecodableError;
use crate::util::errors::BStreamingError;

use thiserror::Error;

/// Custom error enum for reading torrent operations.
#[derive(Error, Debug)]
pub enum ReadTorrentError {
    //variant for streaming errors with a display message
    #[error("Streaming error: {0}")]
    Streaming(#[from] BStreamingError),

    //key not found error
    #[error("Key not found: {0}")]
    BencodeDecodable(#[from] BencodeDecodableError),

    //io error with a display message
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
}
