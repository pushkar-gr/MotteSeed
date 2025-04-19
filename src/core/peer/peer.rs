use crate::util::bencode::bencode_decodable::{BencodeDecodable, BencodeDecodableError};
use bencode::Bencode;
use std::collections::HashMap;
use std::time::Instant;

/// Represents a peer in BitTorrent
#[derive(Debug)]
pub struct Peer {
    pub ip: Vec<u8>,
    pub port: u16,
    pub peer_id: Option<Vec<u8>>,
    pub am_choking: bool,       // We are choking
    pub am_interested: bool,    // We are interested
    pub peer_choking: bool,     // Peer is choking
    pub peer_interested: bool,  // Peer is interested
    pub bitfield: Vec<u8>,
    pub downloading: bool,
    pub uploading: bool,
    pub available_pieces: Vec<usize>,
    pub outstanding_requests: HashMap<u32, u32>,
    pub last_active: Instant,
}

/// Decodes Peer from Bencode
impl<'a> BencodeDecodable<'a> for Peer {
    /// Decode Bencode to Peer
    fn decode(b: &'a Bencode) -> Result<Self, BencodeDecodableError> {
        // Ensure the input is a dictionary
        let dict = match b {
            Bencode::Dict(d) => d,
            _ => return Err(BencodeDecodableError::WrongType("Expected a dictionary".into())),
        };

        // Extract `ip`
        let ip = dict
            .get(&b"ip".to_vec())
            .ok_or_else(|| BencodeDecodableError::KeyNotFound("ip".into()))
            .and_then(|v| match v {
                Bencode::String(ip) => Ok(ip.clone()),
                _ => Err(BencodeDecodableError::WrongType("Expected a string for ip".into())),
            })?;

        // Extract `port`
        let port = dict
            .get(&b"port".to_vec())
            .ok_or_else(|| BencodeDecodableError::KeyNotFound("port".into()))
            .and_then(|v| get_u64(v))
            .map(|p| p as u16)?;

        // Extract `peer_id`
        let peer_id = dict.get(&b"peer_id".to_vec()).and_then(|v| match v {
            Bencode::String(peer_id) => Some(peer_id.clone()),
            _ => None,
        });

        // Create and return the Peer instance
        Ok(Peer {
            ip,
            port,
            peer_id,
            am_choking: true,
            am_interested: false,
            peer_choking: true,
            peer_interested: false,
            bitfield: Vec::new(),
            downloading: false,
            uploading: false,
            available_pieces: Vec::new(),
            outstanding_requests: HashMap::new(),
            last_active: Instant::now(),
        })
    }
}

/// Helper function to extract u64 from Bencode
fn get_u64(b: &Bencode) -> Result<u64, BencodeDecodableError> {
    match b {
        Bencode::Integer(i) => Ok(*i as u64),
        _ => Err(BencodeDecodableError::WrongType("Expected an integer".into())),
    }
}