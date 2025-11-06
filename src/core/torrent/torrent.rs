//! Torrent file structure and parsing.
//!
//! Defines the Torrent and related structs, with BencodeDecodable implementations.

use super::torrent_error::ReadTorrentError;
use crate::util::{
    bencode::{
        bencode_decodable::BencodeDecodable, bencode_decodable_error::BencodeDecodableError,
    },
    errors::BStreamingError,
};

use bencode::{Bencode, from_buffer, util::ByteString};
use once_cell::sync::Lazy;
use sha1::{Digest, Sha1};
use std::{borrow::Cow, fs, path::Path, rc::Rc};

/// Cached "length" key for bencode decoding optimization.
static LENGTH_KEY: Lazy<ByteString> = Lazy::new(|| ByteString::from_str("length"));
/// Cached "path" key for bencode decoding optimization.
static PATH_KEY: Lazy<ByteString> = Lazy::new(|| ByteString::from_str("path"));

/// Represents a parsed torrent file.
///
/// This structure contains all the metadata from a .torrent file, including
/// tracker URLs, file information, and the computed info hash.
#[derive(Debug)]
pub struct Torrent<'a> {
    /// Primary tracker URL for peer discovery.
    pub announce: &'a [u8],
    /// Metadata about the files in this torrent.
    pub info: Info<'a>,
    /// SHA-1 hash of the bencoded info dictionary, used to identify the torrent.
    pub info_hash: [u8; 20],
    /// Optional comment from the torrent creator.
    pub comment: Option<&'a [u8]>,
    /// Optional name of the program that created the torrent.
    pub created_by: Option<&'a [u8]>,
    /// Optional Unix timestamp when the torrent was created.
    pub creation_date: Option<u64>,
    /// Optional character encoding used in the torrent.
    pub encoding: Option<&'a [u8]>,
    /// Optional list of backup tracker URLs organized in tiers.
    pub announce_list: Option<Vec<Vec<&'a [u8]>>>,
}

impl<'a> BencodeDecodable<'a> for Torrent<'a> {
    fn decode(b: &'a Bencode) -> Result<Self, BencodeDecodableError> {
        //get dict from bencode
        let dict = Self::get_struct(b)?;
        //get announce value
        let announce = Self::get_str(Self::get_struct_value("announce", dict)?)?;
        //get info dict
        let info_dict = Self::get_struct_value("info", dict)?;
        //decode info dict
        let info = Info::decode(info_dict)?;

        //get raw info bytes to calculate SHA1
        let info_bytes = info_dict
            .to_bytes()
            .map_err(|e| BencodeDecodableError::Other(e.into()))?;
        //calculate sha1 of info
        let mut hasher = Sha1::new();
        hasher.update(&info_bytes);
        let info_hash = hasher.finalize().into();

        //get comment value
        let comment = Self::get_struct_value("comment", dict)
            .ok()
            .and_then(|value| Self::get_str(value).ok());

        //get created_by value
        let created_by = Self::get_struct_value("created by", dict)
            .ok()
            .and_then(|value| Self::get_str(value).ok());

        //get creation_date value
        let creation_date = Self::get_struct_value("creation date", dict)
            .ok()
            .and_then(|value| Self::get_u64(value).ok());

        //get encoding value
        let encoding = Self::get_struct_value("encoding", dict)
            .ok()
            .and_then(|value| Self::get_str(value).ok());

        //get announce-list
        let announce_list =
            Self::get_struct_value("announce-list", dict)
                .ok()
                .and_then(|announce_list_value| {
                    Self::get_list(announce_list_value).ok().map(|outer_list| {
                        outer_list
                            .iter()
                            .filter_map(|inner_list| {
                                Self::get_list(inner_list).ok().map(|value| {
                                    value
                                        .iter()
                                        .filter_map(|url| Self::get_str(url).ok())
                                        .collect::<Vec<_>>()
                                })
                            })
                            .collect::<Vec<_>>()
                    })
                });

        Ok(Self {
            announce,
            info,
            info_hash,
            comment,
            created_by,
            creation_date,
            encoding,
            announce_list,
        })
    }
}

/// Represents the info dictionary of a torrent.
///
/// Contains metadata about the files, pieces, and piece hashes for the torrent.
#[derive(Debug)]
pub struct Info<'a> {
    /// Name of the torrent or the root file/directory.
    pub name: Cow<'a, str>,
    /// Size of each piece in bytes (except possibly the last piece).
    pub piece_length: u64,
    /// Concatenated SHA-1 hashes of all pieces (each hash is 20 bytes).
    pub raw_pieces: &'a [u8],
    /// Information about files: single file or multiple files.
    pub file_details: FileDetails<'a>,
    /// Optional flag indicating if this is a private torrent (1 = private).
    pub private: Option<u64>,
    /// Optional source identifier for private torrents.
    pub source: Option<&'a [u8]>,
}

impl<'a> Info<'a> {
    /// Gets the SHA-1 hash of a piece by index.
    ///
    /// # Arguments
    ///
    /// * `index` - The zero-based index of the piece
    ///
    /// # Returns
    ///
    /// Returns `Some(&[u8; 20])` containing the 20-byte SHA-1 hash if the index is valid,
    /// or `None` if the index is out of range.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::torrent::torrent::Info;
    /// # let info: Info = todo!();
    /// if let Some(hash) = info.piece_hash(0) {
    ///     println!("First piece hash: {:?}", hash);
    /// }
    /// ```
    pub fn piece_hash(&self, index: usize) -> Option<&[u8; 20]> {
        //compute start and end
        let start = index * 20;
        let end = start + 20;
        //check if in range
        if end <= self.raw_pieces.len() {
            //get the slice and convert it into a reference to a fixed-size array
            self.raw_pieces[start..end].try_into().ok()
        } else {
            None
        }
    }
}

impl<'a> BencodeDecodable<'a> for Info<'a> {
    fn decode(b: &'a Bencode) -> Result<Self, BencodeDecodableError> {
        //get dict from bencode
        let dict = Self::get_struct(b)?;
        //get name value
        let name = Self::get_string(Self::get_struct_value("name", dict)?)?;
        //get piece length value
        let piece_length = Self::get_u64(Self::get_struct_value("piece length", dict)?)?;
        //get raw pieces
        let raw_pieces = Self::get_str(Self::get_struct_value("pieces", dict)?)?;

        //validate that pieces data contains complete SHA-1 hashes (each hash is exactly 20 bytes)
        if raw_pieces.len() % 20 != 0 {
            return Err(BencodeDecodableError::Other("Invalid pieces length".into()));
        }

        //get file details
        //get length value. If found, single file. Else multi file
        let file_details = match Self::get_struct_value("length", dict) {
            Ok(b) => FileDetails::SingleFile {
                length: Self::get_u64(b)?,
            },
            _ => FileDetails::MultiFile {
                //get files details
                files: {
                    //get file list value
                    let file_list = Self::get_list(Self::get_struct_value("files", dict)?)?;

                    let mut files = Vec::with_capacity(file_list.len());
                    //fill files from file list
                    for file_item in file_list {
                        files.push(FileEntry::decode(file_item)?)
                    }

                    files
                },
            },
        };

        //get private value
        let private = Self::get_struct_value("private", dict)
            .ok()
            .and_then(|value| Self::get_u64(value).ok());

        //get source value
        let source = Self::get_struct_value("source", dict)
            .ok()
            .and_then(|value| Self::get_str(value).ok());

        Ok(Self {
            name,
            piece_length,
            raw_pieces,
            file_details,
            private,
            source,
        })
    }
}

/// Represents file information in a torrent.
///
/// A torrent can contain either a single file or multiple files. This enum
/// distinguishes between these two cases.
#[derive(Debug)]
pub enum FileDetails<'a> {
    /// A single-file torrent.
    SingleFile {
        /// Length of the file in bytes.
        length: u64,
    },
    /// A multi-file torrent.
    MultiFile {
        /// List of files in the torrent.
        files: Vec<FileEntry<'a>>,
    },
}

/// Represents a file entry in multi-file torrents.
///
/// Each file entry contains the file size and path components.
#[derive(Debug)]
pub struct FileEntry<'a> {
    /// Length of the file in bytes.
    pub length: u64,
    /// Path components forming the file path relative to the torrent root.
    pub path: Vec<&'a [u8]>,
}

impl<'a> BencodeDecodable<'a> for FileEntry<'a> {
    fn decode(b: &'a Bencode) -> Result<Self, BencodeDecodableError> {
        //get dict from bencode
        let dict = Self::get_struct(b)?;
        //get length value
        let length = Self::get_u64(Self::get_struct_value_from_bytestring(&LENGTH_KEY, dict)?)?;
        //get path list value
        let path_list = Self::get_list(Self::get_struct_value_from_bytestring(&PATH_KEY, dict)?)?;

        let mut path = Vec::with_capacity(path_list.len());
        //file path from path list
        for path_item in path_list {
            path.push(Self::get_str(path_item)?);
        }

        Ok(Self { length, path })
    }
}

/// Wrapper for parsed torrent data with lifetime management.
///
/// This structure manages the lifetime of the underlying data and bencode
/// structures, ensuring they remain valid while the `Torrent` struct references them.
/// It uses reference counting to safely extend lifetimes.
#[derive(Debug)]
pub struct TorrentFile<'a> {
    /// Reference-counted storage for the raw torrent file bytes.
    _data: Rc<Vec<u8>>,
    /// Reference-counted storage for the parsed bencode structure.
    _bencode: Rc<Bencode>,
    /// The parsed torrent metadata that references the data.
    pub torrent: Torrent<'a>,
}

impl<'a> TorrentFile<'a> {
    /// Creates a TorrentFile from raw bytes.
    ///
    /// # Arguments
    ///
    /// * `bytes` - The raw bytes of a .torrent file
    ///
    /// # Returns
    ///
    /// Returns `Ok(TorrentFile)` on success.
    ///
    /// # Errors
    ///
    /// Returns `ReadTorrentError` if:
    /// - The bytes are not valid bencode data
    /// - The torrent structure is malformed or missing required fields
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::torrent::torrent::TorrentFile;
    /// let torrent_data = vec![/* torrent file bytes */];
    /// let torrent_file = TorrentFile::from_bytes(torrent_data)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, ReadTorrentError> {
        //create reference-counted data
        let data = Rc::new(bytes);

        //create a place to store the bencode
        let bencode_holder = Rc::new(from_buffer(&data).map_err(BStreamingError::from)?);

        //extract the bencode and create a 'static reference
        //this is safe because we ensure the data lives as long as TorrentFile
        let bencode_static = unsafe {
            let bencode_ref = bencode_holder.as_ref();
            std::mem::transmute::<&Bencode, &'static Bencode>(bencode_ref)
        };

        //parse the torrent
        let torrent = Torrent::decode(bencode_static)?;

        Ok(TorrentFile {
            _data: data,
            _bencode: bencode_holder,
            torrent,
        })
    }

    /// Creates a TorrentFile from a file path.
    ///
    /// # Arguments
    ///
    /// * `file` - Path to the .torrent file
    ///
    /// # Returns
    ///
    /// Returns `Ok(TorrentFile)` on success.
    ///
    /// # Errors
    ///
    /// Returns `ReadTorrentError` if:
    /// - The file cannot be read from disk
    /// - The file is not valid bencode data
    /// - The torrent structure is malformed
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use MotteSeed::core::torrent::torrent::TorrentFile;
    /// # use std::path::Path;
    /// let torrent_file = TorrentFile::from_file(Path::new("example.torrent"))?;
    /// println!("Loaded torrent: {:?}", torrent_file.torrent.info.name);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_file(file: &Path) -> Result<Self, ReadTorrentError> {
        let content = fs::read(file).map_err(ReadTorrentError::IO)?;
        Self::from_bytes(content)
    }
}
