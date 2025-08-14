use crate::core::torrent::torrent::FileDetails;

use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

//structure to represent file system
#[derive(Debug)]
pub struct DiskIO<'a> {
    base_path: PathBuf,
    file_details: Arc<FileDetails<'a>>,
    piece_length: u32,
    files: Vec<File>,
    total_size: u32,
}
