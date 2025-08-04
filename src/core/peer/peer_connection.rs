use crate::core::peer::peer_state::PeerState;

use bytes::{Bytes, BytesMut};
use std::net::SocketAddr;
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
