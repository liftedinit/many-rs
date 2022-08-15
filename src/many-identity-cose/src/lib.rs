use coset::{CborSerializable, CoseKeySet, CoseSign1};
use many_error::ManyError;
use many_identity::{Address, Identity, Verifier};
use std::fmt::{Debug, Formatter};

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
        if let Ok(result) = impls::ecdsa::EcDsaIdentity::from_pem(&pem) {
            return Ok(Self {
                inner: Box::new(result),
            });
        }

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

fn _keyset_from_cose_sign1(envelope: &CoseSign1) -> Option<CoseKeySet> {
    let field = "keyset".to_string();
    envelope
        .protected
        .header
        .rest
        .iter()
        .find(|kv| {
            matches!(
                kv,
                (
                    coset::Label::Text(field),
                    coset::cbor::value::Value::Bytes(_)
                )
            )
        })
        .and_then(|(_, v)| CoseKeySet::from_slice(v.as_bytes().unwrap()).ok())
}

pub struct CoseKeyVerifier;

impl Verifier for CoseKeyVerifier {
    fn sign_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
        let keyid = &envelope.protected.header.key_id;

        // Extract the keyset argument.
        let keyset = _keyset_from_cose_sign1(envelope)
            .ok_or_else(|| ManyError::unknown("Could not find keyset in headers."))?;

        let key = keyset
            .0
            .iter()
            .find(|key| key.key_id.eq(keyid))
            .ok_or_else(|| ManyError::unknown("Could not find the key in keyset."))?;

        #[cfg(feature = "ed25519")]
        if let Ok(v) = impls::ed25519::Ed25519Verifier::from_key(key) {
            return v.sign_1(envelope);
        }

        Err(ManyError::unknown("Algorithm unsupported."))
    }
}
