use bytes::Bytes;
use std::time::{Duration, Instant};

//structure to represent a block
#[derive(Debug)]
pub struct Block<'a> {
    pub index: u32,  //index of the piece block belongs to
    pub offset: u32, //offset of block in the piece
    pub length: u32, //lenght of data
    // pub data: Option<Bytes>,   //data store in bytes
    pub state: BlockState<'a>, //block state
}

impl<'a> Block<'a> {
    const TIMEOUT: Duration = Duration::from_secs(120);
    //create new object
    pub fn new(index: u32, offset: u32, length: u32) -> Self {
        Self {
            index,
            offset,
            length,
            state: BlockState::Missing,
        }
    }

    //check if block is compelte
    pub fn is_complete(&self) -> bool {
        matches!(self.state, BlockState::Received(_) | BlockState::Written)
    }

    //request block from peer
    pub fn request_from_peer(&mut self, peer_ip: &'a [u8; 6]) {
        self.state = BlockState::Requested {
            peer_ip,
            instant: Instant::now(),
        }
    }

    //calcle request from peer
    pub fn cancle(&mut self) {
        self.state = BlockState::Missing;
    }

    //reset request from peer if timout
    pub fn reset(&mut self) -> bool {
        if let BlockState::Requested { peer_ip, instant } = self.state {
            if instant.elapsed() > Self::TIMEOUT {
                self.cancle();
                true;
            }
        }
        false
    }

    //data received
    pub fn receive_data(&mut self, bytes: Bytes) {
        if bytes.len() as u32 == self.length {
            self.state = BlockState::Received(bytes);
        }
    }

    //mark block as written
    pub fn mask_written(&mut self) {
        self.state = BlockState::Written;
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
    Received(Bytes), //block received from a peer
    Written, //block written to disk
}
