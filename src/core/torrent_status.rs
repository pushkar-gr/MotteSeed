use std::sync::Arc;
use tokio::sync::RwLock;

//manages torrent status 
#[derive(Debug)]
pub struct TorrentStatus {
    pub downloaded: Arc<RwLock<u64>>,
    pub uploaded: Arc<RwLock<u64>>,
    pub left: Arc<RwLock<u64>>,
}

impl TorrentStatus {
    //create a new object
    pub fn new(total_size: u64) -> Self {
        Self {
            downloaded: Arc::new(RwLock::new(0)),
            uploaded: Arc::new(RwLock::new(0)),
            left: Arc::new(RwLock::new(total_size)),
        }
    }

    //update downloaded bytes
    pub async fn update_downloaded(&self, bytes: u64) {
        let mut downloaded = self.downloaded.write().await;
        let mut left = self.left.write().await;
        *downloaded += bytes;
        *left = left.saturating_sub(bytes);
    }

    //update uploaded bytes
    pub async fn update_uploaded(&self, bytes: u64) {
        let mut uploaded = self.uploaded.write().await;
        *uploaded += bytes;
    }

    //update left bytes
    pub async fn updated_left(&self, bytes: u64) {
        let mut left = self.left.write().await;
        *left += bytes;
    }
}
