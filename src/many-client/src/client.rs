pub mod base;
pub mod blockchain;
pub mod blocking;
pub mod ledger;

pub use ledger::LedgerClient;

use coset::{CoseSign1, TaggedCborSerializable};
use many_identity::verifiers::AnonymousVerifier;
use many_identity::{verifiers, Identity};
use many_identity_dsa::CoseKeyVerifier;
use many_modules::base::Status;
use many_protocol::{
    encode_cose_sign1_from_request, RequestMessage, RequestMessageBuilder, ResponseMessage,
};
use many_server::{Address, ManyError};
use minicbor::Encode;
use reqwest::{IntoUrl, Url};
use std::fmt::{Debug, Formatter};

#[derive(Clone)]
pub struct ManyClient<I: Identity> {
    identity: I,
    to: Option<Address>,
    url: Url,
    verifier: (AnonymousVerifier, CoseKeyVerifier),
}

impl<I: Identity + Debug> Debug for ManyClient<I> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManyClient")
            .field("id", &self.identity)
            .field("to", &self.to)
            .field("url", &self.url)
            .finish()
    }
}

pub async fn send_envelope<S: IntoUrl>(url: S, message: CoseSign1) -> Result<CoseSign1, ManyError> {
    let bytes = message
        .to_tagged_vec()
        .map_err(|_| ManyError::internal_server_error())?;

    let client = reqwest::Client::new();
    tracing::debug!("request {}", hex::encode(&bytes));
    let response = client
        .post(url)
        .body(bytes)
        .send()
        .await
        .map_err(|e| ManyError::unexpected_transport_error(e.to_string()))?;
    let body = response.bytes().await.unwrap();
    let bytes = body.to_vec();
    tracing::debug!("reply {}", hex::encode(&bytes));
    CoseSign1::from_tagged_slice(&bytes)
        .map_err(|e| ManyError::deserialization_error(e.to_string()))
}

impl<I: Identity> ManyClient<I> {
    pub fn new<S: IntoUrl>(url: S, to: Address, identity: I) -> Result<Self, String> {
        let verifier = (verifiers::AnonymousVerifier, CoseKeyVerifier);

        Ok(Self {
            identity,
            to: Some(to),
            url: url.into_url().map_err(|e| e.to_string())?,
            verifier,
        })
    }

    pub async fn send_message(
        &self,
        message: RequestMessage,
    ) -> Result<ResponseMessage, ManyError> {
        let cose = encode_cose_sign1_from_request(message, &self.identity).unwrap();
        let cose_sign1 = send_envelope(self.url.clone(), cose).await?;

        ResponseMessage::decode_and_verify(&cose_sign1, &self.verifier)
    }

    pub async fn call_raw<M>(
        &self,
        method: M,
        argument: &[u8],
    ) -> Result<ResponseMessage, ManyError>
    where
        M: Into<String>,
    {
        let mut nonce = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce);

        let mut builder = RequestMessageBuilder::default();

        builder
            .version(1)
            .from(self.identity.address())
            .method(method.into())
            .data(argument.to_vec())
            .nonce(nonce.to_vec());

        let message: RequestMessage = if let Some(to) = self.to {
            builder.to(to)
        } else {
            &mut builder
        }
        .build()
        .map_err(|_| ManyError::internal_server_error())?;

        self.send_message(message).await
    }

    pub async fn call<M, A>(&self, method: M, argument: A) -> Result<ResponseMessage, ManyError>
    where
        M: Into<String>,
        A: Encode<()>,
    {
        let bytes: Vec<u8> = minicbor::to_vec(argument)
            .map_err(|e| ManyError::serialization_error(e.to_string()))?;

        self.call_raw(method, bytes.as_slice()).await
    }

    pub async fn call_<M, A>(&self, method: M, argument: A) -> Result<Vec<u8>, ManyError>
    where
        M: Into<String>,
        A: Encode<()>,
    {
        self.call(method, argument).await?.data
    }

    pub async fn status(&self) -> Result<Status, ManyError> {
        let response = self.call_("status", ()).await?;

        let status = minicbor::decode(response.as_slice())
            .map_err(|e| ManyError::deserialization_error(e.to_string()))?;
        Ok(status)
    }
}
