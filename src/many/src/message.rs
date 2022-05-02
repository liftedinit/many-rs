pub mod error;
pub mod request;
pub mod response;

use std::collections::BTreeMap;

use coset::cbor::value::Value;
use coset::iana::Algorithm;
use coset::CborSerializable;
use coset::CoseKeySet;
use coset::CoseSign1;
use coset::CoseSign1Builder;
use coset::HeaderBuilder;
use coset::Label;
pub use error::ManyError;
pub use request::RequestMessage;
pub use request::RequestMessageBuilder;
pub use response::ResponseMessage;
pub use response::ResponseMessageBuilder;
use serde::Deserialize;
use sha2::Digest;

use crate::cose_helpers::public_key;
use crate::types::identity::cose::{CoseKeyIdentity, CoseKeyIdentitySignature};
use crate::Identity;
use signature::{Signature, Signer, Verifier};
use tracing::error;

pub fn decode_request_from_cose_sign1(sign1: CoseSign1) -> Result<RequestMessage, ManyError> {
    let request = CoseSign1RequestMessage { sign1 };
    let from_id = request.verify().map_err(|e| {
        error!("e {}", e);
        ManyError::could_not_verify_signature()
    })?;

    let payload = request
        .sign1
        .payload
        .ok_or_else(ManyError::empty_envelope)?;
    let message = RequestMessage::from_bytes(&payload).map_err(ManyError::deserialization_error)?;

    // Check the `from` field.
    if from_id != message.from.unwrap_or_default() {
        return Err(ManyError::invalid_from_identity());
    }

    // We don't check the `to` field, leave that to the server itself.
    // Some servers might want to proxy messages that aren't for them, for example, or
    // accept anonymous messages.

    Ok(message)
}

pub fn decode_response_from_cose_sign1(
    sign1: CoseSign1,
    to: Option<Identity>,
) -> Result<ResponseMessage, String> {
    let request = CoseSign1RequestMessage { sign1 };
    let from_id = request.verify()?;

    let payload = request
        .sign1
        .payload
        .ok_or_else(|| "Envelope does not have payload.".to_string())?;
    let message = ResponseMessage::from_bytes(&payload)?;

    // Check the `from` field.
    if from_id != message.from {
        return Err("The message's from field does not match the envelope.".to_string());
    }

    // Check the `to` field to make sure we have the right one.
    if let Some(to_id) = to {
        if to_id != message.to.unwrap_or_default() {
            return Err("The message's to field is not for this server.".to_string());
        }
    }

    Ok(message)
}

fn encode_cose_sign1_from_payload(
    payload: Vec<u8>,
    cose_key: &CoseKeyIdentity,
) -> Result<CoseSign1, String> {
    let mut protected = HeaderBuilder::new()
        .algorithm(Algorithm::EdDSA)
        .key_id(cose_key.identity.to_vec());

    // Add the keyset to the headers.
    if let Some(key) = cose_key.key.as_ref() {
        let mut keyset = CoseKeySet::default();
        let mut key_public = public_key(key)?;
        key_public.key_id = cose_key.identity.to_vec();
        keyset.0.push(key_public);

        protected = protected.text_value(
            "keyset".to_string(),
            Value::Bytes(keyset.to_vec().map_err(|e| e.to_string())?),
        );
    }

    let protected = protected.build();

    let mut cose_builder = CoseSign1Builder::default()
        .protected(protected)
        .payload(payload);

    if cose_key.key.is_some() {
        cose_builder = cose_builder
            .try_create_signature(b"", |msg| {
                cose_key
                    .try_sign(msg)
                    .map(|v| v.as_bytes().to_vec())
                    .map_err(|e| e.to_string())
            })
            .map_err(|e| e)?;
    }
    Ok(cose_builder.build())
}

pub fn encode_cose_sign1_from_response(
    response: ResponseMessage,
    cose_key: &CoseKeyIdentity,
) -> Result<CoseSign1, String> {
    encode_cose_sign1_from_payload(
        response
            .to_bytes()
            .map_err(|e| format!("Could not serialize response: {}", e))?,
        cose_key,
    )
}

pub fn encode_cose_sign1_from_request(
    request: RequestMessage,
    cose_key: &CoseKeyIdentity,
) -> Result<CoseSign1, String> {
    encode_cose_sign1_from_payload(request.to_bytes().unwrap(), cose_key)
}

/// Provide utility functions surrounding request and response messages.
#[derive(Clone, Debug, Default)]
pub(crate) struct CoseSign1RequestMessage {
    pub sign1: CoseSign1,
}

#[derive(Deserialize)]
struct ClientData {
    challenge: String,
    #[allow(dead_code)]
    origin: String,
    r#type: String,
}

impl CoseSign1RequestMessage {
    pub fn get_keyset(&self) -> Option<CoseKeySet> {
        let keyset = self
            .sign1
            .protected
            .header
            .rest
            .iter()
            .find(|(k, _)| k == &Label::Text("keyset".to_string()))?
            .1
            .clone();

        if let Value::Bytes(ref bytes) = keyset {
            CoseKeySet::from_slice(bytes).ok()
        } else {
            None
        }
    }

    pub fn get_public_key_for_identity(&self, id: &Identity) -> Option<CoseKeyIdentity> {
        // Verify the keybytes matches the identity.
        if id.is_anonymous() {
            return None;
        }

        let cose_key = self
            .get_keyset()?
            .0
            .into_iter()
            .find(|key| id.matches_key(Some(key)))?; // TODO: We might want to optimize this for lookup?

        // The hsm: false parameter is not important here. We always perform
        // signature verification on the CPU server-side
        let key = CoseKeyIdentity::from_key(cose_key, false).ok()?;
        if id == &key.identity {
            Some(key)
        } else {
            None
        }
    }

    /// Perform WebAuthn request verification
    ///
    /// This is non-standard COSE
    fn _verify_webauthn(
        &self,
        unprotected: BTreeMap<Label, Value>,
        key: CoseKeyIdentity,
    ) -> Result<(), String> {
        tracing::trace!("We got a WebAuthn request");
        tracing::trace!("Getting `clientData` from unprotected header");
        let client_data = unprotected
            .get(&Label::Text("clientData".to_string()))
            .ok_or("`clientData` entry missing from unprotected header")?
            .as_text()
            .ok_or("`clientData` entry is not Text")?;
        let client_data_json: ClientData =
            serde_json::from_str(client_data).map_err(|e| e.to_string())?;

        if client_data_json.r#type != "webauthn.get" {
            return Err("request type != webauthn.get".to_string());
        }

        tracing::trace!("Getting `authData` from unprotected header");
        let auth_data = unprotected
            .get(&Label::Text("authData".to_string()))
            .ok_or("`authData` entry missing from unprotected header")?
            .as_bytes()
            .ok_or("`authData` entry is not Bytes")?;

        tracing::trace!("Getting `signature` from unprotected header");
        let signature = unprotected
            .get(&Label::Text("signature".to_string()))
            .ok_or("`signature` entry missing from unprotected header")?
            .as_bytes()
            .ok_or("`signature` entry is not Bytes")?;

        let payload = self
            .sign1
            .payload
            .as_ref()
            .ok_or("`payload` entry missing but required")?;

        let payload_sha512 = sha2::Sha512::digest(payload);
        let payload_sha512_base64url =
            base64::encode_config(payload_sha512, base64::URL_SAFE_NO_PAD);

        tracing::trace!("Verifying `challenge`");
        if payload_sha512_base64url != client_data_json.challenge {
            return Err("`challenge` doesn't match".to_string());
        }

        tracing::trace!("Concatenating `authData` and sha256(`clientData`)");
        let mut msg = auth_data.clone();
        msg.extend(sha2::Sha256::digest(client_data));

        let cose_sig =
            CoseKeyIdentitySignature::from_bytes(signature).map_err(|e| e.to_string())?;

        tracing::trace!("Verifying WebAuthn signature");
        key.verify(&msg, &cose_sig).map_err(|e| e.to_string())?;

        tracing::trace!("WebAuthn verifications succedded");
        Ok(())
    }

    /// Perform standard COSE verification
    fn _verify(&self, key: CoseKeyIdentity) -> Result<(), String> {
        self.sign1
            .verify_signature(b"", |sig, content| {
                let sig = CoseKeyIdentitySignature::from_bytes(sig).unwrap();
                key.verify(content, &sig)
            })
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn verify(&self) -> Result<Identity, String> {
        if !self.sign1.protected.header.key_id.is_empty() {
            if let Ok(id) = Identity::from_bytes(&self.sign1.protected.header.key_id) {
                if id.is_anonymous() {
                    return Ok(id);
                }

                let key = self
                    .get_public_key_for_identity(&id)
                    .ok_or_else(|| "Could not find a public key in the envelope".to_string())?;
                let unprotected =
                    BTreeMap::from_iter(self.sign1.unprotected.rest.clone().into_iter());
                if unprotected.contains_key(&Label::Text("webauthn".to_string())) {
                    self._verify_webauthn(unprotected, key)?;
                } else {
                    self._verify(key)?;
                }
                Ok(id)
            } else {
                Err("Invalid (not a MANY identity) key ID".to_string())
            }
        } else {
            if self.sign1.signature.is_empty() {
                return Ok(Identity::anonymous());
            }

            Err("Missing key ID".to_string())
        }
    }
}
