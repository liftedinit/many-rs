#![cfg(feature = "identity")]

mod u2fhid;

use crate::challenge::Challenge;
use coset::cbor::value::Value;
use coset::{iana, CborSerializable, CoseKey, CoseSign1, KeyOperation, Label};
use many_error::ManyError;
use many_identity::{Address, Identity};
use many_identity_dsa::ecdsa;
use many_modules::idstore;
use many_protocol::ManyUrl;
use webauthn_authenticator_rs::AuthenticatorBackend;
use webauthn_rs::prelude::Url;
use webauthn_rs_proto::PublicKeyCredentialRequestOptions;
use webauthn_rs_proto::{AllowCredentials, AuthenticatorTransport, UserVerificationPolicy};

const ONE_MINUTE: u32 = 1_000 * 60;

pub struct WebAuthnIdentity {
    address: Address,
    public_key: CoseKey,
    cred_id: idstore::CredentialId,
    origin_url: ManyUrl,
    rp_id: String,
}

impl WebAuthnIdentity {
    pub fn authenticate(
        origin_url: ManyUrl,
        rp_id: String,
        creds: idstore::GetReturns,
    ) -> Result<Self, ManyError> {
        let mut public_key = CoseKey::from_slice(creds.public_key.0.as_slice())
            .map_err(ManyError::deserialization_error)?;
        public_key
            .key_ops
            .insert(KeyOperation::Assigned(iana::KeyOperation::Verify));

        Ok(Self {
            address: ecdsa::address(&public_key)?,
            public_key,
            cred_id: creds.cred_id,
            origin_url,
            rp_id,
        })
    }
}

impl Identity for WebAuthnIdentity {
    fn address(&self) -> Address {
        self.address
    }

    fn public_key(&self) -> Option<CoseKey> {
        Some(self.public_key.clone())
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        let mut envelope = many_identity::cose::add_keyset_header(envelope, self)?;

        envelope
            .protected
            .header
            .rest
            .push((Label::Text("webauthn".to_string()), Value::Bool(true)));
        envelope.protected.header.key_id = self.address.to_vec();

        let challenge: Challenge = (&envelope).try_into()?;
        let challenge = minicbor::to_vec(challenge).map_err(ManyError::serialization_error)?;
        let mut provider = u2fhid::U2FHid::new();

        let public_key = PublicKeyCredentialRequestOptions {
            challenge: challenge.into(),
            timeout: Some(ONE_MINUTE),
            rp_id: self.rp_id.clone(),
            user_verification: UserVerificationPolicy::Preferred,
            allow_credentials: vec![AllowCredentials {
                type_: "public-key".to_string(),
                id: self.cred_id.0.as_slice().to_vec().into(),
                transports: Some(vec![
                    AuthenticatorTransport::Usb,
                    AuthenticatorTransport::Nfc,
                    AuthenticatorTransport::Ble,
                ]),
            }],
            extensions: None,
        };

        let r = provider
            .perform_auth(
                Url::parse(self.origin_url.as_str()).unwrap(),
                public_key,
                ONE_MINUTE,
            )
            .map_err(|e| ManyError::unknown(format!("Webauthn error: {e:?}")))?;
        let response = r.response;

        envelope.unprotected.rest.push((
            Label::Text("authData".to_string()),
            Value::Bytes(response.authenticator_data.0),
        ));
        envelope.unprotected.rest.push((
            Label::Text("clientData".to_string()),
            Value::Text(String::from_utf8(response.client_data_json.0).unwrap()),
        ));
        envelope.unprotected.rest.push((
            Label::Text("signature".to_string()),
            Value::Bytes(response.signature.0),
        ));

        Ok(envelope)
    }
}
