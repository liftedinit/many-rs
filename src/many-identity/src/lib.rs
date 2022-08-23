use many_error::ManyError;

mod address;

#[cfg(feature = "coset")]
pub mod cose_helpers;

#[cfg(feature = "coset")]
mod identity;

pub use address::Address;
#[cfg(feature = "coset")]
pub use identity::*;

pub mod hsm;

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
