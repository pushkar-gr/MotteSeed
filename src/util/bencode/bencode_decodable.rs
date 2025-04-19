use crate::util::bencode::bencode_decodable_error::BencodeDecodableError;

use bencode::Bencode;
use bencode::util::ByteString;
use std::borrow::Cow;
use std::collections::BTreeMap;

//a trait for decoding Bencode data into Rust types
pub trait BencodeDecodable<'a>: Sized {
    //decode Bencode into Self
    fn decode(b: &'a Bencode) -> Result<Self, BencodeDecodableError>;

    //extract u64 value from a Bencode Number variant
    fn get_u64(b: &'a Bencode) -> Result<u64, BencodeDecodableError> {
        match b {
            Bencode::Number(num) => Ok((*num)
                .try_into()
                .map_err(|_| BencodeDecodableError::WrongType("Expected a Number".into()))?),
            _ => Err(BencodeDecodableError::WrongType("Expected a Number".into())),
        }
    }

    //extract raw bytes from a Bencode ByteString variant
    fn get_str(b: &'a Bencode) -> Result<&'a [u8], BencodeDecodableError> {
        match b {
            Bencode::ByteString(bytes) => Ok(bytes),
            _ => Err(BencodeDecodableError::WrongType(
                "Expected a ByteString".into(),
            )),
        }
    }

    //extract string from a Bencode ByteString variant
    fn get_string(b: &'a Bencode) -> Result<Cow<'a, str>, BencodeDecodableError> {
        let bytes = Self::get_str(b)?;
        Ok(String::from_utf8_lossy(bytes))
    }

    //extract dictionary from a Bencode Dict variant
    fn get_struct(
        b: &'a Bencode,
    ) -> Result<&'a BTreeMap<ByteString, Bencode>, BencodeDecodableError> {
        match b {
            Bencode::Dict(dict_map) => Ok(dict_map),
            _ => Err(BencodeDecodableError::WrongType(
                "Expected a dictionary".into(),
            )),
        }
    }

    //retrieve value from a Bencode dictionary by key
    fn get_struct_value_from_bytestring(
        key: &ByteString,
        dict_map: &'a BTreeMap<ByteString, Bencode>,
    ) -> Result<&'a Bencode, BencodeDecodableError> {
        dict_map
            .get(key)
            .ok_or_else(|| BencodeDecodableError::KeyNotFound(format!("Key '{}' not found", key)))
    }

    //retrieve value from a Bencode dictionary by key
    fn get_struct_value(
        key: &str,
        dict_map: &'a BTreeMap<ByteString, Bencode>,
    ) -> Result<&'a Bencode, BencodeDecodableError> {
        Self::get_struct_value_from_bytestring(&ByteString::from_str(key), dict_map)
    }

    //extracts a list from a Bencode List variant
    fn get_list(b: &'a Bencode) -> Result<&'a Vec<Bencode>, BencodeDecodableError> {
        match b {
            Bencode::List(list) => Ok(list),
            _ => Err(BencodeDecodableError::WrongType("Expected a list".into())),
        }
    }
}
