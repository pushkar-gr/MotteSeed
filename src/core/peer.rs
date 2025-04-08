use std::collections::HashMap;
use std::net::IpAddr;

pub struct Peer {
    pub ip: IpAddr,
    pub port: u16,
    pub peer_id: [u8; 20],
    pub am_choking: bool,      //is we choking
    pub am_interested: bool,   // is we want
    pub peer_choking: bool,    // is they choking
    pub peer_interested: bool, // is they want
    pub bitfield: Vec<u8>,
    pub downloading: bool,
    pub uploading: bool,
    pub available_pieces: Vec<usize>,
    pub outstanding_requests: HashMap<u32, u32>,
}

impl Peer {
    pub fn new(ip: IpAddr, port: u16, peer_id: [u8; 20]) -> Peer {
        Peer {
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
        }
    }

    pub fn set_bitfield(&mut self, bitfield: Vec<u8>) {
        //Todo
        self.bitfield = bitfield;
    }

    pub fn has_piece(&self, piece_index: usize) -> bool {
        //Todo
        if self.bitfield.is_empty() {
            return false;
        }
        let byte_index = piece_index / 8;
        let bit_index = 7 - (piece_index % 8);
        if byte_index >= self.bitfield.len() {
            return false;
        }
        (self.bitfield[byte_index] >> bit_index) & 1 == 1
    }

    pub fn set_am_choking(&mut self, choking: bool) {
        //Todo
        self.am_choking = choking;
    }

    pub fn get_am_choking(&self) -> bool {
        //Todo
        self.am_choking
    }

    pub fn set_am_interested(&mut self, interested: bool) {
        //Todo
        self.am_interested = interested;
    }

    pub fn get_am_interested(&self) -> bool {
        //Todo
        self.am_interested
    }

    pub fn set_peer_choking(&mut self, choking: bool) {
        //Todo
        self.peer_choking = choking;
    }

    pub fn get_peer_choking(&self) -> bool {
        //Todo
        self.peer_choking
    }

    pub fn set_peer_interested(&mut self, interested: bool) {
        //Todo
        self.peer_interested = interested;
    }

    pub fn get_peer_interested(&self) -> bool {
        //Todo
        self.peer_interested
    }

    pub fn update_available_pieces(&mut self, available_pieces: Vec<usize>) {
        //Todo
        self.available_pieces = available_pieces;
    }

    pub fn get_available_pieces(&self) -> &Vec<usize> {
        //Todo
        &self.available_pieces
    }

    pub fn add_outstanding_request(&mut self, begin: u32, length: u32) {
        //Todo
        self.outstanding_requests.insert(begin, length);
    }

    pub fn remove_outstanding_request(&mut self, begin: u32) {
        //Todo
        self.outstanding_requests.remove(&begin);
    }

    pub fn get_outstanding_requests(&self) -> &HashMap<u32, u32> {
        //Todo
        &self.outstanding_requests
    }

    pub fn set_downloading(&mut self, downloading: bool) {
        //Todo
        self.downloading = downloading;
    }

    pub fn get_downloading(&self) -> bool {
        //Todo
        self.downloading
    }

    pub fn set_uploading(&mut self, uploading: bool) {
        //Todo
        self.uploading = uploading;
    }

    pub fn get_uploading(&self) -> bool {
        //Todo
        self.uploading
    }
}
