//! An Identity is a signer that also has an address on the MANY protocol.
use crate::Address;
use coset::{CoseKey, CoseSign1};
use many_error::ManyError;

pub trait Identity: Send {
    fn address(&self) -> Address;
    fn public_key(&self) -> Option<CoseKey>;
    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError>;
}

pub trait Verifier: Send {
    fn verify_1(&self, envelope: &CoseSign1) -> Result<(), ManyError>;
}

#[derive(Debug, Clone)]
pub struct AnonymousIdentity;

impl Identity for AnonymousIdentity {
    fn address(&self) -> Address {
        Address::anonymous()
    }

    fn public_key(&self) -> Option<CoseKey> {
        None
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        // An anonymous envelope has no signature, or special header.
        Ok(envelope)
    }
}

impl Verifier for AnonymousIdentity {
    fn verify_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
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
mod testing {
    use crate::Verifier;

    pub struct AcceptAllVerifier;

    impl Verifier for AcceptAllVerifier {
        fn verify_1(&self, _envelope: &coset::CoseSign1) -> Result<(), many_error::ManyError> {
            Ok(())
        }
    }
}

#[cfg(feature = "testing")]
pub use testing::*;

impl Identity for Box<dyn Identity> {
    fn address(&self) -> Address {
        (&**self).address()
    }

    fn public_key(&self) -> Option<CoseKey> {
        (&**self).public_key()
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        (&**self).sign_1(envelope)
    }
}

impl Verifier for Box<dyn Verifier> {
    fn verify_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
        (&**self).verify_1(envelope)
    }
}

macro_rules! declare_tuple_verifiers {
        ( $name: ident: 0 ) => {
            impl< $name: Verifier > Verifier for ( $name, ) {
                #[inline]
                fn verify_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
                    self.0.verify_1(envelope)
                }
            }
        };

        ( $( $name: ident: $index: tt ),* ) => {
            impl< $( $name: Verifier ),* > Verifier for ( $( $name ),* ) {
                #[inline]
                fn verify_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
                    let mut errs = Vec::new();
                    $(
                        match self. $index . verify_1(envelope) {
                            Ok(_) => return Ok(()),
                            Err(e) => errs.push(e.to_string()),
                        }
                    )*

                    Err(ManyError::could_not_verify_signature(errs.join(", ")))
                }
            }
        };
    }

// 8 outta be enough for everyone (but you can also ((a, b), (c, d), ...) recursively).
declare_tuple_verifiers!(A: 0);
declare_tuple_verifiers!(A: 0, B: 1);
declare_tuple_verifiers!(A: 0, B: 1, C: 2);
declare_tuple_verifiers!(A: 0, B: 1, C: 2, D: 3);
declare_tuple_verifiers!(A: 0, B: 1, C: 2, D: 3, E: 4);
declare_tuple_verifiers!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5);
declare_tuple_verifiers!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6);
declare_tuple_verifiers!(A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7);

pub mod verifiers {
    use crate::{Address, Verifier};
    use coset::CoseSign1;
    use many_error::ManyError;

    #[derive(Clone, Debug)]
    pub struct AnonymousVerifier;

    impl Verifier for AnonymousVerifier {
        fn verify_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
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
