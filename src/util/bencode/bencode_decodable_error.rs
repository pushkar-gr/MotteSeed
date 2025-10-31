//! Errors for bencode decoding.

use thiserror::Error;

/// Custom error enum for bencode decoding operations.
///
/// This enum represents all possible errors that can occur while decoding
/// bencode data into Rust types.
#[derive(Error, Debug)]
pub enum BencodeDecodableError {
    /// A required key was not found in the bencode dictionary.
    ///
    /// Contains the name of the missing key.
    #[error("Key not found: {0}")]
    KeyNotFound(String),

    /// The bencode value had an unexpected type.
    ///
    /// Contains a description of the type mismatch.
    #[error("Found wrong type: {0}")]
    WrongType(String),

    /// A generic error occurred during decoding.
    ///
    /// Wraps any other error type that might occur.
    #[error("Error: {0}")]
    Other(#[from] Box<dyn std::error::Error>),
}
