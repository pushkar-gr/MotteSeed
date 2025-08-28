use crate::core::torrent::torrent::FileDetails;

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::PathBuf;
use thiserror::Error;

//structure to represent file system
#[derive(Debug)]
pub struct DiskIO<'a> {
    base_path: PathBuf,                //base path of the files
    file_details: &'a FileDetails<'a>, //file details (single file/ multi file)
    piece_length: u32,                 //length of each piece
    files: Vec<File>,                  //opened files
    total_size: u64,                   //total size
}

impl<'a> DiskIO<'a> {
    //create a new object
    pub fn new(
        base_path: PathBuf,
        file_details: &'a FileDetails<'a>,
        piece_length: u32,
    ) -> Result<Self, DiskError> {
        //create base directory
        fs::create_dir_all(&base_path)?;

        let (files, total_size) = match file_details {
            //single file
            FileDetails::SingleFile { length } => {
                //create a directory "data" to store data
                let file_path = base_path.join("data");
                //open file with all requierd perms
                let file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(&file_path)?;

                file.set_len(*length)?;

                (vec![file], *length)
            }
            //multi files
            FileDetails::MultiFile { files } => {
                //vector to hold all files
                let mut open_files = Vec::with_capacity(files.len());
                let mut total_size = 0;

                for file_entry in files {
                    //get complete file path
                    let mut file_path = base_path.clone();
                    for component in &file_entry.path {
                        let component_str = std::str::from_utf8(component)
                            .map_err(|_| DiskError::PathError("Invalid UTF-8 in path".into()))?;
                        file_path.push(component_str);
                    }

                    //create parent directory
                    if let Some(parent) = file_path.parent() {
                        fs::create_dir_all(parent)?;
                    }

                    //open file with all required perms
                    let file = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create(true)
                        .open(&file_path)?;

                    file.set_len(file_entry.length)?;

                    open_files.push(file);
                    total_size += file_entry.length;
                }

                (open_files, total_size)
            }
        };

        Ok(Self {
            base_path,
            file_details,
            piece_length,
            files,
            total_size,
        })
    }
}

//custom error enum for disk operations
#[derive(Error, Debug)]
pub enum DiskError {
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("Path Error: {0}")]
    PathError(String),

    #[error("Error: {0}")]
    Other(#[from] Box<dyn std::error::Error>),
}
