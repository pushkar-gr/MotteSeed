use bytes::Bytes;
use std::time::Instant;

//structure to represent a block
#[derive(Debug)]
pub struct Block {
    pub index: u32,          //index of the piece block belongs to
    pub offset: u32,         //offset of block in the piece
    pub length: u32,         //lenght of data
    pub data: Option<Bytes>, //data store in bytes
    pub state: BlockState,   //block state
}

impl Block {
    pub fn new(index: u32, offset: u32, length: u32) -> Self {
        Self {
            index,
            offset,
            length,
            data: None,
            state: BlockState::Missing,
        }
    }
}

//represents block state
#[derive(Debug)]
pub enum BlockState {
    Missing,                                          //block needs to be downloaded
    Requested { peer_ip: [u8; 6], instant: Instant }, //block requested from a peer
    Received,                                         //block received from a peer
    Written,                                          //block written to disk
}
