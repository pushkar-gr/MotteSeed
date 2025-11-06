//! HTTP tracker implementation.
//!
//! Implements the BitTorrent HTTP tracker protocol for announcing to trackers
//! and retrieving peer lists. Supports both compact and non-compact peer formats.

use super::{
    tracker::{Tracker, TrackerConstructor},
    tracker_error::TrackerError,
};
use crate::core::torrent_stats::TorrentStats;
use crate::util::{
    bencode::{
        bencode_decodable::BencodeDecodable, bencode_decodable_error::BencodeDecodableError,
    },
    errors::BStreamingError,
};

use async_trait::async_trait;
use bencode::{Bencode, from_buffer};
use http::{Request, Uri, uri::PathAndQuery};
use http_body_util::{BodyExt, Empty};
use hyper::{body::Bytes, client::conn::http1::handshake};
use hyper_util::rt::TokioIo;
use itoa;
use std::{array::TryFromSliceError, sync::Arc, time::Instant};
use tokio::{net::TcpStream, sync::RwLock};

/// HTTP tracker client implementation.
///
/// Manages communication with HTTP/HTTPS BitTorrent trackers, handling
/// announce requests and peer list responses.
#[derive(Debug)]
pub struct TrackerHTTP<'a> {
    /// The announce request to send to the tracker.
    request: AnnounceRequestHTTP<'a>,
    /// Timestamp of the last announce request.
    last_announce: Instant,
    /// Most recent response from the tracker.
    response: AnnounceResponseHTTP,
}

impl<'a> TrackerHTTP<'a> {
    /// Sends an announce request to the tracker and processes the response.
    ///
    /// # Returns
    ///
    /// Returns the bencode-encoded tracker response.
    ///
    /// # Errors
    ///
    /// Returns `TrackerError` if the HTTP request fails or the response is invalid.
    async fn announce(&mut self) -> Result<Bencode, TrackerError> {
        let response = announce(&self.request).await?;
        self.last_announce = Instant::now();
        Ok(response)
    }
}

#[async_trait]
impl<'a> Tracker for TrackerHTTP<'a> {
    /// Gets peers from tracker, making a new request if needed.
    async fn get_peers(&mut self) -> Result<&Vec<[u8; 6]>, TrackerError> {
        //request again if interval has passed
        if self.last_announce.elapsed().as_secs() > self.response.interval {
            let response_bencode = self.announce().await?;
            self.response = AnnounceResponseHTTP::decode(&response_bencode)?;
        }
        Ok(&self.response.peers)
    }
}

impl<'a> TrackerConstructor<'a> for TrackerHTTP<'a> {
    /// Creates a new tracker and sends an initial request.
    async fn new(
        tracker: &'a [u8],
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        stats: Arc<RwLock<TorrentStats>>,
        port: u16,
    ) -> Result<Self, TrackerError> {
        let request = AnnounceRequestHTTP::new(tracker, info_hash, peer_id, port, stats, true)?;
        let response_bencode = announce(&request).await?;

        Ok(Self {
            request,
            last_announce: Instant::now(),
            response: AnnounceResponseHTTP::decode(&response_bencode)?,
        })
    }
}

/// Represents an HTTP announce request to a BitTorrent tracker.
///
/// Contains all parameters needed to construct a valid tracker announce URL.
#[derive(Debug)]
struct AnnounceRequestHTTP<'a> {
    /// Tracker URL as a string.
    tracker: &'a str,
    /// URL-encoded info hash (20 bytes encoded as percent-escaped string).
    url_info_hash: String,
    /// URL-encoded peer ID (20 bytes encoded as percent-escaped string).
    url_peer_id: String,
    /// Port number the client is listening on for incoming connections.
    port: u16,
    /// Shared torrent statistics (downloaded, uploaded, left).
    stats: Arc<RwLock<TorrentStats>>,
    /// Whether to request compact peer list format (binary).
    compact: bool,
}

impl<'a> AnnounceRequestHTTP<'a> {
    /// Creates a new HTTP tracker announce request.
    ///
    /// # Arguments
    ///
    /// * `tracker` - The tracker announce URL
    /// * `info_hash` - The 20-byte torrent info hash
    /// * `peer_id` - The 20-byte client peer ID
    /// * `port` - Port number for incoming connections
    /// * `stats` - Shared torrent statistics
    /// * `compact` - Whether to request compact peer format
    ///
    /// # Returns
    ///
    /// Returns a new AnnounceRequestHTTP instance.
    ///
    /// # Errors
    ///
    /// Returns `TrackerError` if the tracker URL contains invalid UTF-8.
    fn new(
        tracker: &'a [u8],
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        port: u16,
        stats: Arc<RwLock<TorrentStats>>,
        compact: bool,
    ) -> Result<Self, TrackerError> {
        //convert uri from bytes to str
        let tracker = std::str::from_utf8(tracker)?;

        Ok(Self {
            tracker,
            url_info_hash: Self::url_encode(info_hash),
            url_peer_id: Self::url_encode(peer_id),
            port,
            stats,
            compact,
        })
    }

    /// URL encodes a 20-byte value for use in tracker requests.
    ///
    /// Encodes bytes using percent-encoding, preserving unreserved characters
    /// (alphanumeric, hyphen, underscore, period, tilde) as-is.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The 20-byte value to encode (info hash or peer ID)
    ///
    /// # Returns
    ///
    /// Returns the URL-encoded string.
    fn url_encode(bytes: &[u8; 20]) -> String {
        //count bytes that need encoding
        let encoded_count = bytes
            .iter()
            .filter(|&&b| !(b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~')))
            .count();

        //allocate exact capacity needed
        let capacity = bytes.len() + (encoded_count * 2);
        let mut result = String::with_capacity(capacity);

        for &b in bytes {
            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
                //direct character push - no allocation
                result.push(b as char);
            } else {
                //add percent encoding without format!
                result.push('%');
                //convert byte to hex digits
                let digit1 = char::from_digit((b >> 4).into(), 16)
                    .unwrap_or('0')
                    .to_ascii_uppercase();
                let digit2 = char::from_digit((b & 0xF).into(), 16)
                    .unwrap_or('0')
                    .to_ascii_uppercase();
                result.push(digit1);
                result.push(digit2);
            }
        }

        result
    }

    /// Builds the complete tracker request URL with all required parameters.
    ///
    /// Constructs a URL with query parameters including info_hash, peer_id, port,
    /// uploaded, downloaded, left, and compact.
    ///
    /// # Returns
    ///
    /// Returns the constructed URI for the tracker request.
    ///
    /// # Errors
    ///
    /// Returns `TrackerError` if URI construction fails.
    async fn build_url(&'a self) -> Result<Uri, TrackerError> {
        //buffer for int to str
        let mut buffer = itoa::Buffer::new();

        let mut uri_parts = self.tracker.parse::<Uri>()?.into_parts();

        let path = uri_parts
            .path_and_query
            .as_ref()
            .map(|p| p.path())
            .unwrap_or("/");

        //construct query string with all tracker parameters
        let approx_query_capacity = path.len() + 100 + (20 * 3) * 2;
        let mut path_and_query = String::with_capacity(approx_query_capacity);

        //start with base path
        path_and_query.push_str(path);

        //add query delimiter
        if path.contains('?') {
            path_and_query.push('&');
        } else {
            path_and_query.push('?');
        }

        //build query parameters without intermediate allocations
        path_and_query.push_str("info_hash=");
        path_and_query.push_str(&self.url_info_hash);

        path_and_query.push_str("&peer_id=");
        path_and_query.push_str(&self.url_peer_id);

        path_and_query.push_str("&port=");
        path_and_query.push_str(buffer.format(self.port));

        let (uploaded, downloaded, left) = {
            let stats = self.stats.read().await;
            (stats.uploaded, stats.downloaded, stats.left)
        };

        path_and_query.push_str("&uploaded=");
        path_and_query.push_str(buffer.format(uploaded));

        path_and_query.push_str("&downloaded=");
        path_and_query.push_str(buffer.format(downloaded));

        path_and_query.push_str("&left=");
        path_and_query.push_str(buffer.format(left));

        path_and_query.push_str("&compact=");
        path_and_query.push(if self.compact { '1' } else { '0' });

        uri_parts.path_and_query = Some(PathAndQuery::try_from(path_and_query)?);

        Ok(Uri::from_parts(uri_parts)?)
    }
}

/// Represents a tracker announce response.
///
/// Contains the interval for the next announce and the list of peers.
#[derive(Debug)]
struct AnnounceResponseHTTP {
    /// Seconds to wait before sending the next announce request.
    interval: u64,
    /// List of peers in compact format (6 bytes each: 4 IP + 2 port).
    peers: Vec<[u8; 6]>,
}

impl<'a> BencodeDecodable<'a> for AnnounceResponseHTTP {
    fn decode(b: &'a Bencode) -> Result<Self, BencodeDecodableError> {
        //get dict from bencode
        let dict = Self::get_struct(b)?;

        //get interval value
        let interval = Self::get_u64(Self::get_struct_value("interval", dict)?)?;

        //get peers
        let peers_bytes = Self::get_str(Self::get_struct_value("peers", dict)?)?;
        if peers_bytes.len() % 6 != 0 {
            return Err(BencodeDecodableError::Other(
                format!(
                    "Peer data length {} is not a multiple of 6.",
                    peers_bytes.len()
                )
                .into(),
            ));
        }

        //get number of peers
        let peer_count = peers_bytes.len() / 6;
        //pre-allocate with exact capacity
        let mut peers = Vec::with_capacity(peer_count);

        //process peers
        for chunk in peers_bytes.chunks_exact(6) {
            let peer_bytes: [u8; 6] = chunk
                .try_into()
                .map_err(|e: TryFromSliceError| BencodeDecodableError::Other(e.into()))?;
            peers.push(peer_bytes);
        }

        Ok(Self { interval, peers })
    }
}

/// Sends a request to the tracker and processes the response.
async fn announce<'a>(req: &'a AnnounceRequestHTTP<'a>) -> Result<Bencode, TrackerError> {
    let url = req.build_url().await?;

    //set up connection to tracker
    let host = url
        .host()
        .ok_or(TrackerError::Other("Missing host in tracker URL".into()))?;
    let port = url.port_u16().unwrap_or(6969);

    let stream = TcpStream::connect((host, port)).await?;
    let io = TokioIo::new(stream);

    let (mut sender, conn) = handshake(io).await?;

    //spawn connection handler
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            println!("Connection failed: {:?}", err);
        }
    });

    let authority = url.authority().unwrap().clone();

    //build and send HTTP request
    let req = Request::builder()
        .uri(url)
        .header(hyper::header::HOST, authority.as_str())
        .body(Empty::<Bytes>::new())?;

    let res = sender.send_request(req).await?;

    let body_bytes: &[u8] = &res.collect().await?.to_bytes();

    //create a place to store the bencode
    let bencode_holder = from_buffer(body_bytes).map_err(BStreamingError::from)?;

    Ok(bencode_holder)
}
