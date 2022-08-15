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

#[derive(Clone)]
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

pub mod verifiers {
    use crate::{Address, Verifier};
    use coset::CoseSign1;
    use many_error::ManyError;

    #[derive(Clone)]
    pub struct OneOf<L: Verifier, R: Verifier>(L, R);

    impl OneOf {
        pub fn empty() -> Self {
            Self::new([])
        }

        pub fn push(&mut self, v: impl Verifier + Clone + 'static) {
            self.0.push(Box::new(v))
        }
    }

    impl Verifier for OneOf {
        fn sign_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
            let mut errors = Vec::with_capacity(8);
            for v in self.0.iter() {
                if let Err(e) = v.sign_1(envelope) {
                    errors.push(e);
                } else {
                    return Ok(());
                }
            }

            Err(ManyError::unknown(format!(
                "Could not verify: [{}]",
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<String>>()
                    .join(", ")
            )))
        }
    }

    pub struct AnonymousVerifier;

    impl Verifier for AnonymousVerifier {
        fn sign_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
            let kid = &envelope.protected.header.key_id;
            if !kid.is_empty() {
                if Address::from_bytes(kid)?.is_anonymous() {
                    Ok(())
                } else {
                    Err(ManyError::unknown("Anonymous requires no key id."))
                }
            } else if !envelope.signature.is_empty() {
                Err(ManyError::unknown("Anonymous requires no signature."))
            } else {
                Ok(())
            }
        }
    }
}
