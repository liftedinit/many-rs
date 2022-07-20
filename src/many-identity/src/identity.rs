//! An Identity is a signer that also has an address on the MANY protocol.
use crate::Address;
use coset::CoseSign1;
use many_error::ManyError;

pub trait Identity {
    fn address(&self) -> Address;
    fn sign_1(&self, _envelope: CoseSign1) -> Result<CoseSign1, ManyError>;
}

pub struct AnonymousIdentity;

impl Identity for AnonymousIdentity {
    fn address(&self) -> Address {
        Address::anonymous()
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        // An anonymous envelope has no signature, or special header.
        Ok(envelope)
    }
}
