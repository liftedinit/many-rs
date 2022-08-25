//! An Identity is a signer that also has an address on the MANY protocol.
use crate::Address;
use coset::{CoseKey, CoseSign1};
use many_error::ManyError;

/// An Identity is anything that is a unique address and can sign messages.
pub trait Identity: Send {
    /// The address of the identity.
    fn address(&self) -> Address;

    /// Its public key. In some cases, the public key is absent or unknown.
    fn public_key(&self) -> Option<CoseKey>;

    /// Signs an envelope with this identity.
    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError>;
}

/// A Verifier is the other side of the signature. It verifies that an envelope
/// matches its signature, either using the envelope or the message fields.
/// It should also resolve the address used to sign or represent the signer
/// the envelope, and returns it.
pub trait Verifier: Send {
    fn verify_1(&self, envelope: &CoseSign1) -> Result<Address, ManyError>;
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

#[cfg(feature = "testing")]
mod testing {
    use crate::{Address, Verifier};

    /// Accept ALL envelopes, and uses the key id as is to resolve the address.
    /// No verification is made. This should NEVER BE used for production.
    pub struct AcceptAllVerifier;

    impl Verifier for AcceptAllVerifier {
        fn verify_1(&self, envelope: &coset::CoseSign1) -> Result<Address, many_error::ManyError> {
            // Does not verify the signature and key id.
            let kid = &envelope.protected.header.key_id;
            if kid.is_empty() {
                Ok(Address::anonymous())
            } else {
                Address::from_bytes(kid)
            }
        }
    }
}

#[cfg(feature = "testing")]
pub use testing::*;

macro_rules! decl_redirection {
    (
        $(
            fn $name: ident ( &self $(,)? $($argn: ident : $argt: ty),* ) -> $ret: tt $( < $( $lt:tt ),+ > )?
        ),* $(,)?
    ) => {
        $(
        fn $name ( &self, $($argn : $argt),* ) -> $ret $(< $( $lt ),+ >)? {
            (&**self) . $name ( $($argn),* )
        }
        )*
    };
}

macro_rules! decl_identity_impl {
    (
        $(
            impl $(
                < $( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+ >
            )? for $ty: ty
        );+ $(;)?
    ) => {
        $(
        impl $(< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? Identity for $ty {
            decl_redirection!(
                fn address(&self) -> Address,
                fn public_key(&self) -> Option<CoseKey>,
                fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError>,
            );
        }
        )+
    };
}

macro_rules! decl_verifier_impl {
    (
        $(
            impl $(
                < $( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+ >
            )? for $ty: ty
        );+ $(;)?
    ) => {
        $(
        impl $(< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? Verifier for $ty {
            decl_redirection!(
                fn verify_1(&self, envelope: &CoseSign1) -> Result<Address, ManyError>,
            );
        }
        )+
    };
}

decl_identity_impl!(
    impl for Box<dyn Identity>;
    impl<I: Identity> for Box<I>;
    impl<I: Identity + Sync> for std::sync::Arc<I>;
);

decl_verifier_impl!(
    impl for Box<dyn Verifier>;
    impl<I: Verifier> for Box<I>;
    impl<I: Verifier + Sync> for std::sync::Arc<I>;
);

macro_rules! declare_tuple_verifiers {
    ( $name: ident: 0 ) => {
        impl< $name: Verifier > Verifier for ( $name, ) {
            #[inline]
            fn verify_1(&self, envelope: &CoseSign1) -> Result<Address, ManyError> {
                self.0.verify_1(envelope)
            }
        }
    };

    ( $( $name: ident: $index: tt ),* ) => {
        impl< $( $name: Verifier ),* > Verifier for ( $( $name ),* ) {
            #[inline]
            fn verify_1(&self, envelope: &CoseSign1) -> Result<Address, ManyError> {
                let mut errs = Vec::new();
                $(
                    match self. $index . verify_1(envelope) {
                        Ok(a) => return Ok(a),
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
    use tracing::trace;

    #[derive(Clone, Debug)]
    pub struct AnonymousVerifier;

    impl Verifier for AnonymousVerifier {
        fn verify_1(&self, envelope: &CoseSign1) -> Result<Address, ManyError> {
            let kid = &envelope.protected.header.key_id;
            if !kid.is_empty() {
                if Address::from_bytes(kid)?.is_anonymous() {
                    trace!("Anonymous message");
                    Ok(Address::anonymous())
                } else {
                    Err(ManyError::unknown("Anonymous requires no key id."))
                }
            } else if !envelope.signature.is_empty() {
                Err(ManyError::unknown("Anonymous requires no signature."))
            } else {
                trace!("Anonymous message");
                Ok(Address::anonymous())
            }
        }
    }
}
