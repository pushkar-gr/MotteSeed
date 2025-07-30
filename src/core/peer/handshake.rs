use crate::core::peer::peer_error::PeerError;

use std::array::TryFromSliceError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

//perform handshake with peer and return peer_id if succuss
async fn handshake(
    stream: &mut TcpStream,
    peer_id: &[u8; 20],
    info_hash: &[u8; 20],
) -> Result<[u8; 20], PeerError> {
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
        return Err(PeerError::InvalidHandShake);
    }
    if &buf[28..48] != info_hash {
        return Err(PeerError::InvalidInfoHash);
    }

    //return peer id
    Ok(buf[48..68]
        .try_into()
        .map_err(|e: TryFromSliceError| PeerError::Other(e.into()))?)
}
