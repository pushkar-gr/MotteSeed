//! Errors for bencode decoding.

use thiserror::Error;

/// Custom error enum for reading torrent operations.
#[derive(Error, Debug)]
pub enum BencodeDecodableError {
    //key not found error
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    //wrong tyep error
    #[error("Found wrong type: {0}")]
    WrongType(String),

    #[error("Error: {0}")]
    Other(#[from] Box<dyn std::error::Error>),
}
