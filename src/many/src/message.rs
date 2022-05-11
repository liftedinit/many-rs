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
use minicbor::Decode;
pub use request::RequestMessage;
pub use request::RequestMessageBuilder;
pub use response::ResponseMessage;
pub use response::ResponseMessageBuilder;
use serde::Deserialize;
use sha2::Digest;

use crate::cose_helpers::public_key;
use crate::server::ManyUrl;
use crate::server::ALLOWED_URLS;
use crate::types::identity::cose::{CoseKeyIdentity, CoseKeyIdentitySignature};
use crate::Identity;
use signature::{Signature, Signer, Verifier};

#[cfg(test)]
use serde::Serialize;

pub fn decode_request_from_cose_sign1(sign1: CoseSign1) -> Result<RequestMessage, ManyError> {
    let request = CoseSign1RequestMessage { sign1 };
    let from_id = request
        .verify()
        .map_err(ManyError::could_not_verify_signature)?;

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

/// WebAuthn ClientData
#[derive(Deserialize)]
#[cfg_attr(test, derive(Serialize))]
struct ClientData {
    challenge: String,
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
    /// See https://webauthn.guide/#webauthn-api
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

        tracing::trace!("Verifying the webauthn request type");
        if client_data_json.r#type != "webauthn.get" {
            return Err("request type != webauthn.get".to_string());
        }

        tracing::trace!("Verifying origin");
        {
            let origin = ManyUrl::parse(&client_data_json.origin).map_err(|e| e.to_string())?;
            ALLOWED_URLS.with(|urls| {
                if let Some(urls) = urls.get().ok_or("ALLOWED_URLS was not initialized")? {
                    if !urls.contains(&origin) {
                        return Err("Origin not allowed".to_string());
                    }
                }
                Ok(())
            })?;
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

        tracing::trace!("Getting payload");
        let payload = self
            .sign1
            .payload
            .as_ref()
            .ok_or("`payload` entry missing but required")?;

        let payload_sha512 = sha2::Sha512::digest(payload);
        let payload_sha512_base64url = base64::encode(payload_sha512);

        #[derive(Clone, Decode)]
        #[cbor(map)]
        struct Challenge {
            #[cbor(n(0), with = "minicbor::bytes")]
            protected_header: Vec<u8>,

            #[n(1)]
            request_message_sha: String,
        }
        tracing::trace!("Decoding `challenge`");
        let challenge = base64::decode_config(&client_data_json.challenge, base64::URL_SAFE_NO_PAD)
            .map_err(|e| e.to_string())?;
        let challenge: Challenge = minicbor::decode(&challenge).map_err(|e| e.to_string())?;
        tracing::trace!("Verifying `challenge` SHA against payload");
        if payload_sha512_base64url != challenge.request_message_sha {
            return Err("`challenge` SHA doesn't match".to_string());
        }

        tracing::trace!("Decoding ProtectedHeader");
        let protected_header =
            coset::ProtectedHeader::from_cbor_bstr(Value::Bytes(challenge.protected_header))
                .map_err(|e| e.to_string())?;
        tracing::trace!("Verifying protected header against `challenge`");
        if self.sign1.protected != protected_header {
            return Err("Protected header doesn't match `challenge`".to_string());
        }

        tracing::trace!("Concatenating `authData` and sha256(`clientData`)");
        let mut msg = auth_data.clone();
        msg.extend(sha2::Sha256::digest(client_data));
        let cose_sig =
            CoseKeyIdentitySignature::from_bytes(signature).map_err(|e| e.to_string())?;
        tracing::trace!("Verifying WebAuthn signature");
        key.verify(&msg, &cose_sig).map_err(|e| e.to_string())?;

        tracing::trace!("WebAuthn verifications succedded!");
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

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;

    static ENVELOPE: Lazy<CoseSign1> = Lazy::new(|| {
        let cbor = concat!(
            "84589da3012604581d0103db2d266f53339c00571f6c8813d027c7a308ba291a5",
            "b31228cde7e666b6579736574587181a7010202581d0103db2d266f53339c0057",
            "1f6c8813d027c7a308ba291a5b31228cde7e03260481022001215820738cc5654",
            "74defb6af2e7b385461380433cf5663a54eb715ec9e9e04bf295f69225820b709",
            "ed3d1ec57f367f2deb490ff1bdf50992659a4bdfb97e0610a5093786bad1a4686",
            "175746844617461582549960de5880e8c687434170f6476605b8fe4aeb9a28632",
            "c7995cf3ba831d976301000000cf68776562617574686ef5697369676e6174757",
            "2655847304502202d4564f676c44de08d2b81b9d0050e94d2ebd90cf6fb7ee104",
            "013b7965c22616022100e7667d72af46315e258f34600bc13ac1c11efb4be4565",
            "fb50eb742bc96202db36a636c69656e74446174617901c87b226368616c6c656e",
            "6765223a226f6742596e614d424a675259485145443279306d62314d7a6e41425",
            "8483279494539416e78364d4975696b61577a45696a4e352d5a6d746c65584e6c",
            "644668786761634241674a59485145443279306d62314d7a6e414258483279494",
            "539416e78364d4975696b61577a45696a4e352d41795945675149674153465949",
            "484f4d7857564854652d32727935374f4652684f41517a7a315a6a70553633466",
            "579656e67535f4b563970496c676774776e7450523746667a5a5f4c65744a445f",
            "473939516d535a5a704c33376c2d4268436c43546547757445426546683463314",
            "64661484a614e566446565752484d7a517756334e36574652365a33705a4f586c",
            "7a557a46704d55357a6245785462303559526c4e32626b6c3154565a4f5657743",
            "24d466b7653475177576a526c4d574a4e6146427a556d59796132397a51326435",
            "57585a6c4d6b4e6b65464a4355543039222c22636c69656e74457874656e73696",
            "f6e73223a7b7d2c2268617368416c676f726974686d223a225348412d32353622",
            "2c226f726967696e223a2268747470733a2f2f6c6f63616c686f73743a3330303",
            "0222c2274797065223a22776562617574686e2e676574227d584fd92711a40001",
            "0178326d61656235776c6a676e356a74686861616b3470777a636174326174347",
            "06979697869757275777a72656b676e3437717334036b6c65646765722e696e66",
            "6f05c11a627bfe9140"
        );
        CoseSign1::from_slice(&hex::decode(cbor).unwrap()).unwrap()
    });

    fn init_urls() {
        ALLOWED_URLS.with(|f| {
            f.get_or_init(|| Some(vec![ManyUrl::parse("https://localhost:3000").unwrap()]));
        });
    }

    fn tamper_uh_rest(uh_rest: &mut [(Label, Value)], field: String, value: Value) {
        let pos = uh_rest
            .iter()
            .position(|(k, _)| k == &Label::Text(field.clone()))
            .unwrap();
        if let Some((_, v)) = uh_rest.get_mut(pos) {
            *v = value;
        }
    }

    fn get_client_data_json(uh_rest: &[(Label, Value)]) -> ClientData {
        let client_data = uh_rest
            .iter()
            .find(|(k, _)| k == &Label::Text("clientData".to_string()))
            .unwrap()
            .1
            .as_text()
            .unwrap();
        serde_json::from_str(client_data).unwrap()
    }

    #[test]
    fn webauthn_ok() {
        init_urls();
        let request = CoseSign1RequestMessage {
            sign1: ENVELOPE.clone(),
        };
        assert!(request.verify().is_ok());
    }

    #[test]
    fn webauthn_tamper_signature() {
        init_urls();
        let mut envelope = ENVELOPE.clone();
        tamper_uh_rest(
            &mut envelope.unprotected.rest,
            "signature".to_string(),
            Value::Bytes(vec![1, 2, 3]),
        );
        let request = CoseSign1RequestMessage { sign1: envelope };
        assert!(request.verify().is_err());
    }

    #[test]
    fn webauthn_tamper_authdata() {
        init_urls();
        let mut envelope = ENVELOPE.clone();
        tamper_uh_rest(
            &mut envelope.unprotected.rest,
            "authData".to_string(),
            Value::Bytes(vec![1, 2, 3]),
        );
        let request = CoseSign1RequestMessage { sign1: envelope };
        assert!(request.verify().is_err());
    }

    #[test]
    fn webauthn_tamper_clientdata() {
        init_urls();
        let mut envelope = ENVELOPE.clone();
        tamper_uh_rest(
            &mut envelope.unprotected.rest,
            "clientData".to_string(),
            Value::Text("Foobar".to_string()),
        );
        let request = CoseSign1RequestMessage { sign1: envelope };
        assert!(request.verify().is_err());
    }

    #[test]
    fn webauthn_tamper_challenge() {
        init_urls();
        let mut envelope = ENVELOPE.clone();
        let mut client_data_json: ClientData = get_client_data_json(&envelope.unprotected.rest);
        client_data_json.challenge = client_data_json.challenge[1..].to_string();
        tamper_uh_rest(
            &mut envelope.unprotected.rest,
            "clientData".to_string(),
            Value::Text(serde_json::to_string(&client_data_json).unwrap()),
        );
        let request = CoseSign1RequestMessage { sign1: envelope };
        assert!(request.verify().is_err());
    }

    #[test]
    fn webauthn_tamper_type() {
        init_urls();
        let mut envelope = ENVELOPE.clone();
        let mut client_data_json: ClientData = get_client_data_json(&envelope.unprotected.rest);
        client_data_json.r#type = "foobar".to_string();
        tamper_uh_rest(
            &mut envelope.unprotected.rest,
            "clientData".to_string(),
            Value::Text(serde_json::to_string(&client_data_json).unwrap()),
        );
        let request = CoseSign1RequestMessage { sign1: envelope };
        assert!(request.verify().is_err());
    }

    #[test]
    fn webauthn_tamper_origin() {
        init_urls();
        let mut envelope = ENVELOPE.clone();
        let mut client_data_json: ClientData = get_client_data_json(&envelope.unprotected.rest);
        client_data_json.origin = "https://test.com".to_string();
        tamper_uh_rest(
            &mut envelope.unprotected.rest,
            "clientData".to_string(),
            Value::Text(serde_json::to_string(&client_data_json).unwrap()),
        );
        let request = CoseSign1RequestMessage { sign1: envelope };
        assert!(request.verify().is_err());
    }

    #[test]
    fn webauthn_tamper_payload() {
        init_urls();
        let mut envelope = ENVELOPE.clone();
        envelope.payload = Some(vec![1, 2, 3]);
        let request = CoseSign1RequestMessage { sign1: envelope };
        assert!(request.verify().is_err());
    }

    #[test]
    fn webauthn_tamper_protected_header() {
        init_urls();
        let mut envelope = ENVELOPE.clone();
        envelope
            .protected
            .header
            .rest
            .insert(0, (Label::Text("Foo".to_string()), Value::Bool(true)));
        let request = CoseSign1RequestMessage { sign1: envelope };
        assert!(request.verify().is_err());
    }
}
