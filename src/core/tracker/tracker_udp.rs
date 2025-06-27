use crate::core::tracker::tracker_error::TrackerError;

use rand;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::{Instant, timeout};

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_RETRIES: usize = 3;

//represents UDP tracker client
pub struct TrackerUDP {
    socket: UdpSocket,
    server_addr: SocketAddr,
    connection_id: u64,
    connection_time: Instant,
}

impl TrackerUDP {
    const CONNECTION_EXPIRY: Duration = Duration::from_secs(120);

    //creates new TrackerUDP
    pub async fn new(announce_url: &[u8]) -> Result<Self, TrackerError> {
        //parse announce url
        let address_str = std::str::from_utf8(announce_url)?;
        let server_addr = parse_udp_url(address_str)?;

        //bind to local port
        let socket = UdpSocket::bind("0.0.0.0:0").await?;

        let connection_id = get_connection_id(&socket, &server_addr).await?;

        Ok(Self {
            socket,
            server_addr,
            connection_id,
            connection_time: Instant::now(),
        })
    }

    //get connection id
    async fn get_connection_id(&mut self) -> Result<u64, TrackerError> {
        //update connection id if expired
        if self.connection_time.elapsed() >= Self::CONNECTION_EXPIRY {
            self.refresh_connection().await?;
        }
        Ok(self.connection_id)
    }

    //refresh connection id
    async fn refresh_connection(&mut self) -> Result<(), TrackerError> {
        self.connection_id = get_connection_id(&self.socket, &self.server_addr).await?;
        Ok(())
    }
}

//represents connection response from a tracker
struct ConnectionResponse {
    action: u32,
    transaction_id: u32,
    connection_id: u64,
}

impl ConnectionResponse {
    //convert bytes to ConnectionResponse
    pub fn from_bytes(data: [u8; 16]) -> Self {
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

//get connection id for a tracker
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
        match timeout(CONNECTION_TIMEOUT, recv_response(&socket)).await {
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

//parse str to get SocketAddr
#[inline]
fn parse_udp_url(url: &str) -> Result<SocketAddr, TrackerError> {
    let host_port = url
        .strip_prefix("udp://")
        .ok_or_else(|| TrackerError::Other("Invalid UDP URL format".into()))?;

    SocketAddr::from_str(host_port)
        .map_err(|e| TrackerError::Other(format!("Invalid address: {}", e).into()))
}

//recive tracker response
async fn recv_response(socket: &UdpSocket) -> Result<ConnectionResponse, TrackerError> {
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

//create a connection request message
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
