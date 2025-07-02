use crate::core::peer::peer::Peer;
use crate::core::tracker::tracker::Tracker;
use crate::core::tracker::tracker_error::TrackerError;
use crate::util::bencode::bencode_decodable::BencodeDecodable;
use crate::util::bencode::bencode_decodable_error::BencodeDecodableError;
use crate::util::errors::BStreamingError;

use bencode::{Bencode, from_buffer};
use http::uri::PathAndQuery;
use http::{Request, Uri};
use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::client::conn::http1::handshake;
use hyper_util::rt::TokioIo;
use itoa;
use std::array::TryFromSliceError;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpStream;
use tokio::sync::RwLock;

//manages communication with a BitTorrent tracker
#[derive(Debug)]
pub struct TrackerHTTP<'a> {
    request: AnnounceRequestHTTP<'a>, //request object
    last_announce: Instant,           //time of last tracker request
    response: AnnounceResponseHTTP,   //response by tracker
}

impl<'a> TrackerHTTP<'a> {
    //send a request to the tracker and processes the response
    async fn announce(&mut self) -> Result<Bencode, TrackerError> {
        let response = announce(&self.request).await?;
        self.last_announce = Instant::now();
        Ok(response)
    }
}

impl<'a> Tracker<'a> for TrackerHTTP<'a> {
    //create a new tracker and sends an initial request
    async fn new(
        tracker: &'a [u8],
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        downloaded: Arc<RwLock<u64>>,
        left: Arc<RwLock<u64>>,
        uploaded: Arc<RwLock<u64>>,
        port: u16,
    ) -> Result<Self, TrackerError> {
        let request = AnnounceRequestHTTP::new(
            tracker, info_hash, peer_id, port, uploaded, downloaded, left, true,
        )?;
        let response_bencode = announce(&request).await?;

        Ok(Self {
            request,
            last_announce: Instant::now(),
            response: AnnounceResponseHTTP::decode(&response_bencode)?,
        })
    }

    //get peers from tracker, making a new request if needed
    async fn get_peers(&mut self) -> Result<&Vec<Peer>, TrackerError> {
        //request again if interval has passed
        if self.last_announce.elapsed().as_secs() > self.response.interval {
            let response_bencode = self.announce().await?;
            self.response = AnnounceResponseHTTP::decode(&response_bencode)?;
        }
        Ok(&self.response.peers)
    }
}

//represents a request to be sent to a BitTorrent tracker
#[derive(Debug)]
struct AnnounceRequestHTTP<'a> {
    tracker: &'a str,             //tracker URL as str
    url_info_hash: String,        //URL-encoded info hash
    url_peer_id: String,          //URL-encoded peer ID
    port: u16,                    //port number for incoming connections
    uploaded: Arc<RwLock<u64>>,   //total bytes uploaded
    downloaded: Arc<RwLock<u64>>, //total bytes downloaded
    left: Arc<RwLock<u64>>,       //bytes left to download
    compact: bool,                //whether to request compact peer list
}

impl<'a> AnnounceRequestHTTP<'a> {
    //create a new tracker request
    fn new(
        tracker: &'a [u8],
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        port: u16,
        uploaded: Arc<RwLock<u64>>,
        downloaded: Arc<RwLock<u64>>,
        left: Arc<RwLock<u64>>,
        compact: bool,
    ) -> Result<Self, TrackerError> {
        //convert uri from bytes to str
        let tracker = std::str::from_utf8(tracker)?;

        Ok(Self {
            tracker,
            url_info_hash: Self::url_encode(info_hash),
            url_peer_id: Self::url_encode(peer_id),
            port,
            uploaded,
            downloaded,
            left,
            compact,
        })
    }

    //URL encodes a 20-byte value for use in tracker requests
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

    //build a complete tracker request URL with all required parameters
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

        let uploaded = self.uploaded.read().await;
        path_and_query.push_str("&uploaded=");
        path_and_query.push_str(buffer.format(*uploaded));

        let downloaded = self.uploaded.read().await;
        path_and_query.push_str("&downloaded=");
        path_and_query.push_str(buffer.format(*downloaded));

        let left = self.uploaded.read().await;
        path_and_query.push_str("&left=");
        path_and_query.push_str(buffer.format(*left));

        path_and_query.push_str("&compact=");
        path_and_query.push(if self.compact { '1' } else { '0' });

        uri_parts.path_and_query = Some(PathAndQuery::try_from(path_and_query)?);

        Ok(Uri::from_parts(uri_parts)?)
    }
}

//represents a reponse sent by a trakcer
#[derive(Debug)]
struct AnnounceResponseHTTP {
    interval: u64,    //seconds between tracker requests
    peers: Vec<Peer>, //list of peers received from tracker
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
            peers.push(
                Peer::decode(&peer_bytes).map_err(|e| BencodeDecodableError::Other(e.into()))?,
            );
        }

        Ok(Self { interval, peers })
    }
}

//send a request to the tracker and processes the response
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
