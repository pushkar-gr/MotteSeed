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

//represents block state
#[derive(Debug, Clone, PartialEq)]
pub enum PieceState {
    Missing,    //piece needs to be downloaded
    Incomplete, //some blocks downloaded but not complete
    Complete,   //all blocks downloaded and verified
    Written,    //piece written to disk
}
