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

#[non_exhaustive]
#[derive(Clone)]
enum CoseKeyImpl {
    #[cfg(feature = "ed25519")]
    Ed25519(ed25519::Ed25519Identity),

    #[cfg(feature = "ecdsa")]
    EcDsa(ecdsa::EcDsaIdentity),

    /// This should never be constructed, but in some cases the other enum
    /// values might not exist and an empty enum is illegal.
    #[allow(unused)]
    Illegal_,
}

impl CoseKeyImpl {
    pub fn from_key(key: &CoseKey) -> Option<Self> {
        #[cfg(feature = "ed25519")]
        if let Ok(i) = ed25519::Ed25519Identity::from_key(key) {
            return Some(Self::Ed25519(i));
        }

        #[cfg(feature = "ecdsa")]
        if let Ok(i) = ecdsa::EcDsaIdentity::from_key(key) {
            return Some(Self::EcDsa(i));
        }

        None
    }

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

            CoseKeyImpl::Illegal_ => unreachable!(),
        }
    }

    pub fn public_key(&self) -> Option<CoseKey> {
        match self {
            #[cfg(feature = "ed25519")]
            CoseKeyImpl::Ed25519(i) => Identity::public_key(i),

            #[cfg(feature = "ecdsa")]
            CoseKeyImpl::EcDsa(i) => Identity::public_key(i),

            CoseKeyImpl::Illegal_ => unreachable!(),
        }
    }

    pub fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        match self {
            #[cfg(feature = "ed25519")]
            CoseKeyImpl::Ed25519(i) => i.sign_1(envelope),

            #[cfg(feature = "ecdsa")]
            CoseKeyImpl::EcDsa(i) => i.sign_1(envelope),

            CoseKeyImpl::Illegal_ => unreachable!(),
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
    pub fn from_key(key: &CoseKey) -> Result<Self, ManyError> {
        Ok(Self {
            inner: CoseKeyImpl::from_key(key)
                .ok_or_else(|| ManyError::unknown("Algorithm unsupported."))?,
        })
    }

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
                return v.verify_1($envelope);
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
    fn verify_1(&self, envelope: &CoseSign1) -> Result<Address, ManyError> {
        let keyid = &envelope.protected.header.key_id;

        // Extract the keyset argument.
        let keyset = keyset_from_cose_sign1(envelope)
            .ok_or_else(|| ManyError::unknown("Could not find keyset in headers."))?;

        let key = keyset
            .0
            .iter()
            .find(|key| key.key_id.eq(keyid))
            .ok_or_else(|| ManyError::unknown("Could not find the key in keyset."))?;

        let address = (|| {
            #[cfg(feature = "ed25519")]
            try_verify!(ed25519::Ed25519Verifier::from_key(key), envelope, "ed25519");

            #[cfg(feature = "ecdsa")]
            try_verify!(ecdsa::EcDsaVerifier::from_key(key), envelope, "ecdsa");

            Err(ManyError::unknown("Algorithm unsupported."))
        })()?;

        Ok(address)
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

#[test]
fn ecdsa_sign_and_verify_request() {
    let cose_key = ecdsa::generate_random_ecdsa_cose_key();
    let key = CoseKeyIdentity::from_key(&cose_key).unwrap();
    let envelope = many_protocol::encode_cose_sign1_from_request(
        many_protocol::RequestMessageBuilder::default()
            .from(key.address())
            .method("req".to_string())
            .build()
            .unwrap(),
        &key,
    )
    .unwrap();

    many_protocol::decode_request_from_cose_sign1(&envelope, &CoseKeyVerifier).unwrap();
}

#[test]
fn sign_and_verify_response() {
    let cose_key = ed25519::generate_random_ed25519_cose_key();
    let key = CoseKeyIdentity::from_key(&cose_key).unwrap();
    let envelope = many_protocol::encode_cose_sign1_from_response(
        many_protocol::ResponseMessageBuilder::default()
            .from(key.address())
            .data(Ok(b"".to_vec()))
            .build()
            .unwrap(),
        &key,
    )
    .unwrap();

    many_protocol::decode_response_from_cose_sign1(&envelope, None, &CoseKeyVerifier).unwrap();
}
