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
    use std::fmt::{Debug, Formatter};

    #[macro_export]
    macro_rules! one_of {
        ( $clsName: expr $(,)? ) => {
            $clsName
        };
        ( $cls: expr, $last: expr $(,)? ) => {
            $crate::verifiers::OneOf::new($cls, $last)
        };
        ( $cls: expr, $last: expr, $($tail: expr),* ) => {
            $crate::verifiers::OneOf::new(
                $crate::verifier::OneOf::new($cls, $last),
                one_of!($($tail),*)
            )
        };
    }
    pub use one_of;

    pub struct OneOf<L: Verifier, R: Verifier>(L, R);

    impl<L: Verifier, R: Verifier> OneOf<L, R> {
        pub fn new(l: L, r: R) -> Self {
            Self(l, r)
        }
    }

    impl<L: Verifier, R: Verifier> Verifier for OneOf<L, R> {
        #[inline]
        fn sign_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
            self.0.sign_1(envelope).or_else(|lerr| {
                self.1.sign_1(envelope).map_err(|rerr| {
                    ManyError::unknown(format!("Could not verify: [{}, {}]", lerr, rerr))
                })
            })
        }
    }

    impl<L: Verifier + Clone, R: Verifier + Clone> Clone for OneOf<L, R> {
        fn clone(&self) -> Self {
            Self(self.0.clone(), self.1.clone())
        }
    }

    impl<L: Verifier + Debug, R: Verifier + Debug> Debug for OneOf<L, R> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            f.debug_tuple("OneOf")
                .field(&self.0)
                .field(&self.0)
                .finish()
        }
    }

    #[derive(Clone, Debug)]
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
