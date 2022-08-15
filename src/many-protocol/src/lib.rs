use coset::CoseSign1Builder;
use coset::{CoseKeySet, CoseSign1};
use many_error::ManyError;
use many_identity::{Address, Identity, Verifier};

pub mod request;
pub mod response;

pub use request::{RequestMessage, RequestMessageBuilder};
pub use response::{ResponseMessage, ResponseMessageBuilder};

pub type ManyUrl = reqwest::Url;

pub trait IdentityResolver: Send {
    fn resolve_request(
        &self,
        envelope: &CoseSign1,
        request: &RequestMessage,
    ) -> Result<Address, ManyError>;
    fn resolve_response(
        &self,
        envelope: &CoseSign1,
        response: &ResponseMessage,
    ) -> Result<Address, ManyError>;
}

impl IdentityResolver for Box<dyn IdentityResolver> {
    fn resolve_request(
        &self,
        envelope: &CoseSign1,
        request: &RequestMessage,
    ) -> Result<Address, ManyError> {
        (&**self).resolve_request(envelope, request)
    }

    fn resolve_response(
        &self,
        envelope: &CoseSign1,
        response: &ResponseMessage,
    ) -> Result<Address, ManyError> {
        (&**self).resolve_response(envelope, response)
    }
}

pub fn decode_request_from_cose_sign1(
    envelope: CoseSign1,
    verifier: &impl Verifier,
    resolver: Option<&impl IdentityResolver>,
) -> Result<RequestMessage, ManyError> {
    verifier.sign_1(&envelope)?;

    let payload = envelope
        .payload
        .as_ref()
        .ok_or_else(ManyError::empty_envelope)?;
    let message = RequestMessage::from_bytes(payload).map_err(ManyError::deserialization_error)?;

    if let Some(resolver) = resolver {
        let from_id = resolver.resolve_request(&envelope, &message)?;

        // Check the `from` field.
        if !from_id.matches(&message.from.unwrap_or_default()) {
            return Err(ManyError::invalid_from_identity());
        }
    }

    Ok(message)
}

pub fn decode_response_from_cose_sign1(
    envelope: CoseSign1,
    to: Option<Address>,
    verifier: &impl Verifier,
    resolver: Option<&impl IdentityResolver>,
) -> Result<ResponseMessage, ManyError> {
    let message = ResponseMessage::decode_and_verify(&envelope, verifier)?;

    if let Some(resolver) = resolver {
        let from_id = resolver.resolve_response(&envelope, &message)?;
        // Check the `from` field.
        if from_id != message.from {
            return Err(ManyError::invalid_from_identity());
        }
    }

    // Check the `to` field to make sure we have the right one.
    if let Some(to_id) = to {
        if to_id != message.to.unwrap_or_default() {
            return Err(ManyError::invalid_to_identity());
        }
    }

    Ok(message)
}

fn encode_cose_sign1_from_payload(
    payload: Vec<u8>,
    identity: &impl Identity,
) -> Result<CoseSign1, ManyError> {
    let sign1 = CoseSign1Builder::default().payload(payload).build();
    // TODO: replace this with ManyError, not map_err.
    identity.sign_1(sign1)
}

pub fn encode_cose_sign1_from_response(
    response: ResponseMessage,
    identity: &impl Identity,
) -> Result<CoseSign1, ManyError> {
    encode_cose_sign1_from_payload(response.to_bytes().unwrap(), identity)
}

pub fn encode_cose_sign1_from_request(
    request: RequestMessage,
    identity: &impl Identity,
) -> Result<CoseSign1, ManyError> {
    encode_cose_sign1_from_payload(request.to_bytes().unwrap(), identity)
}
