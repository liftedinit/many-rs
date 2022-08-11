use coset::CoseSign1;
use many_error::ManyError;
use many_identity::{Address, Identity, Verifier};
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

mod cose_helpers;
mod impls;

#[cfg(feature = "ed25519")]
pub use impls::ed25519;

pub struct CoseKeyIdentity {
    inner: Box<dyn Identity>,
}

impl Debug for CoseKeyIdentity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CoseKeyIdentity")
            .field(&self.address())
            .finish()
    }
}

impl CoseKeyIdentity {
    pub fn from_pem(pem: impl AsRef<str>) -> Result<Self, String> {
        #[cfg(feature = "ed25519")]
        if let Ok(result) = impls::ed25519::Ed25519Identity::from_pem(&pem) {
            return Ok(Self {
                inner: Box::new(result),
            });
        }

        #[cfg(feature = "ecdsa")]
        impls::ecdsa::EcDsaIdentity::from_pem(&pem).unwrap();
        // if let Ok(result) = impls::ecdsa::EcDsaIdentity::from_pem(&pem) {
        //     return Ok(Self {
        //         inner: Box::new(result),
        //     });
        // }

        Err("Algorithm unsupported.".to_string())
    }
}

impl Identity for CoseKeyIdentity {
    fn address(&self) -> Address {
        self.inner.address()
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        self.inner.sign_1(envelope)
    }
}

pub struct CoseKeyVerifier {
    inner: Box<dyn Verifier>,
}

impl CoseKeyVerifier {}
