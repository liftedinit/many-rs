use crate::identity::cose::CoseKeyIdentity;
use crate::message::{
    decode_response_from_cose_sign1, encode_cose_sign1_from_request, RequestMessage,
    RequestMessageBuilder,
};
use crate::protocol::Status;
use crate::{Identity, OmniError};
use minicbor::Encode;
use minicose::CoseSign1;
use reqwest::{IntoUrl, Url};
use std::fmt::Formatter;

#[derive(Clone)]
pub struct OmniClient {
    pub id: CoseKeyIdentity,
    pub to: Identity,
    url: Url,
}

impl std::fmt::Debug for OmniClient {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OmniClient")
            .field("id", &self.id)
            .field("to", &self.to)
            .field("url", &self.url)
            .finish()
    }
}

impl OmniClient {
    pub fn new<S: IntoUrl>(url: S, to: Identity, id: CoseKeyIdentity) -> Result<Self, String> {
        Ok(Self {
            id,
            to,
            url: url.into_url().map_err(|e| format!("{}", e))?,
        })
    }

    pub fn send_envelope<S: IntoUrl>(url: S, message: CoseSign1) -> Result<CoseSign1, OmniError> {
        let bytes = message
            .to_bytes()
            .map_err(|_| OmniError::internal_server_error())?;

        let client = reqwest::blocking::Client::new();
        let response = client
            .post(url)
            .body(bytes)
            .send()
            .map_err(|e| OmniError::unexpected_transport_error(e.to_string()))?;
        let body = response.bytes().unwrap();
        let bytes = body.to_vec();
        tracing::debug!("reply\n{}", hex::encode(&bytes));
        CoseSign1::from_bytes(&bytes).map_err(|e| OmniError::deserialization_error(e.to_string()))
    }

    pub fn send_message(&self, message: RequestMessage) -> Result<Vec<u8>, OmniError> {
        let cose = encode_cose_sign1_from_request(message, &self.id).unwrap();
        let cose_sign1 = Self::send_envelope(self.url.clone(), cose)?;

        let response = decode_response_from_cose_sign1(cose_sign1, None)
            .map_err(OmniError::deserialization_error)?;

        response.data
    }

    pub fn call_raw<M>(&self, method: M, argument: &[u8]) -> Result<Vec<u8>, OmniError>
    where
        M: Into<String>,
    {
        let message: RequestMessage = RequestMessageBuilder::default()
            .version(1)
            .from(self.id.identity)
            .to(self.to)
            .method(method.into())
            .data(argument.to_vec())
            .build()
            .map_err(|_| OmniError::internal_server_error())?;

        self.send_message(message)
    }

    pub fn call_<M, I>(&self, method: M, argument: I) -> Result<Vec<u8>, OmniError>
    where
        M: Into<String>,
        I: Encode,
    {
        let bytes: Vec<u8> = minicbor::to_vec(argument)
            .map_err(|e| OmniError::serialization_error(e.to_string()))?;

        self.call_raw(method, bytes.as_slice())
    }

    pub fn status(&self) -> Result<Status, OmniError> {
        let response = self.call_("status", ())?;

        let status = minicbor::decode(response.as_slice())
            .map_err(|e| OmniError::deserialization_error(e.to_string()))?;
        Ok(status)
    }
}
