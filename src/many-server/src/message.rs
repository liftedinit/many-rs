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
pub use many_error::ManyError;
use many_identity::cose_helpers::public_key;
use minicbor::Decode;
pub use request::RequestMessage;
pub use request::RequestMessageBuilder;
pub use response::ResponseMessage;
pub use response::ResponseMessageBuilder;
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::server::ManyUrl;
use crate::Address;
use many_identity::{CoseKeyIdentity, CoseKeyIdentitySignature};
use signature::{Signature, Signer, Verifier};

pub fn decode_request_from_cose_sign1(
    sign1: CoseSign1,
    allowed_origins: Option<Vec<ManyUrl>>,
) -> Result<RequestMessage, ManyError> {
    let request = CoseSign1RequestMessage { sign1 };
    let from_id = request
        .verify(allowed_origins)
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
    to: Option<Address>,
) -> Result<ResponseMessage, String> {
    let request = CoseSign1RequestMessage { sign1 };
    let from_id = request.verify(None)?;

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
        cose_builder = cose_builder.try_create_signature(b"", |msg| {
            cose_key
                .try_sign(msg)
                .map(|v| v.as_bytes().to_vec())
                .map_err(|e| e.to_string())
        })?;
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
#[derive(Deserialize, Serialize)]
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

    pub fn get_public_key_for_identity(&self, id: &Address) -> Option<CoseKeyIdentity> {
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
        allowed_origins: Option<Vec<ManyUrl>>,
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
        let origin = ManyUrl::parse(&client_data_json.origin).map_err(|e| e.to_string())?;
        if let Some(urls) = allowed_origins {
            if !urls.contains(&origin) {
                return Err("Origin not allowed".to_string());
            }
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

    pub fn verify(&self, allowed_origins: Option<Vec<ManyUrl>>) -> Result<Address, String> {
        if !self.sign1.protected.header.key_id.is_empty() {
            if let Ok(id) = Address::from_bytes(&self.sign1.protected.header.key_id) {
                if id.is_anonymous() {
                    return Ok(id);
                }

                let key = self
                    .get_public_key_for_identity(&id)
                    .ok_or_else(|| "Could not find a public key in the envelope".to_string())?;
                let protected =
                    BTreeMap::from_iter(self.sign1.protected.header.rest.clone().into_iter());
                if protected.contains_key(&Label::Text("webauthn".to_string())) {
                    let unprotected =
                        BTreeMap::from_iter(self.sign1.unprotected.rest.clone().into_iter());
                    self._verify_webauthn(unprotected, key, allowed_origins)?;
                } else {
                    self._verify(key)?;
                }
                Ok(id)
            } else {
                Err("Invalid (not a MANY identity) key ID".to_string())
            }
        } else {
            if self.sign1.signature.is_empty() {
                return Ok(Address::anonymous());
            }

            Err("Missing key ID".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;

    // A real CBOR WebAuthn CoseSign1 request
    static ENVELOPE: Lazy<CoseSign1> = Lazy::new(|| {
        let cbor = concat!(
            "8458a7a4012604581d01bed012259e9db16529f10752a99e66c0738c5bcbc008273a4fdf5aaf666b",
            "6579736574587181a7010202581d01bed012259e9db16529f10752a99e66c0738c5bcbc008273a4f",
            "df5aaf032604810220012158204a06f7487abdf4e1629bff173189a2765fee4b9e26d2a75da79211",
            "ef21d5c4ee2258205a7b35acc5df4ad7a389e484c000e869cfe138df8c03160e6bae33ff84ecb6aa",
            "68776562617574686ef5a3686175746844617461582549960de5880e8c687434170f6476605b8fe4",
            "aeb9a28632c7995cf3ba831d97630100000001697369676e61747572655847304502201c0b14fe9a",
            "368218d9dfea93e8798c0ebf9196e97304ed4413ea76df0747d9d102210081c3524dab1730efe281",
            "bc45a0add9a43eee20a5c866ef87df036812d979f7a76a636c69656e74446174617901d67b226368",
            "616c6c656e6765223a226f674259703651424a6752594851472d3042496c6e7032785a536e784231",
            "4b706e6d624163347862793841494a7a7050333171765a6d746c65584e6c64466878676163424167",
            "4a594851472d3042496c6e7032785a536e7842314b706e6d624163347862793841494a7a70503331",
            "717641795945675149674153465949456f4739306836766654685970765f467a474a6f6e5a66376b",
            "75654a744b6e586165534565386831635475496c6767576e7331724d58665374656a696553457741",
            "446f61635f684f4e2d4d4178594f6136347a5f3454737471706f6432566959585630614737314158",
            "685959586f325448687362327453526a5671516c6c4e4e56424656574e715a456c52536a41346557",
            "4e4c65544e7a643055345153747a566e527157556435536b64744f444a50546b5a6d64556f726155",
            "7772526e465152474e6c5130646e4c324e4565556c77596d5677556d564963555644534863395051",
            "222c22636c69656e74457874656e73696f6e73223a7b7d2c2268617368416c676f726974686d223a",
            "225348412d323536222c226f726967696e223a2268747470733a2f2f6c6f63616c686f73743a3330",
            "3030222c2274797065223a22776562617574686e2e676574227d59011dd92711a500010178326d61",
            "67376e6165726674326f33637a6a6a36656476666b6d366d336168686463337a706161716a7a326a",
            "377076766c796635036d696473746f72652e73746f72650458c9a30078326d6167376e6165726674",
            "326f33637a6a6a36656476666b6d366d336168686463337a706161716a7a326a377076766c796635",
            "0158406c9f928914c639f1e00cc28517f7c574271adaf5c0c399dec223d4529f8bb653d9064723f2",
            "73194dc9b87535044fea436079569cd2c348756a619e56f1bd6e0b02584da5010203262001215820",
            "4a06f7487abdf4e1629bff173189a2765fee4b9e26d2a75da79211ef21d5c4ee2258205a7b35acc5",
            "df4ad7a389e484c000e869cfe138df8c03160e6bae33ff84ecb6aa05c11a62b4afe440"
        );
        CoseSign1::from_slice(&hex::decode(cbor).unwrap()).unwrap()
    });

    enum UnprotectedHeaderFieldType {
        Rest { field: String, value: Value },

        // Special use-case to modify the ClientData JSON directly
        ClientData(ClientDataFieldType),
    }

    enum ClientDataFieldType {
        Challenge,
        Origin(String),
        Type(String),
    }

    enum Cose1FieldType {
        Protected { field: String, value: Value },
        Unprotected(UnprotectedHeaderFieldType),
        Payload(Vec<u8>),
    }

    fn get_tampered_request(field_type: Cose1FieldType) -> CoseSign1RequestMessage {
        let mut envelope = ENVELOPE.clone();
        match field_type {
            Cose1FieldType::Protected { field, value } => {
                // Remove the `webauthn` flag from the protected header
                if field == "webauthn" {
                    let pos = envelope
                        .protected
                        .header
                        .rest
                        .iter()
                        .position(|(k, _)| k == &Label::Text(field.clone()))
                        .unwrap();
                    envelope.protected.header.rest.remove(pos);
                }
                // Insert a new field in the protected header
                else {
                    envelope
                        .protected
                        .header
                        .rest
                        .insert(0, (Label::Text(field), value));
                }
            }
            Cose1FieldType::Unprotected(field_type) => match field_type {
                // Find the matching Label in the rest field and change its value
                UnprotectedHeaderFieldType::Rest { field, value } => {
                    let pos = envelope
                        .unprotected
                        .rest
                        .iter()
                        .position(|(k, _)| k == &Label::Text(field.clone()))
                        .unwrap();
                    if let Some((_, v)) = envelope.unprotected.rest.get_mut(pos) {
                        *v = value;
                    }
                }
                // Find the `clientData` entry in the `rest` field, parse the
                // JSON, change the given attribute, and change the `clientData`
                // entry with the modified version
                UnprotectedHeaderFieldType::ClientData(field_type) => {
                    let client_data = envelope
                        .unprotected
                        .rest
                        .iter()
                        .find(|(k, _)| k == &Label::Text("clientData".to_string()))
                        .unwrap()
                        .1
                        .as_text()
                        .unwrap();
                    let mut client_data_json: ClientData =
                        serde_json::from_str(client_data).unwrap();
                    match field_type {
                        ClientDataFieldType::Challenge => {
                            // Change a char in the payload portion of the challenge
                            client_data_json.challenge.replace_range(310..311, "x");
                        }
                        ClientDataFieldType::Origin(value) => {
                            client_data_json.origin = value;
                        }
                        ClientDataFieldType::Type(value) => {
                            client_data_json.r#type = value;
                        }
                    }
                    return get_tampered_request(Cose1FieldType::Unprotected(
                        UnprotectedHeaderFieldType::Rest {
                            field: "clientData".to_string(),
                            value: Value::Text(serde_json::to_string(&client_data_json).unwrap()),
                        },
                    ));
                }
            },
            Cose1FieldType::Payload(value) => {
                envelope.payload = Some(value);
            }
        }
        CoseSign1RequestMessage { sign1: envelope }
    }

    fn run_test<T>(test: T)
    where
        T: FnOnce() + std::panic::UnwindSafe,
    {
        let result = std::panic::catch_unwind(test);

        assert!(result.is_ok())
    }

    #[test]
    fn webauthn_ok() {
        run_test(|| {
            assert!(CoseSign1RequestMessage {
                sign1: ENVELOPE.clone()
            }
            .verify(None)
            .is_ok())
        });
    }

    #[test]
    fn webauthn_tamper_signature() {
        run_test(|| {
            let request = get_tampered_request(Cose1FieldType::Unprotected(
                UnprotectedHeaderFieldType::Rest {
                    field: "signature".to_string(),
                    value: Value::Bytes(vec![1, 2, 3]),
                },
            ))
            .verify(None);
            assert!(request.is_err());
            assert_eq!(request.unwrap_err(), "signature error");
        });
    }

    #[test]
    fn webauthn_tamper_authdata() {
        run_test(|| {
            let request = get_tampered_request(Cose1FieldType::Unprotected(
                UnprotectedHeaderFieldType::Rest {
                    field: "authData".to_string(),
                    value: Value::Bytes(vec![1, 2, 3]),
                },
            ))
            .verify(None);
            assert!(request.is_err());
            assert_eq!(request.unwrap_err(), "signature error");
        });
    }

    #[test]
    fn webauthn_tamper_clientdata() {
        run_test(|| {
            let request = get_tampered_request(Cose1FieldType::Unprotected(
                UnprotectedHeaderFieldType::Rest {
                    field: "clientData".to_string(),
                    value: Value::Bool(false),
                },
            ))
            .verify(None);
            assert!(request.is_err());
            assert_eq!(request.unwrap_err(), "`clientData` entry is not Text");
        });
    }

    #[test]
    fn webauthn_tamper_challenge() {
        run_test(|| {
            let request = get_tampered_request(Cose1FieldType::Unprotected(
                UnprotectedHeaderFieldType::ClientData(ClientDataFieldType::Challenge),
            ))
            .verify(None);

            assert!(request.is_err());
            assert_eq!(request.unwrap_err(), "`challenge` SHA doesn't match");
        });
    }

    #[test]
    fn webauthn_tamper_type() {
        run_test(|| {
            let request = get_tampered_request(Cose1FieldType::Unprotected(
                UnprotectedHeaderFieldType::ClientData(ClientDataFieldType::Type(
                    "Foobar".to_string(),
                )),
            ))
            .verify(None);
            assert!(request.is_err());
            assert_eq!(request.unwrap_err(), "request type != webauthn.get");
        });
    }

    #[test]
    fn webauthn_tamper_origin() {
        run_test(|| {
            let request = get_tampered_request(Cose1FieldType::Unprotected(
                UnprotectedHeaderFieldType::ClientData(ClientDataFieldType::Origin(
                    "https://test.com".to_string(),
                )),
            ))
            .verify(Some(vec![ManyUrl::parse("https://foobar.com").unwrap()]));
            assert!(request.is_err());
            assert_eq!(request.unwrap_err(), "Origin not allowed");
        });
    }

    #[test]
    fn webauthn_tamper_payload() {
        run_test(|| {
            let request = get_tampered_request(Cose1FieldType::Payload(vec![1, 2, 3])).verify(None);
            assert!(request.is_err());
            assert_eq!(request.unwrap_err(), "`challenge` SHA doesn't match");
        });
    }

    #[test]
    fn webauthn_tamper_webauthn_flag() {
        run_test(|| {
            let request = get_tampered_request(Cose1FieldType::Protected {
                field: "webauthn".to_string(),
                value: Value::Bool(false), // Unused
            })
            .verify(None);
            assert!(request.is_err());
            assert_eq!(request.unwrap_err(), "signature error");
        });
    }

    #[test]
    fn webauthn_tamper_protected_header() {
        run_test(|| {
            let request = get_tampered_request(Cose1FieldType::Protected {
                field: "foo".to_string(),
                value: Value::Bool(true),
            })
            .verify(None);
            assert!(request.is_err());
            assert_eq!(
                request.unwrap_err(),
                "Protected header doesn't match `challenge`"
            );
        });
    }
}
