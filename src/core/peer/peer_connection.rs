//! Peer connection management.
//!
//! Handles establishing and maintaining TCP connections with peers, processing incoming
//! messages, sending outgoing messages, and managing the connection lifecycle.

use crate::core::peer::{
    handshake::{HandShakeError, handshake},
    message::{Message, MessageError},
    peer_state::PeerState,
};

use bytes::{Bytes, BytesMut};
use std::net::SocketAddr;
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    select,
    sync::mpsc::error::SendError,
};
use tokio::{net::TcpStream, sync::mpsc};

/// Represents an active connection to a peer.
///
/// Manages the TCP stream, peer state, message processing, and communication
/// with the peer manager through channels.
#[derive(Debug)]
pub struct PeerConnection {
    /// Socket address of the peer.
    peer_addr: SocketAddr,
    /// TCP stream for communication with the peer.
    stream: TcpStream,
    /// Current state of the peer (choking, interested, pieces, etc.).
    state: PeerState,
    /// Channel to send events to the peer manager.
    to_manager: mpsc::Sender<PeerEvent>,
    /// Channel to receive commands from the peer manager.
    from_manager: mpsc::Receiver<ManagerCommand>,
    /// Buffer for reading incoming messages.
    buf: BytesMut,
}

impl PeerConnection {
    /// Creates a new peer connection.
    ///
    /// Establishes a TCP connection to the peer, performs the BitTorrent handshake,
    /// and initializes the connection state.
    ///
    /// # Arguments
    ///
    /// * `ip` - 6-byte peer address (4 bytes IP + 2 bytes port in big-endian)
    /// * `to_manager` - Channel sender to communicate events to the manager
    /// * `from_manager` - Channel receiver for commands from the manager
    /// * `peer_id` - Our 20-byte peer ID
    /// * `info_hash` - The 20-byte torrent info hash
    ///
    /// # Returns
    ///
    /// Returns a new PeerConnection instance.
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError` if:
    /// - TCP connection fails
    /// - Handshake fails (invalid protocol or mismatched info hash)
    pub async fn new(
        ip: &[u8; 6],
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

        let connection = Self {
            peer_addr,
            stream,
            state: PeerState::new(None),
            to_manager,
            from_manager,
            buf: BytesMut::with_capacity(16384), //16KB buffer
        };

        Ok(connection)
    }

    /// Reads a message from the peer.
    ///
    /// Reads the 4-byte length prefix, then reads the message payload and deserializes it.
    ///
    /// # Arguments
    ///
    /// * `buf` - Buffer to use for reading
    /// * `stream` - TCP stream to read from
    ///
    /// # Returns
    ///
    /// Returns the deserialized Message.
    ///
    /// # Errors
    ///
    /// Returns `ConnectionError` if I/O or deserialization fails.
    async fn read_message(
        buf: &mut BytesMut,
        stream: &mut TcpStream,
    ) -> Result<Message, ConnectionError> {
        //read length prefix (4 bytes)
        let mut length_buf = [0u8; 4];
        stream.read_exact(&mut length_buf).await?;
        let length = u32::from_be_bytes(length_buf) as usize;

        //resize buffer if required
        if buf.capacity() < length {
            buf.reserve(length);
        }

        //clear buffer and ensure it can hold the message
        buf.clear();
        unsafe {
            buf.set_len(length);
        }

        //read message
        stream.read_exact(buf).await?;

        Ok(Message::deserialize(buf)?)
    }

    /// Handles a message from the peer.
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
                self.state.bitfield = Some(bytes.clone());
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

    /// Handles a manager command from the manager.
    async fn handle_manager_command(
        &mut self,
        command: ManagerCommand,
    ) -> Result<(), ConnectionError> {
        //convert ManagerCommand to Message
        let message = match command {
            ManagerCommand::KeepAlive => Message::KeepAlive,
            ManagerCommand::Choke => Message::Choke,
            ManagerCommand::Unchoke => Message::UnChoke,
            ManagerCommand::Interested => Message::Interested,
            ManagerCommand::NotInterested => Message::NotInterested,
            ManagerCommand::HavePiece(index) => Message::Have(index),
            ManagerCommand::RequestBlock {
                index,
                begin,
                length,
            } => Message::Request {
                index,
                begin,
                length,
            },
            ManagerCommand::CancelBlock {
                index,
                begin,
                length,
            } => Message::Cancel {
                index,
                begin,
                length,
            },
            ManagerCommand::Disconnect => {
                //todo
                return Ok(());
            }
        };
        //convert message to bytes
        message.serialize_into(&mut self.buf);
        //write to stream
        self.stream.write_all(&self.buf).await?;
        self.stream.flush().await?;
        Ok(())
    }

    /// Shares the bitfield with the peer.
    async fn send_bitfield(&mut self, bitfield: Option<Bytes>) -> Result<(), ConnectionError> {
        if let Some(bitfield) = bitfield {
            //create message
            let message = Message::Bitfield(bitfield);
            //serialize message
            message.serialize_into(&mut self.buf);
            //send message to peer
            self.stream.write_all(&self.buf).await?;
        }
        Ok(())
    }

    /// Runs the peer connection loop.
    pub async fn run(&mut self, bitfield: Option<Bytes>) -> Result<(), ConnectionError> {
        //share bitfield
        self.send_bitfield(bitfield).await?;

        loop {
            //borrow required parameters to avoid borrowing mutable self multipel times
            let buf = &mut self.buf;
            let stream = &mut self.stream;

            select! {
                //handle manager command
                command = self.from_manager.recv() => {
                    match command {
                        Some(cmd) => self.handle_manager_command(cmd).await?,
                        None => return Ok(()),
                    }
                }

                //handle incoming messages from peer
                result = Self::read_message(buf, stream) => {
                    let message = result?;
                    self.handle_message(message).await?;
                }
            }
        }
    }
}

/// Events sent from a peer connection to the manager.
///
/// These events represent state changes or data received from a peer.
#[derive(Debug)]
pub enum PeerEvent {
    /// Peer has choked us (stopped sending data).
    PeerChoked,
    /// Peer has unchoked us (ready to send data).
    PeerUnchoked,
    /// Peer is interested in our pieces.
    PeerInterested,
    /// Peer is not interested in our pieces.
    PeerNotInterested,
    /// Peer announced they have a specific piece.
    ReceivedHave(u32),
    /// Peer sent their bitfield of available pieces.
    ReceivedBitfield(Bytes),
    /// Peer requested a block from us.
    RequestedBlock {
        /// Piece index.
        index: u32,
        /// Byte offset within the piece.
        begin: u32,
        /// Length of the block.
        length: u32,
    },
    /// Received a piece block from the peer.
    ReceivedPiece {
        /// Piece index.
        index: u32,
        /// Byte offset within the piece.
        begin: u32,
        /// Block data.
        block: Bytes,
    },
    /// Peer disconnected from the TCP connection.
    PeerDisconnected,
    /// Current download rate from this peer in bytes per second.
    DownloadRate(f64),
}

/// Commands sent from the manager to a peer connection.
///
/// These commands instruct the peer connection to send specific messages.
#[derive(Debug)]
pub enum ManagerCommand {
    /// Send a keep-alive message.
    KeepAlive,
    /// Choke the peer (stop sending data to them).
    Choke,
    /// Unchoke the peer (ready to send data to them).
    Unchoke,
    /// Tell the peer we're interested in their pieces.
    Interested,
    /// Tell the peer we're not interested in their pieces.
    NotInterested,
    /// Announce that we have a specific piece.
    HavePiece(u32),
    /// Request a block from the peer.
    RequestBlock {
        /// Piece index.
        index: u32,
        /// Byte offset within the piece.
        begin: u32,
        /// Length of the block to request.
        length: u32,
    },
    /// Cancel a previously requested block.
    CancelBlock {
        /// Piece index.
        index: u32,
        /// Byte offset within the piece.
        begin: u32,
        /// Length of the block.
        length: u32,
    },
    /// Disconnect from the peer.
    Disconnect,
}

/// Errors that can occur during peer connection operations.
#[derive(Error, Debug)]
pub enum ConnectionError {
    /// I/O error during network communication.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Handshake with the peer failed.
    #[error("Handshake failed: {0}")]
    HandshakeFailed(#[from] HandShakeError),

    /// Error parsing or serializing a message.
    #[error("Message error: {0}")]
    Message(#[from] MessageError),

    /// Error sending an event to the manager.
    #[error("Error sending message to manager: {0}")]
    SendError(#[from] SendError<PeerEvent>),
}
