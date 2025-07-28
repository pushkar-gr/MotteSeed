use bytes::{BufMut, Bytes, BytesMut};
use thiserror::Error;

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
        self.serialize_into(&mut buf);
        buf.freeze()
    }

    //serialize message to given buf
    pub fn serialize_into(&self, buf: &mut BytesMut) {
        match self {
            Message::KeepAlive => buf.put_u32(0),
            Message::Choke => {
                buf.put_u32(1);
                buf.put_u8(0);
            }
            Message::UnChoke => {
                buf.put_u32(1);
                buf.put_u8(1);
            }
            Message::Interested => {
                buf.put_u32(1);
                buf.put_u8(2);
            }
            Message::NotInterested => {
                buf.put_u32(1);
                buf.put_u8(3);
            }
            Message::Have(piece) => {
                buf.put_u32(5);
                buf.put_u8(4);
                buf.put_u32(*piece);
            }
            Message::Bitfield(bytes) => {
                buf.put_u32(1 + bytes.len() as u32);
                buf.put_u8(5);
                buf.put_slice(bytes);
            }
            Message::Request {
                index,
                begin,
                length,
            } => {
                buf.put_u32(13);
                buf.put_u8(6);
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
                buf.put_u8(7);
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
                buf.put_u8(8);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_u32(*length);
            }
            Message::Port(port) => {
                buf.put_u32(3);
                buf.put_u8(9);
                buf.put_u16(*port);
            }
        };
    }

    //deserialize bytes to Message
    pub fn deserialize(bytes: Box<[u8]>) -> Result<Self, MessageError> {
        //min 4 bytes for message length
        if bytes.len() < 4 {
            return Err(MessageError::MessageTooShort);
        }

        //get message length
        let len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        if len == 0 {
            return Ok(Message::KeepAlive);
        }

        //check if message has message id
        if bytes.len() < 5 {
            return Err(MessageError::MessageTooShort);
        }

        //get message id
        let msg_id = bytes[4];

        match msg_id {
            0 => Ok(Message::Choke),
            1 => Ok(Message::UnChoke),
            2 => Ok(Message::Interested),
            3 => Ok(Message::NotInterested),
            4 => {
                if bytes.len() != 9 {
                    return Err(MessageError::MessageTooShort);
                }
                let piece_index = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
                Ok(Message::Have(piece_index))
            }
            5 => {
                //remove len 1 to get X
                let len = len - 1;
                if bytes.len() < 5 + len as usize {
                    return Err(MessageError::MessageTooShort);
                }
                let bytes_obj = Bytes::from(bytes);
                Ok(Message::Bitfield(bytes_obj.slice(5..(5 + len as usize))))
            }
            6 => {
                if bytes.len() != 17 {
                    return Err(MessageError::MessageTooShort);
                }
                let index = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
                let begin = u32::from_be_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]);
                let length = u32::from_be_bytes([bytes[13], bytes[14], bytes[15], bytes[16]]);
                Ok(Message::Request {
                    index,
                    begin,
                    length,
                })
            }
            7 => {
                //remove len 9 to get X
                let len = len - 9;
                if bytes.len() < 13 + len as usize {
                    return Err(MessageError::MessageTooShort);
                }
                let index = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
                let begin = u32::from_be_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]);
                let bytes_obj = Bytes::from(bytes);
                let block = bytes_obj.slice(13..(13 + len as usize));
                Ok(Message::Piece {
                    index,
                    begin,
                    block,
                })
            }
            8 => {
                if bytes.len() != 17 {
                    return Err(MessageError::MessageTooShort);
                }
                let index = u32::from_be_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
                let begin = u32::from_be_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]);
                let length = u32::from_be_bytes([bytes[13], bytes[14], bytes[15], bytes[16]]);
                Ok(Message::Cancel {
                    index,
                    begin,
                    length,
                })
            }
            9 => {
                if bytes.len() != 7 {
                    return Err(MessageError::MessageTooShort);
                }
                let port = u16::from_be_bytes([bytes[5], bytes[6]]);
                Ok(Message::Port(port))
            }
            i => Err(MessageError::InvalidType(i)),
        }
    }
}

#[derive(Error, Debug)]
pub enum MessageError {
    #[error("Invalid message type: {0}")]
    InvalidType(u8),

    #[error("Message too short")]
    MessageTooShort,
}
