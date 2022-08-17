use coset::{CoseKey, CoseSign1};
use many_error::ManyError;
use many_identity::{Address, Identity, Verifier};
use std::fmt::{Debug, Formatter};
use tracing::trace;

mod impls;

#[cfg(feature = "ed25519")]
pub use impls::ed25519;

#[cfg(feature = "ecdsa")]
pub use impls::ecdsa;
use many_identity::cose::keyset_from_cose_sign1;

#[derive(Clone)]
#[non_exhaustive]
enum CoseKeyImpl {
    #[cfg(feature = "ed25519")]
    Ed25519(ed25519::Ed25519Identity),

    #[cfg(feature = "ecdsa")]
    EcDsa(ecdsa::EcDsaIdentity),
}

impl CoseKeyImpl {
    pub fn from_pem(pem: &str) -> Option<Self> {
        #[cfg(feature = "ed25519")]
        if let Ok(i) = ed25519::Ed25519Identity::from_pem(pem) {
            return Some(Self::Ed25519(i));
        }

        #[cfg(feature = "ecdsa")]
        if let Ok(i) = ecdsa::EcDsaIdentity::from_pem(pem) {
            return Some(Self::EcDsa(i));
        }

        None
    }

    pub fn address(&self) -> Address {
        match self {
            #[cfg(feature = "ed25519")]
            CoseKeyImpl::Ed25519(i) => i.address(),

            #[cfg(feature = "ecdsa")]
            CoseKeyImpl::EcDsa(i) => i.address(),
        }
    }

    pub fn public_key(&self) -> Option<CoseKey> {
        match self {
            #[cfg(feature = "ed25519")]
            CoseKeyImpl::Ed25519(i) => Identity::public_key(i),

            #[cfg(feature = "ecdsa")]
            CoseKeyImpl::EcDsa(i) => Identity::public_key(i),
        }
    }

    pub fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        match self {
            #[cfg(feature = "ed25519")]
            CoseKeyImpl::Ed25519(i) => i.sign_1(envelope),

            #[cfg(feature = "ecdsa")]
            CoseKeyImpl::EcDsa(i) => i.sign_1(envelope),
        }
    }
}

#[derive(Clone)]
pub struct CoseKeyIdentity {
    inner: CoseKeyImpl,
}

impl Debug for CoseKeyIdentity {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CoseKeyIdentity")
            .field(&self.address())
            .finish()
    }
}

impl CoseKeyIdentity {
    pub fn from_pem(pem: impl AsRef<str>) -> Result<Self, ManyError> {
        Ok(Self {
            inner: CoseKeyImpl::from_pem(pem.as_ref())
                .ok_or_else(|| ManyError::unknown("Algorithm unsupported."))?,
        })
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

#[derive(Clone)]
pub struct CoseKeyVerifier;

impl Verifier for CoseKeyVerifier {
    fn sign_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
        let keyid = &envelope.protected.header.key_id;

        // Extract the keyset argument.
        let keyset = keyset_from_cose_sign1(envelope)
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