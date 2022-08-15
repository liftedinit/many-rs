use coset::cbor::value::Value;
use coset::iana::{EnumI64, OkpKeyParameter};
use coset::{CborSerializable, CoseKey, CoseSign1, CoseSign1Builder, Label};
use many_error::ManyError;
use many_identity::{Address, Identity, Verifier};
use pkcs8::der::Document;
use sha3::{Digest, Sha3_224};
use signature::{Signature, Signer};
use std::collections::{BTreeMap, BTreeSet};

/// Build an EdDSA CoseKey
///
/// # Arguments
///
/// * `x` - Public key
/// * `d` - Private key
fn eddsa_cose_key(x: Vec<u8>, d: Option<Vec<u8>>) -> CoseKey {
    let mut params: Vec<(Label, Value)> = Vec::from([
        (
            Label::Int(coset::iana::OkpKeyParameter::Crv as i64),
            Value::from(coset::iana::EllipticCurve::Ed25519 as u64),
        ),
        (
            Label::Int(coset::iana::OkpKeyParameter::X as i64),
            Value::Bytes(x),
        ),
    ]);

    let mut key_ops: BTreeSet<coset::KeyOperation> =
        BTreeSet::from([coset::KeyOperation::Assigned(
            coset::iana::KeyOperation::Verify,
        )]);

    if let Some(d) = d {
        params.push((
            Label::Int(coset::iana::OkpKeyParameter::D as i64),
            Value::Bytes(d),
        ));
        key_ops.insert(coset::KeyOperation::Assigned(
            coset::iana::KeyOperation::Sign,
        ));
    }

    // The CoseKeyBuilder is too limited to be used here
    CoseKey {
        kty: coset::KeyType::Assigned(coset::iana::KeyType::OKP),
        alg: Some(coset::Algorithm::Assigned(coset::iana::Algorithm::EdDSA)),
        key_ops,
        params,
        ..Default::default()
    }
}

fn address(key: &CoseKey) -> Result<Address, ManyError> {
    let pk = Sha3_224::digest(
        &public_key(key)?
            .ok_or_else(|| ManyError::unknown("Could not load key."))?
            .to_vec()
            .map_err(|e| ManyError::unknown(e.to_string()))?,
    );

    Ok(unsafe { Address::public_key_raw(pk.into()) })
}

fn public_key(key: &CoseKey) -> Result<Option<CoseKey>, ManyError> {
    match key.alg {
        Some(coset::Algorithm::Assigned(coset::iana::Algorithm::EdDSA)) => {
            let x = key
                .params
                .iter()
                .find(|(l, _)| l == &Label::Int(coset::iana::OkpKeyParameter::X.to_i64()))
                .map(|(_, v)| v);

            if let Some(x) = x {
                let x = x
                    .as_bytes()
                    .cloned()
                    .ok_or_else(|| ManyError::unknown("Could not get EdDSA X parameter"))?;
                Ok(Some(eddsa_cose_key(x, None)))
            } else {
                Err(ManyError::unknown("Key doesn't have a public key"))
            }
        }
        _ => Ok(None),
    }
}

struct Ed25519IdentityInner {
    address: Address,
    public_key: CoseKey,
    key_pair: ed25519_dalek::Keypair,
}

impl Ed25519IdentityInner {
    pub fn from_points(
        x: impl ToOwned<Owned = Vec<u8>>,
        d: impl ToOwned<Owned = Option<Vec<u8>>>,
    ) -> Result<Self, ManyError> {
        let key = eddsa_cose_key(x.to_owned(), d.to_owned());

        Self::from_key(&key)
    }

    pub fn from_key(cose_key: &CoseKey) -> Result<Self, ManyError> {
        let public_key = public_key(cose_key)?.ok_or_else(|| ManyError::unknown("Invalid key."))?;
        let address = address(&public_key)?;

        // Verify that key is valid Ed25519 (including private key).
        if !cose_key.key_ops.contains(&coset::KeyOperation::Assigned(
            coset::iana::KeyOperation::Sign,
        )) {
            return Err(ManyError::unknown("Key cannot sign"));
        }
        if cose_key.kty != coset::KeyType::Assigned(coset::iana::KeyType::OKP) {
            return Err(ManyError::unknown(format!(
                "Wrong key type: {:?}",
                cose_key.kty
            )));
        }
        if cose_key.alg != Some(coset::Algorithm::Assigned(coset::iana::Algorithm::EdDSA)) {
            return Err(ManyError::unknown(format!(
                "Wrong key algorihm: {:?}",
                cose_key.alg
            )));
        }

        let params = BTreeMap::from_iter(cose_key.params.clone().into_iter());
        let x = params
            .get(&Label::Int(OkpKeyParameter::X.to_i64()))
            .ok_or_else(|| ManyError::unknown("Could not find the X parameter in key"))?
            .as_bytes()
            .ok_or_else(|| ManyError::unknown("Could not convert the D parameter to bytes"))?
            .as_slice();
        let d = params
            .get(&Label::Int(OkpKeyParameter::D.to_i64()))
            .ok_or_else(|| ManyError::unknown("Could not find the D parameter in key"))?
            .as_bytes()
            .ok_or_else(|| ManyError::unknown("Could not convert the D parameter to bytes"))?
            .as_slice();

        let key_pair = ed25519_dalek::Keypair::from_bytes(&vec![d, x].concat())
            .map_err(|e| ManyError::unknown(format!("Invalid Ed25519 keypair from bytes: {e}")))?;

        Ok(Self {
            address,
            public_key,
            key_pair,
        })
    }
}

impl Identity for Ed25519IdentityInner {
    fn address(&self) -> Address {
        self.address
    }

    fn sign_1(&self, mut envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        // Add the algorithm and key id.
        envelope.protected.header.alg =
            Some(coset::Algorithm::Assigned(coset::iana::Algorithm::EdDSA));
        envelope.protected.header.key_id = self.address.to_vec();

        let builder = CoseSign1Builder::new()
            .protected(envelope.protected.header)
            .unprotected(envelope.unprotected);

        let builder = if let Some(payload) = envelope.payload {
            builder.payload(payload)
        } else {
            builder
        };

        Ok(builder
            .try_create_signature(&[], |bytes| {
                let kp = &self.key_pair;
                kp.try_sign(bytes)
                    .map(|x| x.as_bytes().to_vec())
                    .map_err(ManyError::unknown)
            })?
            .build())
    }
}

/// An Ed25519 identity that is already shared with the server, and as such
/// does not need to contain the `keyset` headers. Only use this type if you
/// know you don't need the header in the CoseSign1 envelope.
pub struct Ed25519SharedIdentity(Ed25519IdentityInner);

impl Ed25519SharedIdentity {
    pub fn from_points(
        x: impl ToOwned<Owned = Vec<u8>>,
        d: impl ToOwned<Owned = Option<Vec<u8>>>,
    ) -> Result<Self, ManyError> {
        Ed25519IdentityInner::from_points(x, d).map(Self)
    }

    pub fn from_key(key: &CoseKey) -> Result<Self, ManyError> {
        Ed25519IdentityInner::from_key(key).map(Self)
    }
}

impl Identity for Ed25519SharedIdentity {
    fn address(&self) -> Address {
        self.0.address()
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        self.0.sign_1(envelope)
    }
}

/// An Ed25519 identity that sign messages and include the public key in the
/// protected headers.
pub struct Ed25519Identity(Ed25519IdentityInner);

impl Ed25519Identity {
    pub fn from_key(key: &CoseKey) -> Result<Self, ManyError> {
        Ed25519IdentityInner::from_key(key).map(Self)
    }

    pub fn from_pem<P: AsRef<str>>(pem: P) -> Result<Self, ManyError> {
        let doc = pkcs8::PrivateKeyDocument::from_pem(pem.as_ref()).unwrap();
        let decoded = doc.decode();

        // Ed25519 OID
        if decoded.algorithm.oid != pkcs8::ObjectIdentifier::new("1.3.101.112") {
            return Err(ManyError::unknown(format!(
                "Invalid OID: {}",
                decoded.algorithm.oid
            )));
        }

        // Remove the 0420 header that's in all private keys in pkcs8 for some reason.
        let sk = ed25519_dalek::SecretKey::from_bytes(&decoded.private_key[2..])
            .map_err(ManyError::unknown)?;
        let pk: ed25519_dalek::PublicKey = (&sk).into();
        let keypair: ed25519_dalek::Keypair = ed25519_dalek::Keypair {
            secret: sk,
            public: pk,
        };
        let keypair = ed25519_dalek::Keypair::from_bytes(&keypair.to_bytes()).unwrap();

        let cose_key = eddsa_cose_key(
            keypair.public.to_bytes().to_vec(),
            Some(keypair.secret.to_bytes().to_vec()),
        );
        Self::from_key(&cose_key)
    }
}

impl Identity for Ed25519Identity {
    fn address(&self) -> Address {
        self.0.address
    }

    fn sign_1(&self, mut envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        let mut keyset = coset::CoseKeySet::default();
        let mut key_public = self.0.public_key.clone();
        key_public.key_id = self.address().to_vec();
        keyset.0.push(key_public);

        envelope.protected.header.rest.push((
            Label::Text("keyset".to_string()),
            Value::Bytes(keyset.to_vec().map_err(|e| ManyError::unknown(e))?),
        ));

        self.0.sign_1(envelope)
    }
}

pub struct Ed25519Verifier {
    key: CoseKey,
}

impl Ed25519Verifier {
    pub fn from_key(key: &CoseKey) -> Result<Self, ManyError> {
        match public_key(key) {
            Ok(Some(key)) => Ok(Self { key }),
            Ok(None) => Err(ManyError::unknown("Not an Ed25519 key.")),
            Err(e) => Err(e),
        }
    }
}

impl Verifier for Ed25519Verifier {
    fn sign_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
        todo!()
    }
}
