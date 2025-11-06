//! Bencode decoding trait.
//!
//! Defines the `BencodeDecodable` trait for parsing bencode data.

use super::bencode_decodable_error::BencodeDecodableError;

use bencode::{Bencode, util::ByteString};
use std::{borrow::Cow, collections::BTreeMap};

/// A trait for decoding Bencode data into Rust types.
pub trait BencodeDecodable<'a>: Sized {
    /// Decodes Bencode into Self.
    ///
    /// # Arguments
    ///
    /// * `b` - A reference to the Bencode value to decode
    ///
    /// # Returns
    ///
    /// Returns `Ok(Self)` if decoding succeeds, or a `BencodeDecodableError` if it fails.
    ///
    /// # Errors
    ///
    /// Returns `BencodeDecodableError` if decoding fails.
    fn decode(b: &'a Bencode) -> Result<Self, BencodeDecodableError>;

    /// Extracts u64 value from a Bencode Number variant.
    ///
    /// # Arguments
    ///
    /// * `b` - A reference to the Bencode value to extract from
    ///
    /// # Returns
    ///
    /// Returns the extracted u64 value on success.
    ///
    /// # Errors
    ///
    /// Returns `BencodeDecodableError` if the Bencode value is not a Number or cannot be
    /// converted to u64.
    fn get_u64(b: &'a Bencode) -> Result<u64, BencodeDecodableError> {
        match b {
            Bencode::Number(num) => Ok((*num)
                .try_into()
                .map_err(|_| BencodeDecodableError::WrongType("Expected a Number".into()))?),
            _ => Err(BencodeDecodableError::WrongType("Expected a Number".into())),
        }
    }

    /// Extracts raw bytes from a Bencode ByteString variant.
    ///
    /// # Arguments
    ///
    /// * `b` - A reference to the Bencode value to extract from
    ///
    /// # Returns
    ///
    /// Returns a byte slice reference on success.
    ///
    /// # Errors
    ///
    /// Returns `BencodeDecodableError` if the Bencode value is not a ByteString.
    fn get_str(b: &'a Bencode) -> Result<&'a [u8], BencodeDecodableError> {
        match b {
            Bencode::ByteString(bytes) => Ok(bytes),
            _ => Err(BencodeDecodableError::WrongType(
                "Expected a ByteString".into(),
            )),
        }
    }

    /// Extract string from a Bencode ByteString variant.
    ///
    /// Converts the byte string to a UTF-8 string, replacing invalid sequences.
    ///
    /// # Arguments
    ///
    /// * `b` - A reference to the Bencode value to extract from
    ///
    /// # Returns
    ///
    /// Returns a `Cow<'a, str>` containing the decoded string.
    ///
    /// # Errors
    ///
    /// Returns `BencodeDecodableError` if the Bencode value is not a ByteString.
    fn get_string(b: &'a Bencode) -> Result<Cow<'a, str>, BencodeDecodableError> {
        let bytes = Self::get_str(b)?;
        Ok(String::from_utf8_lossy(bytes))
    }

    /// Extracts dictionary from a Bencode Dict variant.
    ///
    /// # Arguments
    ///
    /// * `b` - A reference to the Bencode value to extract from
    ///
    /// # Returns
    ///
    /// Returns a reference to the BTreeMap representing the dictionary.
    ///
    /// # Errors
    ///
    /// Returns `BencodeDecodableError` if the Bencode value is not a Dict.
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

    /// Retrieves value from a Bencode dictionary by key.
    ///
    /// # Arguments
    ///
    /// * `key` - The ByteString key to look up
    /// * `dict_map` - The dictionary to search in
    ///
    /// # Returns
    ///
    /// Returns a reference to the Bencode value associated with the key.
    ///
    /// # Errors
    ///
    /// Returns `BencodeDecodableError::KeyNotFound` if the key doesn't exist.
    fn get_struct_value_from_bytestring(
        key: &ByteString,
        dict_map: &'a BTreeMap<ByteString, Bencode>,
    ) -> Result<&'a Bencode, BencodeDecodableError> {
        dict_map
            .get(key)
            .ok_or_else(|| BencodeDecodableError::KeyNotFound(format!("Key '{}' not found", key)))
    }

    /// Retrieves value from a Bencode dictionary by string key.
    ///
    /// Convenience method that converts a `&str` to a ByteString before lookup.
    ///
    /// # Arguments
    ///
    /// * `key` - The string key to look up
    /// * `dict_map` - The dictionary to search in
    ///
    /// # Returns
    ///
    /// Returns a reference to the Bencode value associated with the key.
    ///
    /// # Errors
    ///
    /// Returns `BencodeDecodableError::KeyNotFound` if the key doesn't exist.
    fn get_struct_value(
        key: &str,
        dict_map: &'a BTreeMap<ByteString, Bencode>,
    ) -> Result<&'a Bencode, BencodeDecodableError> {
        Self::get_struct_value_from_bytestring(&ByteString::from_str(key), dict_map)
    }

    /// Extracts a list from a Bencode List variant.
    ///
    /// # Arguments
    ///
    /// * `b` - A reference to the Bencode value to extract from
    ///
    /// # Returns
    ///
    /// Returns a reference to the vector containing list elements.
    ///
    /// # Errors
    ///
    /// Returns `BencodeDecodableError` if the Bencode value is not a List.
    fn get_list(b: &'a Bencode) -> Result<&'a Vec<Bencode>, BencodeDecodableError> {
        match b {
            Bencode::List(list) => Ok(list),
            _ => Err(BencodeDecodableError::WrongType("Expected a list".into())),
        }
    }
}
