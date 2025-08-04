use crate::core::peer::peer_state::PeerState;

use bytes::{Bytes, BytesMut};
use std::net::SocketAddr;
use thiserror::Error;
use tokio::{net::TcpStream, sync::mpsc};

//represents a connection to a peer
pub struct PeerConnection {
    peer_addr: SocketAddr, //ip address of peer
    stream: TcpStream,     //TCP stream
    state: PeerState,      //peer state
    //communication channels
    to_manager: mpsc::Sender<PeerEvent>,
    from_manager: mpsc::Receiver<ManagerCommand>,
    buf: BytesMut, //buffer to read messages
}

impl PeerConnection {
    pub async fn new(
        ip: [u8; 6],
        info_hash: &[u8; 20],
        peer_id: &[u8; 20],
        to_manager: mpsc::Sender<PeerEvent>,
        from_manager: mpsc::Receiver<ManagerCommand>,
    ) -> Result<Self, ConnectionError> {
        //convert peer IP and port to socket address
        let peer_addr = SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3])),
            u16::from_be_bytes([ip[4], ip[5]]),
        );

        //connect to peer
        let stream = TcpStream::connect(peer_addr).await?;

        let mut connection = Self {
            peer_addr,
            stream,
            state: PeerState::new(),
            to_manager,
            from_manager,
            buf: BytesMut::with_capacity(16384), //16KB buffer
        };

        Ok(connection)
    }
}

//events sent from peer to manager
pub enum PeerEvent {
    ReceivedBitfield(Bytes), //received bitfield from peer
    ReceivedHave(u32),       //received have messages from peer
    ReceivedPiece { index: u32, begin: u32, data: Bytes }, //received piece from peer
    PeerChoked,              //peer choked client
    PeerUnchoked,            //peer unchoked client
    PeerInterested,          //peer intrested in client
    PeerNotInterested,       //peer not intrested in client
    PeerDisconnected,        //peer disconnected from TCP connection
    DownloadRate(f64),       //download rate of peer
}

//commands sent from manager to peer
pub enum ManagerCommand {
    RequestBlock { index: u32, begin: u32, length: u32 }, //request block from peer
    CancelBlock { index: u32, begin: u32, length: u32 },  //cancle requested block
    Choke,                                                //choke peer
    Unchoke,                                              //unchoke peer
    Interested,                                           //client intrested in peer
    NotInterested,                                        //client not intrested in peer
    HavePiece(u32),                                       //client have piece
    Disconnect,                                           //disconnect peer connection
}

#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
