//! Errors for tracker operations.

use crate::util::bencode::bencode_decodable_error::BencodeDecodableError;
use crate::util::errors::BStreamingError;

use http::uri::{InvalidUri, InvalidUriParts};
use std::str::Utf8Error;
use thiserror::Error;

/// Custom error enum for tracker operations.
#[derive(Error, Debug)]
pub enum TrackerError {
    #[error("Invalid Uri: {0}")]
    InvalidUri(#[from] InvalidUri),

    #[error("Stream Error: {0}")]
    StreamError(#[from] std::io::Error),

    #[error("Hyper Error: {0}")]
    HyperError(#[from] hyper::Error),

    #[error("Hyper http Error: {0}")]
    HttpError(#[from] hyper::http::Error),

    #[error("UTF8 Error: {0}")]
    UTF8Error(#[from] Utf8Error),

    #[error("Invalid URI Parts: {0}")]
    InvalidURIParts(#[from] InvalidUriParts),

    #[error("Bencode Error: {0}")]
    BencodeError(#[from] BencodeDecodableError),

    #[error("Streaming error: {0}")]
    StreamingError(#[from] BStreamingError),

    #[error("Error: {0}")]
    Other(#[from] Box<dyn std::error::Error>),
}
