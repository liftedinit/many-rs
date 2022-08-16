use coset::cbor::value::Value;
use coset::iana::{Ec2KeyParameter, EnumI64};
use coset::{CborSerializable, CoseKey, CoseSign1, CoseSign1Builder, Label};
use many_error::ManyError;
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
            Label::Int(coset::iana::Ec2KeyParameter::Crv as i64),
            Value::from(coset::iana::EllipticCurve::P_256 as u64),
        ),
        (
            Label::Int(coset::iana::Ec2KeyParameter::X as i64),
            Value::Bytes(x),
        ),
        (
            Label::Int(coset::iana::Ec2KeyParameter::Y as i64),
            Value::Bytes(y),
        ),
    ]);
    let mut key_ops: BTreeSet<coset::KeyOperation> =
        BTreeSet::from([coset::KeyOperation::Assigned(
            coset::iana::KeyOperation::Verify,
        )]);

    if let Some(d) = d {
        params.push((
            Label::Int(coset::iana::Ec2KeyParameter::D as i64),
            Value::Bytes(d),
        ));
        key_ops.insert(coset::KeyOperation::Assigned(
            coset::iana::KeyOperation::Sign,
        ));
    }

    // The CoseKeyBuilder is too limited to be used here
    CoseKey {
        kty: coset::KeyType::Assigned(coset::iana::KeyType::EC2),
        alg: Some(coset::Algorithm::Assigned(coset::iana::Algorithm::ES256)),
        key_ops,
        params,
        ..Default::default()
    }
}

fn public_key(key: &CoseKey) -> Result<Option<CoseKey>, ManyError> {
    let params = BTreeMap::from_iter(key.params.clone().into_iter());
    match key.alg {
        Some(coset::Algorithm::Assigned(coset::iana::Algorithm::ES256)) => {
            let x = params.get(&Label::Int(coset::iana::Ec2KeyParameter::X.to_i64()));
            let y = params.get(&Label::Int(coset::iana::Ec2KeyParameter::Y.to_i64()));

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

fn address(key: &CoseKey) -> Result<Address, ManyError> {
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

    if cose_key.kty != coset::KeyType::Assigned(coset::iana::KeyType::EC2) {
        return Err(ManyError::unknown(format!(
            "Wrong key type: {:?}",
            cose_key.kty
        )));
    }

    if cose_key.alg != Some(coset::Algorithm::Assigned(coset::iana::Algorithm::ES256)) {
        return Err(ManyError::unknown(format!(
            "Wrong key algorihm: {:?}",
            cose_key.alg
        )));
    }
    Ok(())
}

struct EcDsaIdentityInner {
    address: Address,
    public_key: CoseKey,
    sk: p256::ecdsa::SigningKey,
}

impl EcDsaIdentityInner {
    pub fn from_points(
        x: impl ToOwned<Owned = Vec<u8>>,
        y: impl ToOwned<Owned = Vec<u8>>,
        d: impl ToOwned<Owned = Option<Vec<u8>>>,
    ) -> Result<Self, ManyError> {
        let key = ecdsa_cose_key((x.to_owned(), y.to_owned()), d.to_owned());

        Self::from_key(key)
    }

    pub fn from_key(cose_key: CoseKey) -> Result<Self, ManyError> {
        let public_key =
            public_key(&cose_key)?.ok_or_else(|| ManyError::unknown("Invalid key."))?;
        let address = address(&public_key)?;

        check_key(&cose_key, true, false)?;

        let params = BTreeMap::from_iter(cose_key.params.clone().into_iter());
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
}

impl Identity for EcDsaIdentityInner {
    fn address(&self) -> Address {
        self.address
    }

    fn sign_1(&self, mut envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        // Add the algorithm and key id.
        envelope.protected.header.alg =
            Some(coset::Algorithm::Assigned(coset::iana::Algorithm::ES256));
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
                let kp = &self.sk;
                kp.try_sign(bytes)
                    .map(|x| x.as_bytes().to_vec())
                    .map_err(ManyError::unknown)
            })?
            .build())
    }
}

/// An EcDsa identity that sign messages and include the public key in the
/// protected headers.
pub struct EcDsaIdentity(EcDsaIdentityInner);

impl EcDsaIdentity {
    pub fn from_points(
        x: impl ToOwned<Owned = Vec<u8>>,
        y: impl ToOwned<Owned = Vec<u8>>,
        d: impl ToOwned<Owned = Option<Vec<u8>>>,
    ) -> Result<Self, ManyError> {
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
}

impl Identity for EcDsaIdentity {
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
}

impl Verifier for EcDsaVerifier {
    fn sign_1(&self, envelope: &CoseSign1) -> Result<(), ManyError> {
        let address = Address::from_bytes(&envelope.protected.header.key_id)?;
        if self.address.matches(&address) {
            envelope.verify_signature(&[], |signature, msg| {
                let signature = p256::ecdsa::Signature::from_der(signature)
                    .or_else(|_| p256::ecdsa::Signature::from_bytes(signature))
                    .map_err(|e| ManyError::could_not_verify_signature(e))?;
                signature::Verifier::verify(&self.pk, msg, &signature)
                    .map_err(|e| ManyError::could_not_verify_signature(e))
            })?;
            Ok(())
        } else {
            Err(ManyError::unknown(format!(
                "Address in envelope does not match expected address. Expected: {}, Actual: {}",
                self.address, address
            )))
        }
    }
}
