use crate::core::torrent::torrent::{FileDetails, FileEntry};

use bytes::Bytes;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use thiserror::Error;
use tokio::sync::Mutex;

//structure to represent file system
#[derive(Debug)]
pub struct DiskIO<'a> {
    base_path: PathBuf,                //base path of the files
    file_details: &'a FileDetails<'a>, //file details (single file/ multi file)
    piece_length: u32,                 //length of each piece
    files: Vec<Mutex<File>>,           //opened files
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

                (vec![Mutex::new(file)], *length)
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

                    open_files.push(Mutex::new(file));
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

    //write a piece to disk
    pub async fn write_piece(&self, piece_index: u32, data: &Bytes) -> Result<(), DiskError> {
        //get offset and verify
        let piece_offset = piece_index * self.piece_length;

        if piece_offset as u64 >= self.total_size {
            return Err(DiskError::InvalidPiece(format!(
                "Piece index out of bounds: {}",
                piece_index
            )));
        }

        match self.file_details {
            //write to single file
            FileDetails::SingleFile { .. } => {
                let mut file = self.files[0].lock().await;
                file.seek(SeekFrom::Start(piece_offset as u64))?;
                file.write_all(data)?;
            }
            //write for multiple files
            FileDetails::MultiFile { files } => {
                self.write_multi_file_piece(piece_index, data, files)
                    .await?;
            }
        }

        Ok(())
    }

    //write piece to multi file
    async fn write_multi_file_piece(
        &self,
        piece_index: u32,
        data: &Bytes,
        file_entries: &Vec<FileEntry<'a>>,
    ) -> Result<(), DiskError> {
        //calculate offset
        let piece_offset = (piece_index * self.piece_length) as u64;
        let mut remaining = data.len();
        let mut data_offset = 0;

        //get fil index and file offset
        let mut current_offset = 0;
        let mut file_index = 0;

        while file_index < file_entries.len() {
            let file_length = file_entries[file_index].length;

            if current_offset + file_length > piece_offset {
                break;
            }

            current_offset += file_length;
            file_index += 1;
        }

        if file_index >= file_entries.len() {
            return Err(DiskError::InvalidPiece(format!(
                "Piece index out of bounds: {}",
                piece_index
            )));
        }

        let mut file_offset = piece_offset - current_offset;

        //write all bytes to files
        while remaining > 0 && file_index < file_entries.len() {
            let file_length = file_entries[file_index].length;
            let length_to_write = std::cmp::min(remaining, (file_length - file_offset) as usize);

            let mut file = self.files[file_index].lock().await;
            file.seek(SeekFrom::Start(piece_offset))?;
            file.write_all(&data[data_offset..data_offset + length_to_write])?;

            data_offset += length_to_write;
            remaining -= length_to_write;

            file_index += 1;
            file_offset = 0;
        }

        Ok(())
    }

    //read piece from file
    pub async fn read_piece(&self, piece_index: u32) -> Result<Bytes, DiskError> {
        //get offset and verify
        let piece_offset = (piece_index * self.piece_length) as u64;

        if piece_offset as u64 >= self.total_size {
            return Err(DiskError::InvalidPiece(format!(
                "Piece index out of bounds: {}",
                piece_index
            )));
        }

        //get piece size
        let piece_size = if ((piece_index + 1) * self.piece_length) as u64 > self.total_size {
            self.total_size - piece_offset
        } else {
            self.piece_length as u64
        };

        let mut buffer = vec![0u8; piece_size as usize];

        match &*self.file_details {
            //read from single file
            FileDetails::SingleFile { .. } => {
                let mut file = self.files[0].lock().await;
                file.seek(SeekFrom::Start(piece_offset))?;
                file.read_exact(&mut buffer)?;
            }
            //read from multi file
            FileDetails::MultiFile { files } => {
                self.read_multi_file_piece(piece_index, &mut buffer, files)
                    .await?;
            }
        }

        Ok(Bytes::from(buffer))
    }

    //read piece from files
    async fn read_multi_file_piece(
        &self,
        piece_index: u32,
        buffer: &mut [u8],
        file_entries: &Vec<FileEntry<'a>>,
    ) -> Result<(), DiskError> {
        //calculate offset
        let piece_offset = (piece_index * self.piece_length) as u64;
        let mut remaining = buffer.len();
        let mut data_offset = 0;

        //get fil index and file offset
        let mut current_offset = 0;
        let mut file_index = 0;

        while file_index < file_entries.len() {
            let file_length = file_entries[file_index].length;

            if current_offset + file_length > piece_offset {
                break;
            }

            current_offset += file_length;
            file_index += 1;
        }

        if file_index >= file_entries.len() {
            return Err(DiskError::InvalidPiece(format!(
                "Piece index out of bounds: {}",
                piece_index
            )));
        }

        let mut file_offset = piece_offset - current_offset;

        //write all bytes to files
        while remaining > 0 && file_index < file_entries.len() {
            let file_length = file_entries[file_index].length;
            let bytes_to_read = std::cmp::min(remaining, (file_length - file_offset) as usize);

            let mut file = self.files[file_index].lock().await;
            file.seek(SeekFrom::Start(file_offset))?;
            file.read_exact(&mut buffer[data_offset..data_offset + bytes_to_read])?;

            data_offset += bytes_to_read;
            remaining -= bytes_to_read;

            file_index += 1;
            file_offset = 0;
        }

        Ok(())
    }
}

//custom error enum for disk operations
#[derive(Error, Debug)]
pub enum DiskError {
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    #[error("Path Error: {0}")]
    PathError(String),

    #[error("Invalid piece: {0}")]
    InvalidPiece(String),

    #[error("Error: {0}")]
    Other(#[from] Box<dyn std::error::Error>),
}
