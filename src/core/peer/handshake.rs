//! Handshake module for establishing peer connections.
//!
//! Implements the BitTorrent protocol handshake to authenticate and connect to peers.

use std::array::TryFromSliceError;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Performs handshake with peer and return peer's ID if successful.
///
/// Builds a handshake message with the protocol string, reserves bytes, info hash, and peer ID,
/// sends it, receives the response, and verifies it.
///
/// # Errors
///
/// Returns `HandShakeError` if the handshake fails due to I/O issues, invalid protocol, or
/// mismatched info hash.
pub async fn handshake(
    stream: &mut TcpStream,
    peer_id: &[u8; 20],
    info_hash: &[u8; 20],
) -> Result<[u8; 20], HandShakeError> {
    //build handshake message
    let mut buf = [0_u8; 68];

    //pstrlen
    buf[0] = 19;
    //pstr
    buf[1..20].copy_from_slice(b"BitTorrent protocol");
    //reserved bytes
    buf[20..28].fill(0);
    //info_hash after reserved 8 bytes
    buf[28..48].copy_from_slice(info_hash);
    //peer_id
    buf[48..68].copy_from_slice(peer_id);

    //send message to peer
    stream.write_all(&buf).await?;

    //recive response
    stream.read_exact(&mut buf).await?;

    //verify response
    if buf[0] != 19 || &buf[1..20] != b"BitTorrent protocol" {
        return Err(HandShakeError::InvalidHandShake);
    }
    if &buf[28..48] != info_hash {
        return Err(HandShakeError::InvalidInfoHash);
    }

    //return peer id
    Ok(buf[48..68]
        .try_into()
        .map_err(|e: TryFromSliceError| HandShakeError::Other(e.into()))?)
}

/// Errors that can occur during the handshake process.
#[derive(Error, Debug)]
pub enum HandShakeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid Handshake")]
    InvalidHandShake,

    #[error("Invalid InfoHash")]
    InvalidInfoHash,

    #[error("Error: {0}")]
    Other(#[from] Box<dyn std::error::Error>),
}
