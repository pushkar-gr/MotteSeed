use crate::core::peer::peer_error::PeerError;

use core::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::array::TryFromSliceError;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

#[derive(Debug)]
pub struct Peer {
    stream: TcpStream,
}

impl Peer {
    pub async fn new(
        peer_ip: [u8; 6],
        client_peer_id: &[u8; 20],
        info_hash: &[u8; 20],
    ) -> Result<Self, PeerError> {
        //create addr from bytes
        let addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(
                peer_ip[0], peer_ip[1], peer_ip[2], peer_ip[3],
            )),
            u16::from_be_bytes(
                peer_ip[4..5]
                    .try_into()
                    .map_err(|e: TryFromSliceError| PeerError::Other(e.into()))?,
            ),
        );

        //create TCP stream
        let mut stream = timeout(Duration::from_secs(10), TcpStream::connect(addr)).await??;

        Ok(Self { stream })
    }

}
