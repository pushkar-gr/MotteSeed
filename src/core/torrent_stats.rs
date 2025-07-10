//manages torrent status
#[derive(Debug)]
pub struct TorrentStats {
    pub downloaded: u64,
    pub uploaded: u64,
    pub left: u64,
}

impl TorrentStats {
    //create a new object
    pub fn new(total_size: u64) -> Self {
        Self {
            downloaded: 0,
            uploaded: 0,
            left: total_size,
        }
    }

    //update downloaded bytes
    pub async fn update_downloaded(&mut self, bytes: u64) {
        self.downloaded += bytes;
        self.left = self.left.saturating_sub(bytes);
    }

    //update uploaded bytes
    pub async fn update_uploaded(&mut self, bytes: u64) {
        self.uploaded += bytes;
    }

    //update left bytes
    pub async fn updated_left(&mut self, bytes: u64) {
        self.left += bytes;
    }
}
