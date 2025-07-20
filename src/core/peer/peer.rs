#[derive(Debug)]
pub struct Peer {
    peer_ip: [u8; 4], //ip address of peer
    peer_port: u16,   //connection port for peer
}

impl Peer {
    pub async fn handshake(
        bytes: [u8; 6],
        peer_id: &[u8; 20],
        info_hash: &[u8; 20],
    ) -> Result<Self, PeerError> {
        let peer_ip = bytes[0..4].try_into()?;
        let peer_port = u16::from_be_bytes([bytes[4], bytes[5]]);

        Ok(Self { peer_ip, peer_port })
    }
}
