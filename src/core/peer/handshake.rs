//! Handshake module for establishing peer connections.
//!
//! Implements the BitTorrent protocol handshake to authenticate and connect to peers.

use std::array::TryFromSliceError;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Performs the BitTorrent handshake with a peer.
///
/// Sends a handshake message containing the protocol identifier, info hash, and peer ID,
/// then receives and validates the peer's handshake response.
///
/// The handshake message format (68 bytes total):
/// - 1 byte: protocol string length (19)
/// - 19 bytes: protocol string ("BitTorrent protocol")
/// - 8 bytes: reserved (all zeros)
/// - 20 bytes: info hash
/// - 20 bytes: peer ID
///
/// # Arguments
///
/// * `stream` - The TCP connection to the peer
/// * `peer_id` - Our 20-byte peer ID
/// * `info_hash` - The 20-byte info hash of the torrent
///
/// # Returns
///
/// Returns the peer's 20-byte peer ID on successful handshake.
///
/// # Errors
///
/// Returns `HandShakeError` if:
/// - I/O error occurs during communication
/// - Peer's handshake has invalid protocol identifier
/// - Peer's info hash doesn't match ours
///
/// # Example
///
/// ```no_run
/// # use MotteSeed::core::peer::handshake::handshake;
/// # use tokio::net::TcpStream;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut stream = TcpStream::connect("127.0.0.1:6881").await?;
/// let peer_id = &[0u8; 20];
/// let info_hash = &[0u8; 20];
/// let remote_peer_id = handshake(&mut stream, peer_id, info_hash).await?;
/// println!("Connected to peer: {:?}", remote_peer_id);
/// # Ok(())
/// # }
/// ```
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
    buf[48..68]
        .try_into()
        .map_err(|e: TryFromSliceError| HandShakeError::Other(e.into()))
}

/// Errors that can occur during the BitTorrent handshake process.
#[derive(Error, Debug)]
pub enum HandShakeError {
    /// I/O error during network communication.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The peer's handshake message has an invalid protocol identifier.
    #[error("Invalid Handshake")]
    InvalidHandShake,

    /// The peer's info hash doesn't match the expected info hash.
    #[error("Invalid InfoHash")]
    InvalidInfoHash,

    /// Other errors that may occur during handshake.
    #[error("Error: {0}")]
    Other(#[from] Box<dyn std::error::Error>),
}
