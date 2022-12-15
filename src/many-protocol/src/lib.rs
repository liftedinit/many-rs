use coset::CoseSign1;
use coset::CoseSign1Builder;
use many_error::ManyError;
use many_identity::{Address, Identity, Verifier};

pub mod request;
pub mod response;

pub use request::{RequestMessage, RequestMessageBuilder};
pub use response::{ResponseMessage, ResponseMessageBuilder};

pub type ManyUrl = url::Url;

pub fn decode_request_from_cose_sign1(
    envelope: &CoseSign1,
    verifier: &impl Verifier,
) -> Result<RequestMessage, ManyError> {
    let from_id = verifier.verify_1(envelope)?;

    if from_id.is_illegal() {
        return Err(ManyError::invalid_from_identity());
    }

    let payload = envelope
        .payload
        .as_ref()
        .ok_or_else(ManyError::empty_envelope)?;
    let message = RequestMessage::from_bytes(payload).map_err(ManyError::deserialization_error)?;

    // Check the `from` field.
    let message_from = message.from.unwrap_or_default();
    if !from_id.matches(&message_from) {
        Err(ManyError::invalid_from_identity())
    } else if message_from.is_illegal() {
        Err(ManyError::invalid_from_identity())
    } else {
        Ok(message)
    }
}

pub fn decode_response_from_cose_sign1(
    envelope: &CoseSign1,
    to: Option<Address>,
    verifier: &impl Verifier,
) -> Result<ResponseMessage, ManyError> {
    let message = ResponseMessage::decode_and_verify(envelope, verifier)?;

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
