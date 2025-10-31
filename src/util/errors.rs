//! Wrapper for bencode streaming errors.

use bencode::streaming::Error as BencStreamingError;

/// Wrapper struct for bencode streaming errors.
///
/// This struct wraps errors from the bencode streaming parser, providing
/// conversion and display implementations to integrate with the standard
/// Rust error handling system.
#[derive(Debug)]
pub struct BStreamingError(BencStreamingError);

impl std::fmt::Display for BStreamingError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl std::error::Error for BStreamingError {}

impl From<BencStreamingError> for BStreamingError {
    fn from(err: BencStreamingError) -> Self {
        BStreamingError(err)
    }
}
