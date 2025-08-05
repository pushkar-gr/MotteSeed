use crate::core::peer::{
    handshake::{HandShakeError, handshake},
    message::{Message, MessageError},
    peer_state::PeerState,
};

use bytes::{Bytes, BytesMut};
use std::net::SocketAddr;
use thiserror::Error;
use tokio::{io::AsyncReadExt, sync::mpsc::error::SendError};
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
    //create a new peer connection
    pub async fn new(
        ip: [u8; 6],
        bitfield: Option<Bytes>,
        to_manager: mpsc::Sender<PeerEvent>,
        from_manager: mpsc::Receiver<ManagerCommand>,
        peer_id: &[u8; 20],
        info_hash: &[u8; 20],
    ) -> Result<Self, ConnectionError> {
        //convert peer IP and port to socket address
        let peer_addr = SocketAddr::new(
            std::net::IpAddr::V4(std::net::Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3])),
            u16::from_be_bytes([ip[4], ip[5]]),
        );

        //connect to peer
        let mut stream = TcpStream::connect(peer_addr).await?;

        //perform handshake with peer
        handshake(&mut stream, peer_id, info_hash).await?;

        let mut connection = Self {
            peer_addr,
            stream,
            state: PeerState::new(bitfield),
            to_manager,
            from_manager,
            buf: BytesMut::with_capacity(16384), //16KB buffer
        };

        Ok(connection)
    }

    //read message from peer
    async fn read_message(&mut self) -> Result<Message, ConnectionError> {
        //read length prefix (4 bytes)
        let mut length_buf = [0u8; 4];
        self.stream.read_exact(&mut length_buf).await?;
        let length = u32::from_be_bytes(length_buf) as usize;

        //resize buffer if required
        if self.buf.capacity() < length {
            self.buf.reserve(length);
        }

        //clear buffer and ensure it can hold the message
        self.buf.clear();
        unsafe {
            self.buf.set_len(length);
        }

        //read message
        self.stream.read_exact(&mut self.buf).await?;

        Ok(Message::deserialize(&mut self.buf)?)
    }

    //handle emssage from peer
    async fn handle_message(&mut self, message: Message) -> Result<(), ConnectionError> {
        match message {
            Message::KeepAlive => {
                //do nothing for keep alive
            }
            Message::Choke => {
                self.state.peer_choking = true;
                self.to_manager.send(PeerEvent::PeerChoked).await?;
            }
            Message::UnChoke => {
                self.state.peer_choking = false;
                self.to_manager.send(PeerEvent::PeerUnchoked).await?;
            }
            Message::Interested => {
                self.state.peer_interested = true;
                self.to_manager.send(PeerEvent::PeerInterested).await?;
            }
            Message::NotInterested => {
                self.state.peer_interested = false;
                self.to_manager.send(PeerEvent::PeerNotInterested).await?;
            }
            Message::Have(index) => {
                self.to_manager.send(PeerEvent::ReceivedHave(index)).await?;
            }
            Message::Bitfield(bytes) => {
                self.to_manager
                    .send(PeerEvent::ReceivedBitfield(bytes))
                    .await?;
            }
            Message::Request {
                index,
                begin,
                length,
            } => {
                self.to_manager
                    .send(PeerEvent::RequestedBlock {
                        index,
                        begin,
                        length,
                    })
                    .await?;
            }
            Message::Piece {
                index,
                begin,
                block,
            } => {
                let len = block.len() as u64;
                self.to_manager
                    .send(PeerEvent::ReceivedPiece {
                        index,
                        begin,
                        block,
                    })
                    .await?;
                self.state.downloaded += len;
                self.state.update_download_rate();
                self.to_manager
                    .send(PeerEvent::DownloadRate(self.state.download_rate))
                    .await?;
            }
            Message::Cancel {
                index,
                begin,
                length,
            } => {
                //todo
            }
            Message::Port(port) => {
                //todo
            }
        };
        Ok(())
    }
}

//events sent from peer to manager
pub enum PeerEvent {
    PeerChoked,              //peer choked client
    PeerUnchoked,            //peer unchoked client
    PeerInterested,          //peer intrested in client
    PeerNotInterested,       //peer not intrested in client
    ReceivedHave(u32),       //received have messages from peer
    ReceivedBitfield(Bytes), //received bitfield from peer
    RequestedBlock {
        index: u32,
        begin: u32,
        length: u32,
    }, //peer requested a block
    ReceivedPiece {
        index: u32,
        begin: u32,
        block: Bytes,
    }, //received piece from peer
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

    #[error("Handshake failed: {0}")]
    HandshakeFailed(#[from] HandShakeError),

    #[error("Message error: {0}")]
    Message(#[from] MessageError),

    #[error("Error sending message to manager: {0}")]
    SendError(#[from] SendError<PeerEvent>),
}
