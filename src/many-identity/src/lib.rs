mod address;
pub use address::{Address, MAX_SUBRESOURCE_ID};

mod identity;
pub use identity::*;

pub mod cose;

#[cfg(feature = "testing")]
pub mod testing {
    use super::Address;

    pub fn identity(seed: u32) -> Address {
        #[rustfmt::skip]
            let bytes = [
            1u8,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0, 0,
            (seed >> 24) as u8, (seed >> 16) as u8, (seed >> 8) as u8, (seed & 0xFF) as u8
        ];
        Address::from_bytes(&bytes).unwrap()
    }
}
