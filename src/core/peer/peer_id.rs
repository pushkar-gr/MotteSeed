use once_cell::sync::Lazy;
use rand::{Rng, rng};

//static peer_id that gets generated once per client session
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
    for i in 8..20 {
        id[i] = rng.random_range(33..=126);
    }

    id
});

//get peer id
pub fn get_peer_id() -> &'static [u8; 20] {
    &PEER_ID
}
