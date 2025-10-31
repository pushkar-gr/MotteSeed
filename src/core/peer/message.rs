//! Message handling for peer communication.
//!
//! Defines the Message enum for BitTorrent protocol messages, with serialization and
//! deserialization functions. Implements the wire protocol for peer-to-peer communication.

use bytes::{BufMut, Bytes, BytesMut};
use thiserror::Error;

/// Represents a message in the BitTorrent peer wire protocol.
///
/// Messages are used to communicate between peers for downloading and uploading pieces.
/// Each message type has a specific format defined by the BitTorrent protocol specification.
#[derive(Debug)]
pub enum Message {
    /// Keep-alive message (no payload), sent to maintain the connection.
    KeepAlive,
    /// Choke message, tells the peer we're choking them (not sending data).
    Choke,
    /// Unchoke message, tells the peer we're ready to send data.
    UnChoke,
    /// Interested message, tells the peer we're interested in their pieces.
    Interested,
    /// Not interested message, tells the peer we're not interested in their pieces.
    NotInterested,
    /// Have message, informs the peer we have a specific piece.
    ///
    /// Contains the piece index.
    Have(u32),
    /// Bitfield message, communicates which pieces we have.
    ///
    /// Contains a bitfield where each bit represents a piece (1 = have, 0 = don't have).
    Bitfield(Bytes),
    /// Request message, asks the peer for a block of data.
    ///
    /// Contains piece index, byte offset within the piece, and length.
    Request {
        /// The piece index to request.
        index: u32,
        /// Byte offset within the piece.
        begin: u32,
        /// Length of the block to request (typically 16 KiB).
        length: u32,
    },
    /// Piece message, sends a block of data to the peer.
    ///
    /// Contains piece index, byte offset, and the block data.
    Piece {
        /// The piece index.
        index: u32,
        /// Byte offset within the piece.
        begin: u32,
        /// The block data.
        block: Bytes,
    },
    /// Cancel message, cancels a previous request.
    ///
    /// Contains the same fields as Request to identify which request to cancel.
    Cancel {
        /// The piece index.
        index: u32,
        /// Byte offset within the piece.
        begin: u32,
        /// Length of the block.
        length: u32,
    },
    /// Port message, informs the peer of our DHT port (DHT extension).
    Port(u16),
}

impl Message {
    /// Serializes the message to bytes.
    ///
    /// # Returns
    ///
    /// Returns a Bytes object containing the serialized message in BitTorrent wire format.
    ///
    /// # Example
    ///
    /// ```
    /// use MotteSeed::core::peer::message::Message;
    ///
    /// let msg = Message::Interested;
    /// let bytes = msg.serialize();
    /// ```
    pub fn serialize(&self) -> Bytes {
        let mut buf = BytesMut::new();
        self.serialize_into(&mut buf);
        buf.freeze()
    }

    /// Serializes the message into the given buffer.
    ///
    /// Writes the message in BitTorrent wire protocol format:
    /// - 4 bytes: message length
    /// - 1 byte: message ID (except for KeepAlive)
    /// - Variable: message payload
    ///
    /// # Arguments
    ///
    /// * `buf` - The buffer to write the serialized message into
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

    /// Deserializes bytes into a Message.
    ///
    /// Parses the BitTorrent wire protocol format and constructs the appropriate Message variant.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Buffer containing the message bytes (will be consumed/modified)
    ///
    /// # Returns
    ///
    /// Returns the parsed Message on success.
    ///
    /// # Errors
    ///
    /// Returns `MessageError` if:
    /// - The message is too short to be valid
    /// - The message has an invalid or unknown type ID
    ///
    /// # Example
    ///
    /// ```no_run
    /// use MotteSeed::core::peer::message::Message;
    /// use bytes::BytesMut;
    ///
    /// let mut buffer = BytesMut::from(&[0, 0, 0, 1, 2][..]); // Interested message
    /// let msg = Message::deserialize(&mut buffer)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn deserialize(bytes: &mut BytesMut) -> Result<Self, MessageError> {
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
                Ok(Message::Bitfield(bytes.split_off(5).freeze()))
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
                let block = bytes.split_off(13).freeze();
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

/// Errors that can occur during message deserialization.
#[derive(Error, Debug)]
pub enum MessageError {
    /// The message has an invalid or unknown type ID.
    #[error("Invalid message type: {0}")]
    InvalidType(u8),

    /// The message buffer is too short to contain a valid message.
    #[error("Message too short")]
    MessageTooShort,
}
