use coset::{CborSerializable, CoseKey, CoseKeySet, CoseSign1};
use many_error::ManyError;
use many_identity::{Address, Identity, Verifier};
use std::fmt::{Debug, Formatter};
use tracing::trace;

mod impls;

#[cfg(feature = "ed25519")]
pub use impls::ed25519;

#[cfg(feature = "ecdsa")]
pub use impls::ecdsa;

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

macro_rules! try_initialize {
    ($init: expr, $name: literal) => {
        match $init {
            Ok(result) => {
                return Ok(Self {
                    inner: Box::new(result),
                });
            }
            Err(err) => {
                trace!("Initialization error ({}): {}", $name, err)
            }
        }
    };
}

macro_rules! try_verify {
    ($init: expr, $envelope: ident, $name: literal) => {
        match $init {
            Ok(v) => {
                return v.sign_1($envelope);
            }
            Err(err) => {
                trace!("Initialization error ({}): {}", $name, err)
            }
        }
    };
}

impl CoseKeyIdentity {
    pub fn from_pem(pem: impl AsRef<str>) -> Result<Self, String> {
        #[cfg(feature = "ed25519")]
        try_initialize!(ed25519::Ed25519Identity::from_pem(&pem), "Ed25519");
        #[cfg(feature = "ecdsa")]
        try_initialize!(ecdsa::EcDsaIdentity::from_pem(&pem), "EcDSA");

        Err("Algorithm unsupported.".to_string())
    }
}

impl Identity for CoseKeyIdentity {
    fn address(&self) -> Address {
        self.inner.address()
    }

    fn public_key(&self) -> Option<CoseKey> {
        self.inner.public_key()
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        self.inner.sign_1(envelope)
    }
}

fn _keyset_from_cose_sign1(envelope: &CoseSign1) -> Option<CoseKeySet> {
    let keyset = &envelope
        .protected
        .header
        .rest
        .iter()
        .find(|(k, _)| k == &coset::Label::Text("keyset".to_string()))?
        .1;

    let bytes = keyset.as_bytes()?;
    CoseKeySet::from_slice(bytes).ok()
}

#[derive(Clone)]
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
        try_verify!(ed25519::Ed25519Verifier::from_key(key), envelope, "ed25519");

        #[cfg(feature = "ecdsa")]
        try_verify!(ecdsa::EcDsaVerifier::from_key(key), envelope, "ecdsa");

        Err(ManyError::unknown("Algorithm unsupported."))
    }
}

impl Debug for CoseKeyVerifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut x = f.debug_tuple("CoseKeyVerifier");

        #[cfg(feature = "ecdsa")]
        x.field(&"ecdsa");

        #[cfg(feature = "ed25519")]
        x.field(&"ed25519");

        x.finish()
    }
}
