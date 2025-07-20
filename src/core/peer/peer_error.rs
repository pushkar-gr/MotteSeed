use thiserror::Error;
use tokio::time::error;

#[derive(Error, Debug)]
pub enum PeerError {
    #[error("Timeout Error: {0}")]
    Elapsed(#[from] error::Elapsed),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid Handshake")]
    InvalidHandShake,

    #[error("Invalid InfoHash")]
    InvalidInfoHash,

    #[error("Error: {0}")]
    Other(#[from] Box<dyn std::error::Error>),
}
