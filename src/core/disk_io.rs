//! Disk I/O for torrent files.
//!
//! Handles reading and writing pieces to disk for both single-file and multi-file torrents.
//! Manages file creation, piece-to-file mapping, and concurrent access using async mutexes.

use crate::core::torrent::torrent::{FileDetails, FileEntry};

use bytes::Bytes;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::PathBuf,
};
use thiserror::Error;
use tokio::sync::Mutex;

/// Manages disk I/O operations for torrent data.
///
/// This structure handles both single-file and multi-file torrents, managing file
/// handles and coordinating piece writes/reads across potentially multiple files.
#[derive(Debug)]
pub struct DiskIO<'a> {
    /// Base directory path where torrent files are stored.
    base_path: PathBuf,
    /// File details indicating single or multi-file torrent structure.
    file_details: &'a FileDetails<'a>,
    /// Length of each piece in bytes.
    piece_length: u32,
    /// Opened file handles with async mutex protection for concurrent access.
    files: Vec<Mutex<File>>,
    /// Total size of all files in the torrent.
    total_size: u64,
}

impl<'a> DiskIO<'a> {
    /// Creates a new DiskIO instance.
    ///
    /// Initializes file structure on disk, creating directories and pre-allocating
    /// files to their expected sizes.
    ///
    /// # Arguments
    ///
    /// * `base_path` - The base directory where torrent files will be stored
    /// * `file_details` - Single or multi-file torrent information
    /// * `piece_length` - The length of each piece in bytes
    ///
    /// # Returns
    ///
    /// Returns a new DiskIO instance with all files opened and ready.
    ///
    /// # Errors
    ///
    /// Returns `DiskError` if:
    /// - Directory or file creation fails
    /// - File path contains invalid UTF-8 (for multi-file torrents)
    /// - File pre-allocation fails
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
                    .truncate(false)
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
                        .truncate(false)
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

    /// Writes a piece to disk.
    ///
    /// Writes the piece data to the appropriate location in the file(s).
    /// For multi-file torrents, the piece may span multiple files.
    ///
    /// # Arguments
    ///
    /// * `piece_index` - The zero-based index of the piece to write
    /// * `data` - The piece data to write
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful write.
    ///
    /// # Errors
    ///
    /// Returns `DiskError` if:
    /// - The piece index is out of bounds
    /// - I/O error occurs during writing
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

    /// Writes a piece to multiple files.
    ///
    /// Internal method that handles writing a piece when it spans multiple files
    /// in a multi-file torrent. Calculates file offsets and distributes the piece
    /// data across the appropriate files.
    ///
    /// # Arguments
    ///
    /// * `piece_index` - The piece index to write
    /// * `data` - The piece data
    /// * `file_entries` - The list of files in the torrent
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful write.
    ///
    /// # Errors
    ///
    /// Returns `DiskError` if the piece index is out of bounds or I/O errors occur.
    #[warn(clippy::ptr_arg)]
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

    /// Reads a piece from disk.
    ///
    /// Reads the piece data from the appropriate location in the file(s).
    /// For multi-file torrents, the piece may span multiple files.
    ///
    /// # Arguments
    ///
    /// * `piece_index` - The zero-based index of the piece to read
    ///
    /// # Returns
    ///
    /// Returns the piece data as Bytes.
    ///
    /// # Errors
    ///
    /// Returns `DiskError` if:
    /// - The piece index is out of bounds
    /// - I/O error occurs during reading
    pub async fn read_piece(&self, piece_index: u32) -> Result<Bytes, DiskError> {
        //get offset and verify
        let piece_offset = (piece_index * self.piece_length) as u64;

        if piece_offset >= self.total_size {
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

        match self.file_details {
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

    /// Reads a piece from multiple files.
    ///
    /// Internal method that handles reading a piece when it spans multiple files
    /// in a multi-file torrent. Calculates file offsets and collects the piece
    /// data from the appropriate files.
    ///
    /// # Arguments
    ///
    /// * `piece_index` - The piece index to read
    /// * `buffer` - The buffer to read data into
    /// * `file_entries` - The list of files in the torrent
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful read.
    ///
    /// # Errors
    ///
    /// Returns `DiskError` if the piece index is out of bounds or I/O errors occur.
    #[warn(clippy::ptr_arg)]
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

/// Custom error enum for disk I/O operations.
#[derive(Error, Debug)]
pub enum DiskError {
    /// I/O error during file operations.
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),

    /// Error with file path (e.g., invalid UTF-8).
    #[error("Path Error: {0}")]
    PathError(String),

    /// Invalid piece index or piece data.
    #[error("Invalid piece: {0}")]
    InvalidPiece(String),

    /// Other errors that may occur during disk operations.
    #[error("Error: {0}")]
    Other(#[from] Box<dyn std::error::Error>),
}
