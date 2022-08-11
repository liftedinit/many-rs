//! An Identity is a signer that also has an address on the MANY protocol.
use crate::Address;
use coset::CoseSign1;
use many_error::ManyError;

pub trait Identity: Send {
    fn address(&self) -> Address;
    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError>;
}

pub trait Verifier: Send {
    fn sign_1(&self, envelope: &CoseSign1) -> Result<(), ManyError>;
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

impl Verifier for AnonymousIdentity {
    fn sign_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
        if !envelope.signature.is_empty() {
            Err(ManyError::could_not_verify_signature(
                "Anonymous should not have a signature.",
            ))
        } else {
            Ok(())
        }
    }
}

/// Accept ALL envelopes.
#[cfg(feature = "testing")]
pub struct AcceptAllVerifier;
impl Verifier for AcceptAllVerifier {
    fn sign_1(&self, _envelope: &CoseSign1) -> Result<(), ManyError> {
        Ok(())
    }
}

impl Identity for Box<dyn Identity> {
    fn address(&self) -> Address {
        (&**self).address()
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        (&**self).sign_1(envelope)
    }
}

impl Verifier for Box<dyn Verifier> {
    fn sign_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
        (&**self).sign_1(envelope)
    }
}
