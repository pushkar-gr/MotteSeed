use std::array::TryFromSliceError;

#[derive(Debug, Clone, Hash, PartialEq, std::cmp::Eq)]
pub struct Peer {
    peer_ip: [u8; 4], //ip address of peer
    peer_port: u16,   //connection port for peer
}

impl Peer {
    pub fn from_bytes(bytes: [u8; 6]) -> Result<Self, TryFromSliceError> {
        let peer_ip = bytes[0..4].try_into()?;
        let peer_port = u16::from_be_bytes([bytes[4], bytes[5]]);

        Ok(Self { peer_ip, peer_port })
    }
}

impl Peer {
    pub fn decode(bytes: &[u8; 6]) -> Result<Self, TryFromSliceError> {
        Ok(Self {
            peer_ip: bytes[0..4].try_into()?,
            peer_port: u16::from_be_bytes(bytes[4..6].try_into()?),
        })
    }
}
