use bytes::{BufMut, Bytes, BytesMut};

//message enum to represent message tyes in BitTorrent protocol
#[derive(Debug)]
pub enum Message {
    KeepAlive,
    Choke,
    UnChoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Bytes),
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        index: u32,
        begin: u32,
        block: Bytes,
    },
    Cancel {
        index: u32,
        begin: u32,
        length: u32,
    },
    Port(u16),
}

impl Message {
    //serialize message to bytes
    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::new();

        match self {
            Message::KeepAlive => buf.put_u32(0),
            Message::Choke => {
                buf.put_u32(1);
                buf.put_u32(0);
            }
            Message::UnChoke => {
                buf.put_u32(1);
                buf.put_u32(1);
            }
            Message::Interested => {
                buf.put_u32(1);
                buf.put_u32(2);
            }
            Message::NotInterested => {
                buf.put_u32(1);
                buf.put_u32(3);
            }
            Message::Have(piece) => {
                buf.put_u32(5);
                buf.put_u32(4);
                buf.put_u32(*piece);
            }
            Message::Bitfield(bytes) => {
                buf.put_u32(1 + bytes.len() as u32);
                buf.put_u32(5);
                buf.put_slice(bytes);
            }
            Message::Request {
                index,
                begin,
                length,
            } => {
                buf.put_u32(13);
                buf.put_u32(6);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_u32(*length);
            }
            Message::Piece {
                index,
                begin,
                block,
            } => {
                buf.put_u32(9 + block.len() as u32);
                buf.put_u32(7);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_slice(block);
            }
            Message::Cancel {
                index,
                begin,
                length,
            } => {
                buf.put_u32(13);
                buf.put_u32(8);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_u32(*length);
            }
            Message::Port(port) => {
                buf.put_u32(3);
                buf.put_u32(9);
                buf.put_u16(*port);
            }
        };

        buf.freeze()
    }
}
