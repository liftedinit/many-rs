use crate::cose_helpers::{ecdsa_cose_key, eddsa_cose_key, public_key};
use crate::hsm::{HSMMechanism, HSMMechanismType, HSM};
use crate::Identity;
use coset::cbor::value::Value;
use coset::iana::{self, Ec2KeyParameter, EnumI64, OkpKeyParameter};
use coset::{Algorithm, CoseKey, KeyOperation, Label};
use ed25519_dalek::PublicKey;
use p256::pkcs8::FromPrivateKey;
use pkcs8::der::Document;
use sha2::Digest;
use signature::{Error, Signature, Signer, Verifier};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use std::fmt::{Debug, Formatter};
use tracing::{error, trace};

#[derive(Clone, Eq, PartialEq)]
pub struct CoseKeyIdentitySignature {
    bytes: Vec<u8>,
}

impl Debug for CoseKeyIdentitySignature {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CoseKeyIdentitySignature(0x{})",
            hex::encode(&self.bytes)
        )
    }
}

impl AsRef<[u8]> for CoseKeyIdentitySignature {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

impl Signature for CoseKeyIdentitySignature {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            bytes: bytes.to_vec(),
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CoseKeyIdentity {
    pub identity: Identity,
    pub key: Option<CoseKey>,
    pub hsm: bool,
}

impl Default for CoseKeyIdentity {
    fn default() -> Self {
        Self::anonymous()
    }
}

impl CoseKeyIdentity {
    pub fn anonymous() -> Self {
        Self {
            identity: Identity::anonymous(),
            key: None,
            hsm: false,
        }
    }

    pub(crate) fn from_key(key: CoseKey, hsm: bool) -> Result<Self, String> {
        let identity = Identity::public_key(&key);
        if identity.is_anonymous() {
            Ok(Self {
                identity,
                key: None,
                hsm,
            })
        } else {
            Ok(Self {
                identity,
                key: Some(key),
                hsm,
            })
        }
    }

    pub fn from_hsm(mechanism: HSMMechanismType) -> Result<Self, String> {
        let hsm = HSM::get_instance().map_err(|e| e.to_string())?;
        let (raw_points, _) = hsm.ec_info(mechanism).map_err(|e| e.to_string())?;
        trace!("Creating NIST P-256 SEC1 encoded point");
        let points = p256::EncodedPoint::from_bytes(raw_points).map_err(|e| e.to_string())?;

        let cose_key = ecdsa_cose_key(
            (
                Some(Value::Bytes(points.x().unwrap().to_vec())),
                Some(Value::Bytes(points.y().unwrap().to_vec())),
            ),
            None,
        );
        Self::from_key(cose_key, true)
    }

    pub fn from_pem(pem: &str) -> Result<Self, String> {
        let doc = pkcs8::PrivateKeyDocument::from_pem(pem).unwrap();
        let decoded = doc.decode();

        if decoded.algorithm.oid == pkcs8::ObjectIdentifier::new("1.3.101.112") {
            // Ed25519
            // Remove the 0420 header that's in all private keys in pkcs8 for some reason.
            let sk = ed25519_dalek::SecretKey::from_bytes(&decoded.private_key[2..])
                .map_err(|e| e.to_string())?;
            let pk: PublicKey = (&sk).into();
            let keypair: ed25519_dalek::Keypair = ed25519_dalek::Keypair {
                secret: sk,
                public: pk,
            };
            let keypair = ed25519_dalek::Keypair::from_bytes(&keypair.to_bytes()).unwrap();

            let cose_key = eddsa_cose_key(
                Some(Value::Bytes(keypair.public.to_bytes().to_vec())),
                Some(Value::Bytes(keypair.secret.to_bytes().to_vec())),
            );
            Self::from_key(cose_key, false)
        } else if decoded.algorithm.oid == pkcs8::ObjectIdentifier::new("1.2.840.10045.2.1") {
            // ECDSA
            let sk = p256::SecretKey::from_pkcs8_pem(pem).unwrap();
            let pk = sk.public_key();
            let points: p256::EncodedPoint = pk.into();

            let cose_key = ecdsa_cose_key(
                (
                    Some(Value::Bytes(points.x().unwrap().to_vec())),
                    Some(Value::Bytes(points.y().unwrap().to_vec())),
                ),
                Some(Value::Bytes(sk.to_bytes().to_vec())),
            );
            Self::from_key(cose_key, false)
        } else {
            return Err(format!("Unknown algorithm OID: {}", decoded.algorithm.oid));
        }
    }

    pub fn public_key(&self) -> Option<CoseKey> {
        public_key(self.key.as_ref()?).ok()
    }
}

impl TryFrom<String> for CoseKeyIdentity {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let identity: Identity = Identity::try_from(value).map_err(|e| e.to_string())?;
        if identity.is_anonymous() {
            Ok(Self {
                identity,
                key: None,
                hsm: false,
            })
        } else {
            Err("Identity must be anonymous".to_string())
        }
    }
}

impl AsRef<Identity> for CoseKeyIdentity {
    fn as_ref(&self) -> &Identity {
        &self.identity
    }
}

impl Verifier<CoseKeyIdentitySignature> for CoseKeyIdentity {
    fn verify(&self, msg: &[u8], signature: &CoseKeyIdentitySignature) -> Result<(), Error> {
        if let Some(cose_key) = self.key.as_ref() {
            match cose_key.alg {
                None => Err(Error::new()),
                Some(Algorithm::Assigned(coset::iana::Algorithm::ES256)) => {
                    let params = BTreeMap::from_iter(cose_key.params.clone().into_iter());
                    let x = params
                        .get(&Label::Int(Ec2KeyParameter::X.to_i64()))
                        .ok_or_else(Error::new)?
                        .as_bytes()
                        .ok_or_else(Error::new)?
                        .as_slice();
                    let y = params
                        .get(&Label::Int(Ec2KeyParameter::Y.to_i64()))
                        .ok_or_else(Error::new)?
                        .as_bytes()
                        .ok_or_else(Error::new)?
                        .as_slice();
                    let points =
                        p256::EncodedPoint::from_affine_coordinates(x.into(), y.into(), false);

                    let verify_key =
                        p256::ecdsa::VerifyingKey::from_encoded_point(&points).unwrap();
                    let signature = p256::ecdsa::Signature::from_bytes(&signature.bytes).unwrap();
                    verify_key.verify(msg, &signature).map_err(|e| {
                        error!("Key verify failed: {}", e);
                        Error::new()
                    })
                }
                Some(Algorithm::Assigned(coset::iana::Algorithm::EdDSA)) => {
                    let params = BTreeMap::from_iter(cose_key.params.clone().into_iter());
                    let x = params
                        .get(&Label::Int(OkpKeyParameter::X.to_i64()))
                        .ok_or_else(Error::new)?;

                    let public_key = ed25519_dalek::PublicKey::from_bytes(
                        x.as_bytes().ok_or_else(Error::new)?.as_slice(),
                    )
                    .map_err(|e| {
                        error!("Public key does not deserialize: {}", e);
                        Error::new()
                    })?;
                    public_key
                        .verify_strict(msg, &ed25519::Signature::from_bytes(&signature.bytes)?)
                        .map_err(|e| {
                            error!("Verification failed (ed25519): {}", e);
                            Error::new()
                        })
                }
                // TODO: Raise a "Algorithm not supported" error
                _ => Err(Error::new()),
            }
        } else {
            Err(Error::new())
        }
    }
}

impl Signer<CoseKeyIdentitySignature> for CoseKeyIdentity {
    fn try_sign(&self, msg: &[u8]) -> Result<CoseKeyIdentitySignature, Error> {
        if let Some(cose_key) = self.key.as_ref() {
            match cose_key.alg {
                None => Err(Error::new()),
                Some(Algorithm::Assigned(coset::iana::Algorithm::ES256)) => {
                    if self.hsm {
                        let hsm = HSM::get_instance().map_err(|e| {
                            error!("HSM mutex poisoned {}", e);
                            Error::new()
                        })?;

                        // TODO: This operation should be done on the HSM, but cryptoki doesn't support it yet
                        // See https://github.com/parallaxsecond/rust-cryptoki/issues/88
                        trace!("Digesting message using SHA256 (CPU)");
                        let digest = sha2::Sha256::digest(msg);

                        trace!("Singning message using HSM");
                        let msg_signature = hsm
                            .sign(digest.as_slice(), &HSMMechanism::Ecdsa)
                            .map_err(|e| {
                                error!("Unable to sign message using HSM: {}", e);
                                Error::new()
                            })?;
                        trace!("Message signature is {}", hex::encode(&msg_signature));

                        trace!("Converting message signature to P256 structure");
                        let signature = p256::ecdsa::Signature::try_from(msg_signature.as_slice())
                            .expect("Can't create P256 signature from message signature");

                        CoseKeyIdentitySignature::from_bytes(signature.as_ref())
                    } else {
                        if !cose_key
                            .key_ops
                            .contains(&KeyOperation::Assigned(iana::KeyOperation::Sign))
                        {
                            return Err(Error::new());
                        }

                        let params = BTreeMap::from_iter(cose_key.params.clone().into_iter());
                        let d = params
                            .get(&Label::Int(Ec2KeyParameter::D.to_i64()))
                            .ok_or_else(Error::new)?
                            .as_bytes()
                            .ok_or_else(Error::new)?
                            .as_slice();

                        let secret_key =
                            p256::SecretKey::from_bytes(d).map_err(|_| Error::new())?;
                        let signing_key: p256::ecdsa::SigningKey = secret_key.into();

                        let signature: p256::ecdsa::Signature = signing_key.sign(msg);
                        CoseKeyIdentitySignature::from_bytes(signature.as_ref())
                    }
                }
                Some(Algorithm::Assigned(coset::iana::Algorithm::EdDSA)) => {
                    if !cose_key
                        .key_ops
                        .contains(&KeyOperation::Assigned(iana::KeyOperation::Sign))
                    {
                        return Err(Error::new());
                    }
                    let params = BTreeMap::from_iter(cose_key.params.clone().into_iter());
                    let x = params
                        .get(&Label::Int(OkpKeyParameter::X.to_i64()))
                        .ok_or_else(Error::new)?
                        .as_bytes()
                        .ok_or_else(Error::new)?
                        .as_slice();
                    let d = params
                        .get(&Label::Int(OkpKeyParameter::D.to_i64()))
                        .ok_or_else(Error::new)?
                        .as_bytes()
                        .ok_or_else(Error::new)?
                        .as_slice();

                    let kp = ed25519_dalek::Keypair::from_bytes(&vec![d, x].concat())
                        .map_err(Error::from_source)?;
                    let s = kp.sign(msg);
                    CoseKeyIdentitySignature::from_bytes(&s.to_bytes())
                }
                // TODO: Raise a "Algorithm not supported" error
                _ => Err(Error::new()),
            }
        } else {
            Err(Error::new())
        }
    }
}

#[cfg(test)]
pub mod tests {
    use ed25519_dalek::Keypair;
    use rand_07::rngs::OsRng;

    use super::*;

    // MSG == FOOBAR
    const MSG: &[u8] = &[70, 79, 79, 66, 65, 82];

    pub fn generate_random_eddsa_identity() -> CoseKeyIdentity {
        let mut csprng = OsRng {};
        let keypair: Keypair = Keypair::generate(&mut csprng);

        let cose_key = eddsa_cose_key(
            Some(Value::Bytes(keypair.public.to_bytes().to_vec())),
            Some(Value::Bytes(keypair.secret.to_bytes().to_vec())),
        );

        CoseKeyIdentity::from_key(cose_key, false).unwrap()
    }

    pub fn eddsa_identity() -> CoseKeyIdentity {
        let pem = "-----BEGIN PRIVATE KEY-----\n\
                         MC4CAQAwBQYDK2VwBCIEIHcoTY2RYa48O8ONAgfxEw+15MIyqSat0/QpwA1YxiPD\n\
                         -----END PRIVATE KEY-----";

        CoseKeyIdentity::from_pem(pem).unwrap()
    }

    pub fn ecdsa_256_identity() -> CoseKeyIdentity {
        let pem = "-----BEGIN PRIVATE KEY-----\n\
                         MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgNsLo2hVPeUZEOPCw\n\
                         lLQbhLpwjUbt9BHXKCFMY0i+Wm6hRANCAATyM3MzaNX4ELK6bzqgNC/ODvGOUd60\n\
                         7A4yltVQLNKUxtTywYy2MIPV8ls1BlUp40zYmQfxCL3VANvZ62ofaMPv\n\
                         -----END PRIVATE KEY-----";

        CoseKeyIdentity::from_pem(pem).unwrap()
    }

    #[test]
    fn ecdsa_256_sign_verify() -> Result<(), Error> {
        let id = ecdsa_256_identity();

        let signature = id.sign(MSG);
        id.verify(MSG, &signature)
    }

    #[test]
    fn eddsa_256_sign_verify() -> Result<(), Error> {
        let id = eddsa_identity();

        let signature = id.sign(MSG);
        id.verify(MSG, &signature)
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
        CoseKeyIdentity::from_pem(pem).unwrap();
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
        CoseKeyIdentity::from_pem(pem).unwrap();
    }
}
