use crate::core::peer::peer::Peer;

use http::uri::{InvalidUri, InvalidUriParts, PathAndQuery};
use http::{Request, Uri};
use http_body_util::{BodyExt, Empty};
use hyper::body::Bytes;
use hyper::client::conn::http1::handshake;
use hyper_util::rt::TokioIo;
use std::str::Utf8Error;
use std::time::Instant;
use thiserror::Error;
use tokio::net::TcpStream;

//represents a request to be sent to a BitTorrent tracker
#[derive(Debug)]
pub struct TrackerRequest<'a> {
    tracker: &'a [u8],     //tracker URL as bytes
    url_info_hash: String, //URL-encoded info hash
    url_peer_id: String,   //URL-encoded peer ID
    port: u16,             //port number for incoming connections
    uploaded: u64,         //total bytes uploaded
    downloaded: u64,       //total bytes downloaded
    left: u64,             //bytes left to download
    compact: bool,         //whether to request compact peer list
}

impl<'a> TrackerRequest<'a> {
    //create a new tracker request
    pub fn new(
        tracker: &'a [u8],
        info_hash: &'a [u8; 20],
        peer_id: &'a [u8; 20],
        port: u16,
        uploaded: u64,
        downloaded: u64,
        left: u64,
        compact: bool,
    ) -> Result<Self, TrackerError> {
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
        //pre-allocate capacity - worst case: all bytes need %XX encoding (3 chars each)
        let mut result = String::with_capacity(bytes.len() * 3);

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
    pub fn build_url(&'a self) -> Result<Uri, TrackerError> {
        let mut uri_parts = Uri::from_maybe_shared(self.tracker.to_vec())?.into_parts();

        let path = uri_parts
            .path_and_query
            .as_ref()
            .map(|p| p.path())
            .unwrap_or("/");

        //construct query string with all tracker parameters
        let approx_query_capacity = (10 + 20 * 3) * 2 + 30;
        let mut query_string = String::with_capacity(approx_query_capacity);

        query_string.push_str("?info_hash=");
        query_string.push_str(&self.url_info_hash);

        query_string.push_str("&peer_id=");
        query_string.push_str(&self.url_peer_id);

        query_string.push_str("&port=");
        query_string.push_str(&self.port.to_string());

        query_string.push_str("&uploaded=");
        query_string.push_str(&self.uploaded.to_string());

        query_string.push_str("&downloaded=");
        query_string.push_str(&self.downloaded.to_string());

        query_string.push_str("&left=");
        query_string.push_str(&self.left.to_string());

        query_string.push_str("&compact=");
        query_string.push_str(if self.compact { "1" } else { "0" });

        //combine path with query string
        let mut path_and_query = String::with_capacity(path.len() + query_string.len());
        path_and_query.push_str(path);
        path_and_query.push_str(&query_string);

        uri_parts.path_and_query = Some(PathAndQuery::try_from(path_and_query)?);

        Ok(Uri::from_parts(uri_parts)?)
    }
}

//manages communication with a BitTorrent tracker
#[derive(Debug)]
pub struct Tracker<'a> {
    interval: u64,         //seconds between tracker requests
    last_request: Instant, //time of last tracker request
    peers: Vec<Peer<'a>>,  //list of peers received from tracker
}

//possible errors that can occur during tracker operations
#[derive(Error, Debug)]
pub enum TrackerError {
    #[error("Invalid Uri: {0}")]
    InvalidUri(#[from] InvalidUri),

    #[error("Stream Error: {0}")]
    StreamError(#[from] std::io::Error),

    #[error("Hyper Error: {0}")]
    HyperError(#[from] hyper::Error),

    #[error("Hyper http Error: {0}")]
    HttpError(#[from] hyper::http::Error),

    #[error("UTF8 Error: {0}")]
    UTF8Error(#[from] Utf8Error),

    #[error("Invalid URI Parts: {0}")]
    InvalidURIParts(#[from] InvalidUriParts),

    #[error("Error: {0}")]
    Other(String),
}

impl<'a> Tracker<'a> {
    //create a new tracker and sends an initial request
    pub async fn new(req: &'a TrackerRequest<'a>) -> Result<Self, TrackerError> {
        let mut tracker = Self {
            interval: 0,
            last_request: Instant::now(),
            peers: Vec::new(),
        };

        tracker.send_request(req).await?;

        Ok(tracker)
    }

    //send a request to the tracker and processes the response
    async fn send_request(&mut self, req: &'a TrackerRequest<'a>) -> Result<(), TrackerError> {
        self.last_request = Instant::now();

        let url = req.build_url()?;

        //set up connection to tracker
        let host = url
            .host()
            .ok_or(TrackerError::Other("Invalid host".into()))?;
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

        //process response data
        println!("{:?}", body_bytes);

        Ok(())
    }

    //get peers from tracker, making a new request if needed
    pub async fn get_peers(
        &'a mut self,
        req: &'a TrackerRequest<'a>,
    ) -> Result<&'a Vec<Peer<'a>>, TrackerError> {
        //request again if interval has passed
        if self.last_request.elapsed().as_secs() > self.interval {
            self.send_request(req).await?;
        }
        Ok(&self.peers)
    }
}
