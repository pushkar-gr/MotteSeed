//! Peer ID generation.
//!
//! Generates a unique Azureus-style peer ID for the client.

use once_cell::sync::Lazy;
use rand::{Rng, rng};

/// Static peer ID that gets generated once per client session.
static PEER_ID: Lazy<[u8; 20]> = Lazy::new(|| {
    let mut id = [0u8; 20];
    //create an Azureus-style peer_id
    //-MS0100-[13 random bytes]

    //client identifier part
    id[0] = b'-';
    id[1] = b'M';
    id[2] = b'S';

    //version (v1.0.0)
    id[3] = b'0';
    id[4] = b'1';
    id[5] = b'0';
    id[6] = b'0';

    //separator
    id[7] = b'-';

    //random bytes
    let mut rng = rng();
    for i in id.iter_mut().skip(8) {
        *i = rng.random_range(33..=126);
    }

    id
});

/// Gets the static peer ID.
///
/// Returns a reference to the globally shared peer ID that is generated once
/// per client session.
///
/// # Returns
///
/// A reference to a 20-byte array containing the peer ID.
///
/// # Example
///
/// ```
/// use MotteSeed::core::peer::peer_id::get_peer_id;
///
/// let peer_id = get_peer_id();
/// assert_eq!(peer_id.len(), 20);
/// ```
pub fn get_peer_id() -> &'static [u8; 20] {
    &PEER_ID
}
