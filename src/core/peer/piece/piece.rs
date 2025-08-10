use crate::core::peer::piece::block::Block;

use std::collections::HashMap;

//structure to represent a piece
#[derive(Debug)]
pub struct Piece<'a> {
    pub index: u32,                      //piece index
    pub length: u32,                     //piece length
    pub hash: &'a [u8; 20],              //SHA1 of piece
    pub blocks: HashMap<u32, Block<'a>>, //blocks
    pub state: PieceState,               //piece state
    pub block_size: u32,                 //block size
    pub priority: u8,                    //piece priority
}

impl<'a> Piece<'a> {
    //create new object
    pub fn new(index: u32, length: u32, hash: &'a [u8; 20], block_size: u32) -> Self {
        let mut blocks = HashMap::new();
        let mut offset = 0;

        //create blocks for the piece
        while offset < length {
            let block_length = std::cmp::min(block_size, length - offset);
            blocks.insert(offset, Block::new(index, offset, block_length));
            offset += block_length;
        }

        Self {
            index,
            length,
            hash,
            blocks,
            state: PieceState::Missing,
            block_size,
            priority: 1, //default priority
        }
    }

    //check if block is compelte
    pub fn isComplete(&self) -> bool {
        matches!(self.state, PieceState::Complete | PieceState::Written)
    }

    //check if all blocks are downloaded
    pub fn is_fully_downloaded(&self) -> bool {
        self.blocks.values().all(|block| block.is_complete())
    }
}

//represents block state
#[derive(Debug, Clone, PartialEq)]
pub enum PieceState {
    Missing,    //piece needs to be downloaded
    Incomplete, //some blocks downloaded but not complete
    Complete,   //all blocks downloaded and verified
    Written,    //piece written to disk
}
