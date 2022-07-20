use coset::cbor::value::Value;
use coset::iana::{EnumI64, OkpKeyParameter};
use coset::{Algorithm, CborSerializable, CoseKey, CoseSign1, CoseSign1Builder, KeyType};
use coset::{KeyOperation, Label};
use many_error::ManyError;
use many_identity::{Address, Identity};
use pkcs8::der::Document;
use sha3::Sha3_224;
use signature::Signer;
use std::collections::BTreeMap;

mod cose_helpers;

/// A namespace to keep [many_identity::Address] functions organized here.
pub struct Ed25519Address;

impl Ed25519Address {
    pub fn public_key(key: &CoseKey) -> Result<Address, ManyError> {
        use coset::CborSerializable;
        let pk = Sha3_224::digest(
            &cose_helpers::public_key(key)?
                .to_vec()
                .map_err(|e| e.to_string())?,
        );

        unsafe { Address::public_key(pk.into()) }
    }

    pub fn subresource(key: &CoseKey, subid: SubresourceId) -> Result<Address, ManyError> {
        Self::public_key(key)?.with_subresource_id(subid)
    }
}

struct Ed25519IdentityInner {
    address: Address,
    key: CoseKey,
    key_pair: ed25519_dalek::Keypair,
}

impl Ed25519IdentityInner {
    pub fn from_key(key: CoseKey) -> Result<Self, String> {
        let address = Ed25519Address::public_key(&key);

        // Verify that key is valid Ed25519 (including private key).
        if !key.key_ops.contains(&KeyOperation::Sign) {
            return Err("Key cannot sign".to_string());
        }
        if key.kty != KeyType::Assigned(coset::iana::KeyType::OKP) {
            return Err(format!("Wrong key type: {:?}", key.kty));
        }
        if key.alg != Some(Algorithm::Assigned(coset::iana::Algorithm::EdDSA)) {
            return Err(format!("Wrong key algorihm: {:?}", alg));
        }

        let params = BTreeMap::from_iter(cose_key.params.clone().into_iter());
        let x = params
            .get(&Label::Int(OkpKeyParameter::X.to_i64()))
            .ok_or_else("Could not find the X parameter in key".to_string())?
            .as_bytes()
            .ok_or_else("Could not convert the D parameter to bytes".to_string())?
            .as_slice();
        let d = params
            .get(&Label::Int(OkpKeyParameter::D.to_i64()))
            .ok_or_else("Could not find the D parameter in key".to_string())?
            .as_bytes()
            .ok_or_else("Could not convert the D parameter to bytes".to_string())?
            .as_slice();

        let key_pair = ed25519_dalek::Keypair::from_bytes(&vec![d, x].concat())
            .map_err(format!("Invalid Ed25519 keypair from bytes"))?;

        Ok(Self {
            address,
            key,
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
        envelope.protected.header.alg = Some(Algorithm::Assigned(coset::iana::Algorithm::EdDSA));
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
            .create_signature(&[], |bytes| {
                let kp = &self.key_pair;
                let s = kp.sign(bytes);
                s.to_vec()
            })
            .build())
    }
}

/// An Ed25519 identity that is already shared with the server, and as such
/// does not need to contain the `keyset` headers. Only use this type if you
/// know you don't need the header.
pub struct Ed25519SharedIdentity(Ed25519IdentityInner);

impl Ed25519SharedIdentity {
    pub fn from_key(key: CoseKey) -> Result<Self, String> {
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
    pub fn from_key(key: CoseKey) -> Result<Self, String> {
        Ed25519IdentityInner::from_key(key).map(Self)
    }

    pub fn from_pem<P: AsRef<str>>(pem: P) -> Result<Self, String> {
        let doc = pkcs8::PrivateKeyDocument::from_pem(pem).unwrap();
        let decoded = doc.decode();

        // Ed25519 OID
        if decoded.algorithm.oid != pkcs8::ObjectIdentifier::new("1.3.101.112") {
            return Err(format!("Invalid OID: {}", decoded.algorithm.oid));
        }

        // Remove the 0420 header that's in all private keys in pkcs8 for some reason.
        let secret = ed25519_dalek::SecretKey::from_bytes(&decoded.private_key[2..])
            .map_err(|e| e.to_string())?;
        let public: ed25519_dalek::PublicKey = (&sk).into();
        let keypair: ed25519_dalek::Keypair = ed25519_dalek::Keypair { secret, public };
        let keypair = ed25519_dalek::Keypair::from_bytes(&keypair.to_bytes()).unwrap();

        let cose_key = cose_helpers::eddsa_cose_key(
            keypair.public.to_bytes().to_vec(),
            Some(keypair.secret.to_bytes().to_vec()),
        );
        Self::from_key(cose_key)
    }
}

impl Identity for Ed25519Identity {
    fn address(&self) -> Address {
        self.address
    }

    fn sign_1(&self, mut envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        let mut keyset = coset::CoseKeySet::default();
        let mut key_public = cose_helpers::public_key(key)?;
        key_public.key_id = cose_key.identity.to_vec();
        keyset.0.push(key_public);

        envelope.protected.header.rest.push((
            Label::Text("keyset".to_string()),
            Value::Bytes(keyset.to_vec().map_err(|e| e.to_string())?),
        ));

        self.0.sign_1(envelope)
    }
}
