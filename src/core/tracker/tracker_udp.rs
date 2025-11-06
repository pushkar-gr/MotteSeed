//! UDP tracker implementation.
//!
//! Implements the BitTorrent UDP tracker protocol for announcing to trackers
//! and retrieving peer lists. Uses a connection-oriented protocol over UDP.

use super::{
    tracker::{Tracker, TrackerConstructor},
    tracker_error::TrackerError,
};
use crate::core::torrent_stats::TorrentStats;

use async_trait::async_trait;
use rand;
use std::{array::TryFromSliceError, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    net::UdpSocket,
    sync::RwLock,
    time::{Instant, timeout},
};

/// Connection timeout for UDP requests.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);
/// Maximum number of retries for failed requests.
const MAX_RETRIES: usize = 3;

/// UDP tracker client implementation.
///
/// Manages communication with UDP BitTorrent trackers using the
/// UDP tracker protocol, which includes connection ID management.
#[derive(Debug)]
pub struct TrackerUDP<'a> {
    /// UDP socket for communication.
    socket: UdpSocket,
    /// Socket address of the tracker server.
    server_addr: SocketAddr,
    /// Current connection ID (expires after 2 minutes).
    connection_id: u64,
    /// Timestamp when the connection ID was obtained.
    connection_time: Instant,
    /// The announce request to send to the tracker.
    announce_request: AnnounceRequestUDP<'a>,
    /// Timestamp of the last announce request.
    last_announce: Instant,
    /// Most recent announce response from the tracker.
    announce_response: AnnounceResponseUDP,
}

impl<'a> TrackerUDP<'a> {
    /// Connection ID expiration time (2 minutes per BEP 15).
    const CONNECTION_EXPIRY: Duration = Duration::from_secs(120);

    /// Gets the current connection ID, refreshing if expired.
    ///
    /// # Returns
    ///
    /// Returns the current valid connection ID.
    ///
    /// # Errors
    ///
    /// Returns `TrackerError` if refreshing the connection ID fails.
    async fn get_connection_id(&mut self) -> Result<u64, TrackerError> {
        //update connection id if expired
        if self.connection_time.elapsed() >= Self::CONNECTION_EXPIRY {
            self.refresh_connection().await?;
        }
        Ok(self.connection_id)
    }

    /// Refreshes the connection ID from the tracker.
    ///
    /// Requests a new connection ID from the tracker and updates the connection time.
    ///
    /// # Errors
    ///
    /// Returns `TrackerError` if the connection request fails.
    async fn refresh_connection(&mut self) -> Result<(), TrackerError> {
        self.connection_id = get_connection_id(&self.socket, &self.server_addr).await?;
        Ok(())
    }

    /// Sends an announce request to the tracker and processes the response.
    ///
    /// # Returns
    ///
    /// Returns the announce response containing peers and interval.
    ///
    /// # Errors
    ///
    /// Returns `TrackerError` if the announce request fails.
    async fn announce(&mut self) -> Result<AnnounceResponseUDP, TrackerError> {
        let connection_id = self.get_connection_id().await?;
        let announce_response = announce(
            &self.server_addr,
            &self.socket,
            connection_id,
            &self.announce_request,
        )
        .await?;
        self.last_announce = Instant::now();
        Ok(announce_response)
    }
}

#[async_trait]
impl<'a> Tracker for TrackerUDP<'a> {
    /// Gets peers from tracker, making a new request if needed.
    async fn get_peers(&mut self) -> Result<&Vec<[u8; 6]>, TrackerError> {
        //request again if interval has passed
        if self.last_announce.elapsed().as_secs() > self.announce_response.interval.into() {
            self.announce_response = self.announce().await?;
        }
        Ok(&self.announce_response.peers)
    }
}

impl<'a> TrackerConstructor<'a> for TrackerUDP<'a> {
    //creates new TrackerUDP
    async fn new(
        announce_url: &'a [u8],
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        stats: Arc<RwLock<TorrentStats>>,
        port: u16,
    ) -> Result<Self, TrackerError> {
        //parse announce url
        let address_str = std::str::from_utf8(announce_url)?;
        let server_addr = parse_udp_url(address_str).await?;

        //bind to local port
        let socket = UdpSocket::bind("0.0.0.0:0").await?;

        let connection_id = get_connection_id(&socket, &server_addr).await?;

        let announce_request = AnnounceRequestUDP::new(info_hash, peer_id, stats, port);

        let announce_response =
            announce(&server_addr, &socket, connection_id, &announce_request).await?;

        Ok(Self {
            socket,
            server_addr,
            connection_id,
            connection_time: Instant::now(),
            announce_request,
            last_announce: Instant::now(),
            announce_response,
        })
    }
}

/// Represents a UDP tracker announce request.
///
/// Contains all parameters for the UDP tracker announce protocol.
#[derive(Debug)]
struct AnnounceRequestUDP<'a> {
    /// SHA-1 hash of the torrent info dictionary.
    info_hash: &'a [u8; 20],
    /// Client's 20-byte peer ID.
    peer_id: &'a [u8; 20],
    /// Shared torrent statistics.
    stats: Arc<RwLock<TorrentStats>>,
    /// Event type: 0=none, 1=completed, 2=started, 3=stopped.
    event: u32,
    /// IP address (0=default, use sender's IP).
    ip_address: u32,
    /// Random key for identification.
    key: u32,
    /// Number of peers wanted (-1=default).
    num_want: u32,
    /// Port number the client is listening on.
    port: u16,
}

impl<'a> AnnounceRequestUDP<'a> {
    /// Creates a new UDP tracker announce request.
    ///
    /// # Arguments
    ///
    /// * `info_hash` - The 20-byte torrent info hash
    /// * `peer_id` - The 20-byte client peer ID
    /// * `stats` - Shared torrent statistics
    /// * `port` - Port number for incoming connections
    ///
    /// # Returns
    ///
    /// Returns a new AnnounceRequestUDP instance with default values.
    fn new(
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        stats: Arc<RwLock<TorrentStats>>,
        port: u16,
    ) -> Self {
        Self {
            info_hash,
            peer_id,
            stats,
            event: 0,
            ip_address: 0,
            key: rand::random(),
            num_want: 50,
            port,
        }
    }

    /// Serializes request to bytes.
    async fn to_bytes(&self, connection_id: u64, transaction_id: u32) -> [u8; 98] {
        let mut buf = [0u8; 98];
        buf[0..8].copy_from_slice(&connection_id.to_be_bytes());
        buf[8..12].copy_from_slice(&1_u32.to_be_bytes());
        buf[12..16].copy_from_slice(&transaction_id.to_be_bytes());
        buf[16..36].copy_from_slice(self.info_hash);
        buf[36..56].copy_from_slice(self.peer_id);
        let (uploaded, downloaded, left) = {
            let stats = self.stats.read().await;
            (stats.uploaded, stats.downloaded, stats.left)
        };
        buf[56..64].copy_from_slice(&downloaded.to_be_bytes());
        buf[64..72].copy_from_slice(&left.to_be_bytes());
        buf[72..80].copy_from_slice(&uploaded.to_be_bytes());
        buf[80..84].copy_from_slice(&self.event.to_be_bytes());
        buf[84..88].copy_from_slice(&self.ip_address.to_be_bytes());
        buf[88..92].copy_from_slice(&self.key.to_be_bytes());
        buf[92..96].copy_from_slice(&self.num_want.to_be_bytes());
        buf[96..98].copy_from_slice(&self.port.to_be_bytes());
        buf
    }
}

/// Represents a UDP tracker announce response.
///
/// Contains the response data from a UDP tracker including peer list and interval.
#[derive(Debug)]
struct AnnounceResponseUDP {
    /// Action field (should be 1 for announce response).
    action: u32,
    /// Transaction ID (matches the request).
    transaction_id: u32,
    /// Seconds to wait before the next announce.
    interval: u32,
    /// Number of leechers (peers downloading).
    leechers: u32,
    /// Number of seeders (peers with complete file).
    seeders: u32,
    /// List of peers in compact format (6 bytes each).
    peers: Vec<[u8; 6]>,
}

impl AnnounceResponseUDP {
    /// Deserializes a UDP announce response from bytes.
    ///
    /// Parses the binary UDP tracker response format:
    /// - 4 bytes: action
    /// - 4 bytes: transaction_id
    /// - 4 bytes: interval
    /// - 4 bytes: leechers
    /// - 4 bytes: seeders
    /// - N*6 bytes: peer list
    ///
    /// # Arguments
    ///
    /// * `data` - The raw response bytes from the tracker
    ///
    /// # Returns
    ///
    /// Returns the parsed AnnounceResponseUDP.
    ///
    /// # Errors
    ///
    /// Returns `TrackerError` if:
    /// - Response is too short (< 20 bytes)
    /// - Peer data length is not a multiple of 6
    fn from_bytes(data: &[u8]) -> Result<Self, TrackerError> {
        if data.len() < 20 {
            return Err(TrackerError::Other("Response too short".into()));
        }

        //get data
        let action = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let transaction_id = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let interval = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        let leechers = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
        let seeders = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);

        //get peers
        let peer_data = &data[20..];

        if !peer_data.len().is_multiple_of(6) {
            return Err(TrackerError::Other("Invalid peer data length".into()));
        }

        let peer_count = peer_data.len() / 6;
        let mut peers = Vec::with_capacity(peer_count);

        for chunk in peer_data.chunks_exact(6) {
            let peer_bytes: [u8; 6] = chunk
                .try_into()
                .map_err(|e: TryFromSliceError| TrackerError::Other(e.into()))?;
            peers.push(peer_bytes);
        }

        Ok(Self {
            action,
            transaction_id,
            interval,
            leechers,
            seeders,
            peers,
        })
    }
}

/// Represents connection response from a tracker.
struct ConnectionResponse {
    action: u32,
    transaction_id: u32,
    connection_id: u64,
}

impl ConnectionResponse {
    /// Converts bytes to ConnectionResponse.
    fn from_bytes(data: [u8; 16]) -> Self {
        let action = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let transaction_id = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let connection_id = u64::from_be_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);
        Self {
            action,
            transaction_id,
            connection_id,
        }
    }
}

/// Gets connection ID for a tracker.
async fn get_connection_id(
    socket: &UdpSocket,
    server_addr: &SocketAddr,
) -> Result<u64, TrackerError> {
    for _ in 0..MAX_RETRIES {
        //generate random transaction id
        let transaction_id: u32 = rand::random();
        //get connection request message
        let request_message = create_connect_request(transaction_id);
        //send message
        socket.send_to(&request_message, server_addr).await?;

        //wait for response
        match timeout(CONNECTION_TIMEOUT, recv_conn_id_response(socket)).await {
            Ok(Ok(response)) => {
                //verify transaction id
                if response.action == 0 && response.transaction_id == transaction_id {
                    return Ok(response.connection_id);
                }
                //wrong response, try again
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                //timeout, try again
                continue;
            }
        }
    }

    Err(TrackerError::Other("Max Retries Exceeded".into()))
}

/// Parses str to get SocketAddr.
async fn parse_udp_url(url: &str) -> Result<SocketAddr, TrackerError> {
    //get url host
    let host_port = url
        .strip_prefix("udp://")
        .ok_or_else(|| TrackerError::Other("Invalid UDP URL format".into()))?;
    let host_port = match host_port.find('/') {
        Some(index) => &host_port[..index],
        None => host_port,
    };

    //get ip addr of tracker
    let addrs = tokio::net::lookup_host(host_port)
        .await
        .map_err(|e| TrackerError::Other(format!("DNS resolve error: {}", e).into()))?;

    //create SocketAddr
    let ipv4_addrs: Vec<SocketAddr> = addrs.filter(|addr| addr.is_ipv4()).collect();

    //return if IPv4 found
    if !ipv4_addrs.is_empty() {
        Ok(ipv4_addrs[0])
    } else {
        Err(TrackerError::Other(
            "No IPv4 address found for tracker".into(),
        ))
    }
}
/// Recives tracker response.
async fn recv_conn_id_response(socket: &UdpSocket) -> Result<ConnectionResponse, TrackerError> {
    let mut buf = [0_u8; 1024];
    let (bytes_received, _) = socket.recv_from(&mut buf).await?;
    if bytes_received != 16 {
        Err(TrackerError::Other(
            format!(
                "Invalid tracker response length: expected 16, got {}",
                bytes_received
            )
            .into(),
        ))
    } else {
        Ok(ConnectionResponse::from_bytes(
            buf[..16]
                .try_into()
                .map_err(|_| TrackerError::Other("Failed to parse response".into()))?,
        ))
    }
}

/// Creates a connection request message.
#[inline]
fn create_connect_request(transaction_id: u32) -> [u8; 16] {
    let mut buf = [0_u8; 16];
    //magic connection id
    buf[0..8].copy_from_slice(&0x41727101980_u64.to_be_bytes());
    //action: 0 for connect
    buf[8..12].copy_from_slice(&0_u32.to_be_bytes());
    //transaction id
    buf[12..16].copy_from_slice(&transaction_id.to_be_bytes());
    buf
}

/// Sends a request to the tracker and processes the response.
async fn announce<'a>(
    server_addr: &SocketAddr,
    socket: &UdpSocket,
    connection_id: u64,
    announce_request: &AnnounceRequestUDP<'a>,
) -> Result<AnnounceResponseUDP, TrackerError> {
    for _ in 0..MAX_RETRIES {
        //generate random transaction id
        let transaction_id: u32 = rand::random();
        //get announce request message
        let request_message = announce_request
            .to_bytes(connection_id, transaction_id)
            .await;
        //send message
        socket.send_to(&request_message, server_addr).await?;

        //wait for response
        match timeout(CONNECTION_TIMEOUT, recv_announce_response(socket)).await {
            Ok(Ok(response)) => {
                //verify transaction id
                if response.action == 1 && response.transaction_id == transaction_id {
                    return Ok(response);
                }
                //wrong response, try again
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                //timeout, try again
                continue;
            }
        }
    }

    Err(TrackerError::Other("Max Retries Exceeded".into()))
}

/// Recives tracker response.
async fn recv_announce_response(socket: &UdpSocket) -> Result<AnnounceResponseUDP, TrackerError> {
    let mut buf = [0_u8; 1024];
    let (bytes_received, _) = socket.recv_from(&mut buf).await?;
    if bytes_received < 20 {
        Err(TrackerError::Other(
            format!(
                "Invalid tracker response length: expected at least 20, got {}",
                bytes_received
            )
            .into(),
        ))
    } else {
        AnnounceResponseUDP::from_bytes(&buf[..bytes_received])
    }
}
