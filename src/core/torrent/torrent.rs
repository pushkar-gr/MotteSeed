use crate::core::torrent::torrent_error::{BStreamingError, ReadTorrentError};
use crate::util::bencode::bencode_decodable::BencodeDecodable;
use crate::util::bencode::bencode_decodable_error::BencodeDecodableError;

use bencode::util::ByteString;
use bencode::{Bencode, from_buffer};
use once_cell::sync::Lazy;
use sha1::{Digest, Sha1};
use std::borrow::Cow;
use std::fs;
use std::path::Path;
use std::rc::Rc;

//define cached keys
static LENGTH_KEY: Lazy<ByteString> = Lazy::new(|| ByteString::from_str("length"));
static PATH_KEY: Lazy<ByteString> = Lazy::new(|| ByteString::from_str("path"));

#[derive(Debug)]
pub struct Torrent<'a> {
    pub announce: &'a [u8],  //tracker URL
    pub info: Info<'a>,      //main metadata
    pub info_hash: [u8; 20], //SHA1 encoding of bencode value of info
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

        Ok(Self {
            announce,
            info,
            info_hash,
        })
    }
}

#[derive(Debug)]
pub struct Info<'a> {
    pub name: Cow<'a, str>,            //torrent name/file name
    pub piece_length: u64,             //size of each piece in bytes
    pub raw_pieces: &'a [u8], //raw bytes representing the concatenated SHA-1 hashes of all pieces
    pub file_details: FileDetails<'a>, //single/multi file torrent
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

        Ok(Self {
            name,
            piece_length,
            raw_pieces,
            file_details,
        })
    }
}

#[derive(Debug)]
pub enum FileDetails<'a> {
    SingleFile { length: u64 }, //file length in bytes for single file torrent
    MultiFile { files: Vec<FileEntry<'a>> }, //list of files for multi file torrent
}

#[derive(Debug)]
pub struct FileEntry<'a> {
    pub length: u64,         //file length in bytes
    pub path: Vec<&'a [u8]>, //path components
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

impl<'a> Info<'a> {
    //get SHA1 of a index from raw_pieces
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

#[derive(Debug)]
pub struct TorrentFile {
    _data: Rc<Vec<u8>>,            //store data to ensure it stays alive
    _bencode: Rc<Bencode>,         //store bencode to ensure it stays alive
    pub torrent: Torrent<'static>, //parsed torrent that references the data
}

impl TorrentFile {
    //create TorrentFile from bytes
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

    //create TorrentFile from file
    pub fn from_file(file: &Path) -> Result<Self, ReadTorrentError> {
        let content = fs::read(file).map_err(ReadTorrentError::IOError)?;
        Self::from_bytes(content)
    }
}
