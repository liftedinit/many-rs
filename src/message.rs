pub mod error;
pub mod request;
pub mod response;

pub use error::OmniError;
pub use request::RequestMessage;
pub use request::RequestMessageBuilder;
pub use response::ResponseMessage;
pub use response::ResponseMessageBuilder;

use crate::identity::cose::{CoseKeyIdentity, CoseKeyIdentitySignature};
use crate::Identity;
use minicose::exports::ciborium::value::Value;
use minicose::{
    Algorithm, CoseKeySet, CoseSign1, CoseSign1Builder, HeadersFields, ProtectedHeaders,
};
use signature::{Signature, Signer, Verifier};

pub fn decode_request_from_cose_sign1(sign1: CoseSign1) -> Result<RequestMessage, OmniError> {
    let request = CoseSign1RequestMessage { sign1 };
    let from_id = request.verify().map_err(|e| {
        eprintln!("e {}", e);
        OmniError::could_not_verify_signature()
    })?;

    let payload = request
        .sign1
        .payload
        .ok_or_else(OmniError::empty_envelope)?;
    let message =
        RequestMessage::from_bytes(&payload).map_err(OmniError::deserialization_error)?;

    // Check the `from` field.
    if from_id != message.from.unwrap_or_default() {
        return Err(OmniError::invalid_from_identity());
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
        .ok_or("Envelope does not have payload.".to_string())?;
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
    let mut protected: ProtectedHeaders = ProtectedHeaders::default();

    protected
        .set(HeadersFields::Alg as i128, Algorithm::EDDSA as i128)
        .set(HeadersFields::Kid as i128, cose_key.identity.to_vec());

    // Add the keyset to the headers.
    if let Some(key) = cose_key.key.as_ref() {
        let mut keyset = CoseKeySet::default();
        let mut key_public = key.to_public_key()?;
        key_public.kid = Some(cose_key.identity.to_vec());
        keyset.insert(key_public);

        let ks_bytes = keyset.to_bytes().map_err(|e| e).unwrap();
        protected.set("keyset".to_string(), ks_bytes);
    }

    let mut cose: CoseSign1 = CoseSign1Builder::default()
        .protected(protected)
        .payload(payload)
        .build()
        .unwrap();

    if cose_key.key.is_some() {
        cose.sign_with(|bytes| {
            cose_key
                .try_sign(bytes)
                .map(|v| v.as_bytes().to_vec())
                .map_err(|e| e.to_string())
        })
        .map_err(|e| e.to_string())?;
    }
    Ok(cose)
}

pub fn encode_cose_sign1_from_response(
    response: ResponseMessage,
    cose_key: &CoseKeyIdentity,
) -> Result<CoseSign1, String> {
    encode_cose_sign1_from_payload(response.to_bytes().unwrap(), cose_key)
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

impl CoseSign1RequestMessage {
    pub fn get_keyset(&self) -> Option<CoseKeySet> {
        let keyset = self.sign1.protected.get("keyset".to_string())?;

        if let Value::Bytes(ref bytes) = keyset {
            CoseKeySet::from_bytes(bytes).ok()
        } else {
            None
        }
    }

    pub fn get_public_key_for_identity(&self, id: &Identity) -> Option<CoseKeyIdentity> {
        // Verify the keybytes matches the identity.
        if id.is_anonymous() {
            return None;
        }

        // Find the key_bytes.
        let cose_key = self.get_keyset()?.get_kid(&id.to_vec()).cloned()?;
        let key = CoseKeyIdentity::from_key(cose_key).ok()?;
        if id == &key.identity {
            Some(key)
        } else {
            None
        }
    }

    pub fn verify(&self) -> Result<Identity, String> {
        if let Some(kid) = self.sign1.protected.kid() {
            if let Ok(id) = Identity::from_bytes(kid) {
                if id.is_anonymous() {
                    return Ok(id);
                }

                self.get_public_key_for_identity(&id)
                    .ok_or("Could not find a public key in the envelope".to_string())
                    .and_then(|key| {
                        self.sign1
                            .verify_with(|content, sig| {
                                let sig = CoseKeyIdentitySignature::from_bytes(sig).unwrap();
                                let result = key.verify(content, &sig);
                                match result {
                                    Ok(()) => true,
                                    Err(e) => {
                                        eprintln!("Error from verify: {}", e);
                                        false
                                    }
                                }
                            })
                            .map_err(|e| e.to_string())
                    })
                    .and_then(|valid| {
                        if !valid {
                            Err("Envelope does not verify.".to_string())
                        } else {
                            Ok(id)
                        }
                    })
            } else {
                Err("Invalid (not an OMNI identity) key ID".to_string())
            }
        } else {
            Err("Missing key ID".to_string())
        }
    }
}
