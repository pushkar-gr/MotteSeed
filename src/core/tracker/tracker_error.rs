//! Errors for tracker operations.

use crate::util::bencode::bencode_decodable_error::BencodeDecodableError;
use crate::util::errors::BStreamingError;

use http::uri::{InvalidUri, InvalidUriParts};
use std::str::Utf8Error;
use thiserror::Error;

/// Custom error enum for tracker communication operations.
///
/// This enum represents all possible errors that can occur while communicating
/// with BitTorrent trackers over HTTP or UDP.
#[derive(Error, Debug)]
pub enum TrackerError {
    /// Invalid URI format for tracker URL.
    #[error("Invalid Uri: {0}")]
    InvalidUri(#[from] InvalidUri),

    /// I/O or stream error during tracker communication.
    #[error("Stream Error: {0}")]
    StreamError(#[from] std::io::Error),

    /// HTTP client error from hyper.
    #[error("Hyper Error: {0}")]
    HyperError(#[from] hyper::Error),

    /// HTTP protocol error from hyper.
    #[error("Hyper http Error: {0}")]
    HttpError(#[from] hyper::http::Error),

    /// UTF-8 decoding error in tracker response.
    #[error("UTF8 Error: {0}")]
    UTF8Error(#[from] Utf8Error),

    /// Invalid URI parts when constructing tracker URL.
    #[error("Invalid URI Parts: {0}")]
    InvalidURIParts(#[from] InvalidUriParts),

    /// Bencode decoding error in tracker response.
    #[error("Bencode Error: {0}")]
    BencodeError(#[from] BencodeDecodableError),

    /// Bencode streaming error in tracker response.
    #[error("Streaming error: {0}")]
    StreamingError(#[from] BStreamingError),

    /// Generic error wrapper for other error types.
    #[error("Error: {0}")]
    Other(#[from] Box<dyn std::error::Error>),
}
