use bytes::Bytes;
use std::time::Instant;

//structure to represent a block
#[derive(Debug)]
pub struct Block<'a> {
    pub index: u32,            //index of the piece block belongs to
    pub offset: u32,           //offset of block in the piece
    pub length: u32,           //lenght of data
    pub data: Option<Bytes>,   //data store in bytes
    pub state: BlockState<'a>, //block state
}

impl<'a> Block<'a> {
    pub fn new(index: u32, offset: u32, length: u32) -> Self {
        Self {
            index,
            offset,
            length,
            data: None,
            state: BlockState::Missing,
        }
    }

    pub fn isComplete(&self) -> bool {
        matches!(self.state, BlockState::Received | BlockState::Written)
    }

    pub fn request_from_peer(&mut self, peer_ip: &'a [u8; 6]) {
        self.state = BlockState::Requested {
            peer_ip,
            instant: Instant::now(),
        }
    }
}

//represents block state
#[derive(Debug)]
pub enum BlockState<'a> {
    Missing, //block needs to be downloaded
    Requested {
        peer_ip: &'a [u8; 6],
        instant: Instant,
    }, //block requested from a peer
    Received, //block received from a peer
    Written, //block written to disk
}
