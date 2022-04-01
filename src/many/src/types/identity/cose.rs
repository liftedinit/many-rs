use crate::types::hsm::{HSMMechanism, HSMMechanismType, HSM_INSTANCE};
use crate::Identity;
use ed25519_dalek::PublicKey;
use minicose::{
    Algorithm, CoseKey, EcDsaCoseKey, EcDsaCoseKeyBuilder, Ed25519CoseKey, Ed25519CoseKeyBuilder,
};
use p256::pkcs8::FromPrivateKey;
use pkcs8::der::Document;
use sha2::Digest;
use signature::{Error, Signature, Signer, Verifier};
use std::convert::TryFrom;
use std::fmt::{Debug, Formatter};
use tracing::trace;

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

#[derive(Clone, Debug, Eq, PartialEq)]
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

    pub fn from_key(key: CoseKey, hsm: bool) -> Result<Self, String> {
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
        let (raw_points, _) = HSM_INSTANCE
            .lock()
            .unwrap()
            .ec_info(mechanism)
            .map_err(|e| e.to_string())?;
        trace!("Creating NIST P-256 SEC1 encoded point");
        let points = p256::EncodedPoint::from_bytes(raw_points).map_err(|e| e.to_string())?;

        let cose_key: CoseKey = EcDsaCoseKeyBuilder::default()
            .x(points.x().unwrap().to_vec())
            .y(points.y().unwrap().to_vec())
            .build()
            .expect("Unable to build EcDsaCoseKey")
            .into();
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

            let cose_key: CoseKey = Ed25519CoseKeyBuilder::default()
                .x(keypair.public.to_bytes().to_vec())
                .d(keypair.secret.to_bytes().to_vec())
                .build()
                .unwrap()
                .into();

            Self::from_key(cose_key, false)
        } else if decoded.algorithm.oid == pkcs8::ObjectIdentifier::new("1.2.840.10045.2.1") {
            // ECDSA
            let sk = p256::SecretKey::from_pkcs8_pem(pem).unwrap();
            let pk = sk.public_key();
            let points: p256::EncodedPoint = pk.into();
            let cose_key: CoseKey = EcDsaCoseKeyBuilder::default()
                .x(points.x().unwrap().to_vec())
                .y(points.y().unwrap().to_vec())
                .d(sk.to_bytes().to_vec())
                .build()
                .unwrap()
                .into();

            Self::from_key(cose_key, false)
        } else {
            return Err(format!("Unknown algorithm OID: {}", decoded.algorithm.oid));
        }
    }

    pub fn public_key(&self) -> Option<CoseKey> {
        self.key.as_ref()?.to_public_key().ok()
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
                Algorithm::None => Err(Error::new()),
                Algorithm::ECDSA => {
                    let key = EcDsaCoseKey::try_from(cose_key.clone()).map_err(|e| {
                        eprintln!("Deserializing ECDSA key failed: {}", e);
                        Error::new()
                    })?;
                    let (x, y) = (key.x.ok_or_else(Error::new)?, key.y.ok_or_else(Error::new)?);
                    let points = p256::EncodedPoint::from_affine_coordinates(
                        x.as_slice().into(),
                        y.as_slice().into(),
                        false,
                    );

                    let verify_key =
                        p256::ecdsa::VerifyingKey::from_encoded_point(&points).unwrap();
                    let signature = p256::ecdsa::Signature::from_bytes(&signature.bytes).unwrap();
                    verify_key.verify(msg, &signature).map_err(|e| {
                        eprintln!("Key verify failed: {}", e);
                        Error::new()
                    })
                }
                Algorithm::EDDSA => {
                    let key = Ed25519CoseKey::try_from(cose_key.clone()).map_err(|e| {
                        eprintln!("Deserializing Ed25519 key failed: {}", e);
                        Error::new()
                    })?;
                    let x = key.x.ok_or_else(Error::new)?;

                    let public_key = ed25519_dalek::PublicKey::from_bytes(&x).map_err(|e| {
                        eprintln!("Public key does not deserialize: {}", e);
                        Error::new()
                    })?;
                    public_key
                        .verify_strict(msg, &ed25519::Signature::from_bytes(&signature.bytes)?)
                        .map_err(|e| {
                            eprintln!("Verification failed (ed25519): {}", e);
                            Error::new()
                        })
                }
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
                Algorithm::None => Err(Error::new()),
                Algorithm::ECDSA => {
                    if self.hsm {
                        let hsm = HSM_INSTANCE.lock().unwrap();

                        // TODO: This operation should be done on the HSM, but cryptoki doesn't support it yet
                        // See https://github.com/parallaxsecond/rust-cryptoki/issues/88
                        trace!("Digesting message using SHA256 (CPU)");
                        let digest = sha2::Sha256::digest(msg);

                        trace!("Singning message using HSM");
                        let msg_signature = hsm
                            .sign(digest.as_slice(), &HSMMechanism::Ecdsa)
                            .map_err(|e| {
                                eprintln!("Unable to sign message using HSM: {}", e);
                                Error::new()
                            })?;
                        trace!("Message signature is {}", hex::encode(&msg_signature));

                        trace!("Converting message signature to P256 structure");
                        let signature = p256::ecdsa::Signature::try_from(msg_signature.as_slice())
                            .expect("Can't create P256 signature from message signature");

                        CoseKeyIdentitySignature::from_bytes(signature.as_ref())
                    } else {
                        let key =
                            EcDsaCoseKey::try_from(cose_key.clone()).map_err(|_| Error::new())?;
                        if !key.can_sign() {
                            return Err(Error::new());
                        }

                        let d = key.d.ok_or_else(Error::new)?;
                        let secret_key =
                            p256::SecretKey::from_bytes(&d).map_err(|_| Error::new())?;
                        let signing_key: p256::ecdsa::SigningKey = secret_key.into();

                        let signature: p256::ecdsa::Signature = signing_key.sign(msg);
                        CoseKeyIdentitySignature::from_bytes(signature.as_ref())
                    }
                }
                Algorithm::EDDSA => {
                    let key =
                        Ed25519CoseKey::try_from(cose_key.clone()).map_err(|_| Error::new())?;
                    if !key.can_sign() {
                        return Err(Error::new());
                    }
                    let (x, d) = (key.x.ok_or_else(Error::new)?, key.d.ok_or_else(Error::new)?);

                    let kp = ed25519_dalek::Keypair::from_bytes(&vec![d, x].concat())
                        .map_err(Error::from_source)?;
                    let s = kp.sign(msg);
                    CoseKeyIdentitySignature::from_bytes(&s.to_bytes())
                }
            }
        } else {
            Err(Error::new())
        }
    }
}
