use coset::cbor::value::{Integer, Value};
use coset::iana::{EnumI64, OkpKeyParameter};
use coset::{CborSerializable, CoseKey, CoseSign1, CoseSign1Builder, Label};
use ed25519_dalek::Keypair;
use many_error::ManyError;
use many_identity::cose::add_keyset_header;
use many_identity::{Address, Identity, Verifier};
use pkcs8::der::Document;
use sha3::{Digest, Sha3_224};
use signature::{Signature, Signer};
use std::collections::BTreeSet;
use std::sync::Arc;

/// Build an EdDSA CoseKey
///
/// # Arguments
///
/// * `x` - Public key
/// * `d` - Private key
fn eddsa_cose_key(x: Vec<u8>, d: Option<Vec<u8>>) -> CoseKey {
    let mut params: Vec<(Label, Value)> = Vec::from([
        (
            Label::Int(OkpKeyParameter::Crv.to_i64()),
            Value::from(coset::iana::EllipticCurve::Ed25519.to_i64()),
        ),
        (Label::Int(OkpKeyParameter::X.to_i64()), Value::Bytes(x)),
    ]);

    let mut key_ops: BTreeSet<coset::KeyOperation> =
        BTreeSet::from([coset::KeyOperation::Assigned(
            coset::iana::KeyOperation::Verify,
        )]);

    if let Some(d) = d {
        params.push((Label::Int(OkpKeyParameter::D as i64), Value::Bytes(d)));
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

pub fn public_key(key: &CoseKey) -> Result<Option<CoseKey>, ManyError> {
    match key.alg {
        Some(coset::Algorithm::Assigned(coset::iana::Algorithm::EdDSA)) => {
            let x = key
                .params
                .iter()
                .find(|(l, _)| l == &Label::Int(OkpKeyParameter::X.to_i64()))
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

/// Verify that key is valid Ed25519.
fn check_key(cose_key: &CoseKey, sign: bool, verify: bool) -> Result<(), ManyError> {
    if sign
        && !cose_key.key_ops.contains(&coset::KeyOperation::Assigned(
            coset::iana::KeyOperation::Sign,
        ))
    {
        return Err(ManyError::unknown("Key cannot sign"));
    }
    if verify
        && !cose_key.key_ops.contains(&coset::KeyOperation::Assigned(
            coset::iana::KeyOperation::Verify,
        ))
    {
        return Err(ManyError::unknown("Key cannot verify"));
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

    if cose_key
        .params
        .iter()
        .find(|(k, _v)| k == &Label::Int(OkpKeyParameter::Crv.to_i64()))
        .map(|(_k, v)| v)
        .ok_or_else(|| ManyError::unknown("Crv parameter not found."))?
        .as_integer()
        .ok_or_else(|| ManyError::unknown("Crv parameter not found."))?
        != Integer::from(coset::iana::EllipticCurve::Ed25519.to_i64())
    {
        Err(ManyError::unknown("Curve unsupported. Expected Ed25519"))
    } else {
        Ok(())
    }
}

fn key_pair(cose_key: &CoseKey) -> Result<Keypair, ManyError> {
    check_key(cose_key, true, false)?;

    let mut maybe_x = None;
    let mut maybe_d = None;
    for (k, v) in cose_key.params.iter() {
        if k == &Label::Int(OkpKeyParameter::X.to_i64()) {
            maybe_x = Some(v);
        } else if k == &Label::Int(OkpKeyParameter::D.to_i64()) {
            maybe_d = Some(v);
        }
    }

    let x = maybe_x
        .ok_or_else(|| ManyError::unknown("Could not find the X parameter in key"))?
        .as_bytes()
        .ok_or_else(|| ManyError::unknown("Could not convert the X parameter to bytes"))?
        .as_slice();
    let d = maybe_d
        .ok_or_else(|| ManyError::unknown("Could not find the D parameter in key"))?
        .as_bytes()
        .ok_or_else(|| ManyError::unknown("Could not convert the D parameter to bytes"))?
        .as_slice();

    Keypair::from_bytes(&vec![d, x].concat())
        .map_err(|e| ManyError::unknown(format!("Invalid Ed25519 keypair from bytes: {e}")))
}

#[derive(Clone)]
struct Ed25519IdentityInner {
    address: Address,
    public_key: CoseKey,
    key_pair: Arc<Keypair>,
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
        let key_pair = Arc::new(key_pair(cose_key)?);
        let address = address(cose_key)?;

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

    fn public_key(&self) -> Option<CoseKey> {
        Some(self.public_key.clone())
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

    fn public_key(&self) -> Option<CoseKey> {
        Some(self.0.public_key.clone())
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        self.0.sign_1(envelope)
    }
}

/// An Ed25519 identity that sign messages and include the public key in the
/// protected headers.
#[derive(Clone)]
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
        let keypair: Keypair = Keypair {
            secret: sk,
            public: pk,
        };
        let keypair = Keypair::from_bytes(&keypair.to_bytes()).unwrap();

        let cose_key = eddsa_cose_key(
            keypair.public.to_bytes().to_vec(),
            Some(keypair.secret.to_bytes().to_vec()),
        );
        Self::from_key(&cose_key)
    }

    pub fn public_key(&self) -> CoseKey {
        self.0.public_key.clone()
    }
}

impl Identity for Ed25519Identity {
    fn address(&self) -> Address {
        self.0.address
    }

    fn public_key(&self) -> Option<CoseKey> {
        Some(self.0.public_key.clone())
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        self.0.sign_1(add_keyset_header(envelope, self)?)
    }
}

#[derive(Clone, Debug)]
pub struct Ed25519Verifier {
    address: Address,
    public_key: ed25519_dalek::PublicKey,
}

impl Ed25519Verifier {
    pub fn verify_signature(&self, signature: &[u8], data: &[u8]) -> Result<(), ManyError> {
        let sig = ed25519::Signature::from_bytes(signature)
            .map_err(|e| ManyError::could_not_verify_signature(e))?;
        signature::Verifier::verify(&self.public_key, data, &sig)
            .map_err(|e| ManyError::could_not_verify_signature(e))
    }
}

impl Ed25519Verifier {
    pub fn from_key(cose_key: &CoseKey) -> Result<Self, ManyError> {
        let public_key =
            public_key(cose_key)?.ok_or_else(|| ManyError::unknown("Key not ed25519."))?;

        check_key(&public_key, false, true)?;

        let mut maybe_x = None;
        for (k, v) in cose_key.params.iter() {
            if k == &Label::Int(OkpKeyParameter::X.to_i64()) {
                maybe_x = Some(v);
            }
        }

        let x = maybe_x
            .ok_or_else(|| ManyError::unknown("Could not find the X parameter in key"))?
            .as_bytes()
            .ok_or_else(|| ManyError::unknown("Could not convert the X parameter to bytes"))?
            .as_slice();
        let public_key = ed25519_dalek::PublicKey::from_bytes(x)
            .map_err(|_| ManyError::unknown("Could not create a public key from X."))?;

        let address = address(cose_key)?;

        Ok(Self {
            address,
            public_key,
        })
    }
}

impl Verifier for Ed25519Verifier {
    fn sign_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
        let address = Address::from_bytes(&envelope.protected.header.key_id)?;
        if self.address.matches(&address) {
            envelope.verify_signature(&[], |signature, msg| self.verify_signature(signature, msg))
        } else {
            Err(ManyError::unknown(format!(
                "Address in envelope does not match expected address. Expected: {}, Actual: {}",
                self.address, address
            )))
        }
    }
}

#[cfg(feature = "testing")]
pub fn generate_random_ed25519_identity() -> Ed25519Identity {
    use rand::rngs::OsRng;

    let mut csprng = OsRng {};
    let keypair: Keypair = Keypair::generate(&mut csprng);

    let cose_key = eddsa_cose_key(
        keypair.public.to_bytes().to_vec(),
        Some(keypair.secret.to_bytes().to_vec()),
    );

    Ed25519Identity::from_key(&cose_key).unwrap()
}
