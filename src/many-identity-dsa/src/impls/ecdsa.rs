use coset::cbor::value::Value;
use coset::iana::{Algorithm, Ec2KeyParameter, EllipticCurve, EnumI64, KeyType};
use coset::{CborSerializable, CoseKey, CoseSign1, CoseSign1Builder, Label};
use many_error::ManyError;
use many_identity::cose::add_keyset_header;
use many_identity::{Address, Identity, Verifier};
use p256::pkcs8::FromPrivateKey;
use pkcs8::der::Document;
use sha3::{Digest, Sha3_224};
use signature::{Signature, Signer};
use std::collections::{BTreeMap, BTreeSet};

/// Build an ECDSA CoseKey
///
/// # Arguments
///
/// * `(x, y)` - Public key
/// * `d` - Private key
pub fn ecdsa_cose_key((x, y): (Vec<u8>, Vec<u8>), d: Option<Vec<u8>>) -> CoseKey {
    let mut params: Vec<(Label, Value)> = Vec::from([
        (
            Label::Int(Ec2KeyParameter::Crv as i64),
            Value::from(EllipticCurve::P_256 as u64),
        ),
        (Label::Int(Ec2KeyParameter::X as i64), Value::Bytes(x)),
        (Label::Int(Ec2KeyParameter::Y as i64), Value::Bytes(y)),
    ]);
    let mut key_ops: BTreeSet<coset::KeyOperation> =
        BTreeSet::from([coset::KeyOperation::Assigned(
            coset::iana::KeyOperation::Verify,
        )]);

    if let Some(d) = d {
        params.push((Label::Int(Ec2KeyParameter::D as i64), Value::Bytes(d)));
        key_ops.insert(coset::KeyOperation::Assigned(
            coset::iana::KeyOperation::Sign,
        ));
    }

    // The CoseKeyBuilder is too limited to be used here
    CoseKey {
        kty: coset::KeyType::Assigned(KeyType::EC2),
        alg: Some(coset::Algorithm::Assigned(Algorithm::ES256)),
        key_ops,
        params,
        ..Default::default()
    }
}

pub fn public_key(key: &CoseKey) -> Result<Option<CoseKey>, ManyError> {
    let params = BTreeMap::from_iter(key.params.clone().into_iter());
    match key.alg {
        Some(coset::Algorithm::Assigned(Algorithm::ES256)) => {
            let x = params.get(&Label::Int(Ec2KeyParameter::X.to_i64()));
            let y = params.get(&Label::Int(Ec2KeyParameter::Y.to_i64()));

            if let (Some(x), Some(y)) = (x.cloned(), y.cloned()) {
                let x = x
                    .as_bytes()
                    .cloned()
                    .ok_or_else(|| ManyError::unknown("Could not get ECDSA X parameter"))?;
                let y = y
                    .as_bytes()
                    .cloned()
                    .ok_or_else(|| ManyError::unknown("Could not get ECDSA Y parameter"))?;
                Ok(Some(ecdsa_cose_key((x, y), None)))
            } else {
                Err(ManyError::unknown("Key doesn't have a public key"))
            }
        }
        _ => Ok(None),
    }
}

pub fn address(key: &CoseKey) -> Result<Address, ManyError> {
    let pk = Sha3_224::digest(
        &public_key(key)?
            .ok_or_else(|| ManyError::unknown("Could not load key."))?
            .to_vec()
            .map_err(|e| ManyError::unknown(e.to_string()))?,
    );

    Ok(unsafe { Address::public_key_raw(pk.into()) })
}

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

    if cose_key.kty != coset::KeyType::Assigned(KeyType::EC2) {
        return Err(ManyError::unknown(format!(
            "Wrong key type: {:?}",
            cose_key.kty
        )));
    }

    if cose_key.alg != Some(coset::Algorithm::Assigned(Algorithm::ES256)) {
        return Err(ManyError::unknown(format!(
            "Wrong key algorihm: {:?}",
            cose_key.alg
        )));
    }
    Ok(())
}

#[derive(Clone)]
struct EcDsaIdentityInner {
    address: Address,
    public_key: CoseKey,
    sk: p256::ecdsa::SigningKey,
}

impl EcDsaIdentityInner {
    pub fn from_points(x: Vec<u8>, y: Vec<u8>, d: Option<Vec<u8>>) -> Result<Self, ManyError> {
        let key = ecdsa_cose_key((x, y), d);

        Self::from_key(key)
    }

    pub fn from_key(cose_key: CoseKey) -> Result<Self, ManyError> {
        let public_key =
            public_key(&cose_key)?.ok_or_else(|| ManyError::unknown("Invalid key."))?;
        let address = address(&public_key)?;

        check_key(&cose_key, true, false)?;

        let params = BTreeMap::from_iter(cose_key.params.into_iter());
        let d = params
            .get(&Label::Int(Ec2KeyParameter::D.to_i64()))
            .ok_or_else(|| ManyError::unknown("Could not find the D parameter in key"))?
            .as_bytes()
            .ok_or_else(|| ManyError::unknown("Could not convert the D parameter to bytes"))?
            .as_slice();

        let sk = p256::SecretKey::from_bytes(d)
            .map_err(|e| ManyError::unknown(format!("Invalid EcDSA keypair from bytes: {e}")))?;

        Ok(Self {
            address,
            public_key,
            sk: sk.into(),
        })
    }

    pub(crate) fn try_sign(&self, bytes: &[u8]) -> Result<Vec<u8>, ManyError> {
        self.sk
            .try_sign(bytes)
            .map(|x| x.as_bytes().to_vec())
            .map_err(ManyError::unknown)
    }
}

impl Identity for EcDsaIdentityInner {
    fn address(&self) -> Address {
        self.address
    }

    fn public_key(&self) -> Option<CoseKey> {
        Some(self.public_key.clone())
    }

    fn sign_1(&self, mut envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        // Add the algorithm and key id.
        envelope.protected.header.alg = Some(coset::Algorithm::Assigned(Algorithm::ES256));
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

/// An EcDsa identity that sign messages and include the public key in the
/// protected headers.
#[derive(Clone)]
pub struct EcDsaIdentity(EcDsaIdentityInner);

impl EcDsaIdentity {
    pub fn from_points(x: Vec<u8>, y: Vec<u8>, d: Option<Vec<u8>>) -> Result<Self, ManyError> {
        EcDsaIdentityInner::from_points(x, y, d).map(Self)
    }

    pub fn from_key(key: CoseKey) -> Result<Self, ManyError> {
        EcDsaIdentityInner::from_key(key).map(Self)
    }

    pub fn from_pem<P: AsRef<str>>(pem: P) -> Result<Self, ManyError> {
        let doc = pkcs8::PrivateKeyDocument::from_pem(pem.as_ref()).unwrap();
        let decoded = doc.decode();

        // EcDSA P256 OID
        if decoded.algorithm.oid != pkcs8::ObjectIdentifier::new("1.2.840.10045.2.1") {
            return Err(ManyError::unknown(format!(
                "Invalid OID: {}",
                decoded.algorithm.oid
            )));
        }

        let sk = p256::SecretKey::from_pkcs8_pem(pem.as_ref()).unwrap();
        let pk = sk.public_key();
        let points: p256::EncodedPoint = pk.into();

        let cose_key = ecdsa_cose_key(
            (points.x().unwrap().to_vec(), points.y().unwrap().to_vec()),
            Some(sk.to_bytes().to_vec()),
        );
        Self::from_key(cose_key)
    }

    #[cfg(test)]
    pub(crate) fn try_sign(&self, bytes: &[u8]) -> Result<Vec<u8>, ManyError> {
        self.0.try_sign(bytes)
    }
}

impl Identity for EcDsaIdentity {
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
pub struct EcDsaVerifier {
    address: Address,
    pk: p256::ecdsa::VerifyingKey,
}

impl EcDsaVerifier {
    pub fn from_key(cose_key: &CoseKey) -> Result<Self, ManyError> {
        let public_key =
            public_key(cose_key)?.ok_or_else(|| ManyError::unknown("Key not EcDsa."))?;

        check_key(&public_key, false, true)?;

        let params = BTreeMap::from_iter(cose_key.params.clone().into_iter());
        let x = params
            .get(&Label::Int(Ec2KeyParameter::X.to_i64()))
            .ok_or_else(|| ManyError::unknown("Could not find the X parameter in key"))?
            .as_bytes()
            .ok_or_else(|| ManyError::unknown("Could not convert the X parameter to bytes"))?
            .as_slice();
        let y = params
            .get(&Label::Int(Ec2KeyParameter::Y.to_i64()))
            .ok_or_else(|| ManyError::unknown("Could not find the Y parameter in key"))?
            .as_bytes()
            .ok_or_else(|| ManyError::unknown("Could not convert the Y parameter to bytes"))?
            .as_slice();
        let points = p256::EncodedPoint::from_affine_coordinates(x.into(), y.into(), false);
        let pk = p256::ecdsa::VerifyingKey::from_encoded_point(&points)
            .map_err(|e| ManyError::unknown(format!("Could not create a verifying key: {}", e)))?;

        let address = address(cose_key)?;

        Ok(Self { address, pk })
    }

    pub fn verify_signature(&self, signature: &[u8], data: &[u8]) -> Result<(), ManyError> {
        let signature = p256::ecdsa::Signature::from_der(signature)
            .or_else(|_| p256::ecdsa::Signature::from_bytes(signature))
            .map_err(ManyError::could_not_verify_signature)?;
        signature::Verifier::verify(&self.pk, data, &signature)
            .map_err(ManyError::could_not_verify_signature)
    }
}

impl Verifier for EcDsaVerifier {
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

#[cfg(test)]
pub mod tests {
    use super::*;

    const MSG: &[u8] = b"FOOBAR";

    pub fn ecdsa_256_identity() -> EcDsaIdentity {
        let pem = "-----BEGIN PRIVATE KEY-----\n\
                         MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgNsLo2hVPeUZEOPCw\n\
                         lLQbhLpwjUbt9BHXKCFMY0i+Wm6hRANCAATyM3MzaNX4ELK6bzqgNC/ODvGOUd60\n\
                         7A4yltVQLNKUxtTywYy2MIPV8ls1BlUp40zYmQfxCL3VANvZ62ofaMPv\n\
                         -----END PRIVATE KEY-----";

        EcDsaIdentity::from_pem(pem).unwrap()
    }

    #[test]
    fn ecdsa_256_sign_verify() {
        let id = ecdsa_256_identity();
        let verifier = EcDsaVerifier::from_key(&id.public_key().unwrap()).unwrap();

        let signature = id.try_sign(MSG).unwrap();
        verifier.verify_signature(&signature, MSG).unwrap();
    }

    #[test]
    #[should_panic]
    fn fail_ecdsa_512() {
        let pem = "-----BEGIN PRIVATE KEY-----\n\
                         MIHuAgEAMBAGByqGSM49AgEGBSuBBAAjBIHWMIHTAgEBBEIB2zGGfgHhqK9J8Eug\n\
                         Sb5pnwnRA3OZ5Ks4eXEJJOeqeZu+8vYZbNuK9IY78JcmAI+syc3at1eVPtcAtTUr\n\
                         qSTAkIehgYkDgYYABABVfJDnPyVOY0N1shZaB5kBPM6JcEb3BZRT8MR4qBp0zXwM\n\
                         pyh7pdD9wxqsCYQVxl9FbiJSQZXzZTwmXsmTzO8X5AAS52WLB+7Ch+ddQW5UEqj6\n\
                         Tptw8tbMJhJlD4IH7SDevF+gNetMicMQ1fIFyfCbaK0xxVoLwKJvtp7MIV46IZMC\n\
                         aA==\n\
                         -----END PRIVATE KEY-----";
        EcDsaIdentity::from_pem(pem).unwrap();
    }

    #[test]
    #[should_panic]
    fn fail_ecdsa_384() {
        let pem = "-----BEGIN PRIVATE KEY-----\n\
                         MIG2AgEAMBAGByqGSM49AgEGBSuBBAAiBIGeMIGbAgEBBDAo/RAjCOzB1SklJw3K\n\
                         ASQqyjtuVQv7hruJgoy7EotHqD7kFS8c9dyOuoaNyx0V9HChZANiAAQil9Mt9nV4\n\
                         LDxECgIOQvJJd3UcP1d2rTcBY8XMQDl51gLCvCp9c3v1tz9I/hRCEQcH/d96mNHn\n\
                         SigsOU15Tt1NMHHgrucDBMeDrMZ+uUIDdZbfpvvh0gCtvmvvH5FLs/Y=\n\
                         -----END PRIVATE KEY-----";
        let _ = EcDsaIdentity::from_pem(pem);
    }

    #[test]
    fn from_pem_ecdsa() {
        let id = ecdsa_256_identity();
        assert_eq!(
            id.address(),
            "magcncsncbfmfdvezjmfick47pwgefjnm6zcaghu7ffe3o3qtf"
        );
    }
}
