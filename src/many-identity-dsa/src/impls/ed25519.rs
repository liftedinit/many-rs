use crate::impls::check_key;
use coset::cbor::value::Value;
use coset::iana::{EnumI64, OkpKeyParameter};
use coset::{CoseKey, CoseSign1, CoseSign1Builder, Label};
use ed25519_dalek::Keypair;
use many_error::ManyError;
use many_identity::cose::add_keyset_header;
use many_identity::{cose, Address, Identity, Verifier};
use pkcs8::der::Document;
use signature::{Signature, Signer};
use std::collections::BTreeSet;

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

fn key_pair(cose_key: &CoseKey) -> Result<Keypair, ManyError> {
    check_key(
        cose_key,
        true,
        false,
        coset::iana::KeyType::OKP,
        coset::iana::Algorithm::EdDSA,
        Some(coset::iana::EllipticCurve::Ed25519),
    )?;

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

struct Ed25519IdentityInner {
    address: Address,
    public_key: CoseKey,
    key_pair: Keypair,
}

impl Ed25519IdentityInner {
    pub fn from_points(x: Vec<u8>, d: Option<Vec<u8>>) -> Result<Self, ManyError> {
        let key = eddsa_cose_key(x, d);

        Self::from_key(&key)
    }

    pub fn from_key(cose_key: &CoseKey) -> Result<Self, ManyError> {
        let public_key = public_key(cose_key)?.ok_or_else(|| ManyError::unknown("Invalid key."))?;
        let key_pair = key_pair(cose_key)?;
        let address = unsafe { cose::address(&public_key)? };

        Ok(Self {
            address,
            public_key,
            key_pair,
        })
    }

    pub(crate) fn try_sign(&self, bytes: &[u8]) -> Result<Vec<u8>, ManyError> {
        self.key_pair
            .try_sign(bytes)
            .map(|x| x.as_bytes().to_vec())
            .map_err(ManyError::unknown)
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
            .try_create_signature(&[], |bytes| self.try_sign(bytes))?
            .build())
    }
}

/// An Ed25519 identity that is already shared with the server, and as such
/// does not need to contain the `keyset` headers. Only use this type if you
/// know you don't need the header in the CoseSign1 envelope.
pub struct Ed25519SharedIdentity(Ed25519IdentityInner);

impl Ed25519SharedIdentity {
    pub fn from_points(x: Vec<u8>, d: Option<Vec<u8>>) -> Result<Self, ManyError> {
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
        self.0.public_key()
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

    #[cfg(test)]
    pub(crate) fn try_sign(&self, bytes: &[u8]) -> Result<Vec<u8>, ManyError> {
        self.0.try_sign(bytes)
    }
}

impl Identity for Ed25519Identity {
    fn address(&self) -> Address {
        self.0.address
    }

    fn public_key(&self) -> Option<CoseKey> {
        self.0.public_key()
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
            .map_err(ManyError::could_not_verify_signature)?;
        signature::Verifier::verify(&self.public_key, data, &sig)
            .map_err(ManyError::could_not_verify_signature)
    }

    pub fn from_key(cose_key: &CoseKey) -> Result<Self, ManyError> {
        let public_key =
            public_key(cose_key)?.ok_or_else(|| ManyError::unknown("Key not ed25519."))?;
        let address = unsafe { cose::address(&public_key)? };

        check_key(
            &public_key,
            false,
            true,
            coset::iana::KeyType::OKP,
            coset::iana::Algorithm::EdDSA,
            Some(coset::iana::EllipticCurve::Ed25519),
        )?;

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

        Ok(Self {
            address,
            public_key,
        })
    }
}

impl Verifier for Ed25519Verifier {
    fn verify_1(&self, envelope: &CoseSign1) -> Result<Address, ManyError> {
        let address = Address::from_bytes(&envelope.protected.header.key_id)?;
        if self.address.matches(&address) {
            envelope
                .verify_signature(&[], |signature, msg| self.verify_signature(signature, msg))?;
            Ok(address)
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

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn eddsa_identity() -> Ed25519Identity {
        let pem = "-----BEGIN PRIVATE KEY-----\n\
                         MC4CAQAwBQYDK2VwBCIEIHcoTY2RYa48O8ONAgfxEw+15MIyqSat0/QpwA1YxiPD\n\
                         -----END PRIVATE KEY-----";

        Ed25519Identity::from_pem(pem).unwrap()
    }

    #[test]
    fn eddsa_256_sign_verify() {
        let id = eddsa_identity();
        let verifier = Ed25519Verifier::from_key(&id.public_key()).unwrap();

        let signature = id.try_sign(b"FOOBAR").unwrap();
        verifier.verify_signature(&signature, b"FOOBAR").unwrap();
    }

    #[test]
    fn from_pem_eddsa() {
        let id = eddsa_identity();
        assert_eq!(
            id.address().to_string(),
            "maffbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wijp"
        );
    }
}
