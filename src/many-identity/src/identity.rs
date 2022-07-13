//! An Identity is a signer that also has an address on the MANY protocol.
use crate::Address;
use many_error::ManyError;

pub(crate) mod cose;

pub use cose::*;

pub trait Identity {
    fn address(&self) -> Address;
    fn sign(&self, _envelope: coset::CoseSign) -> Result<coset::CoseSign, ManyError>;
}
