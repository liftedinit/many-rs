use std::sync::Arc;

use many_identity::{Address, Identity};
use many_modules::base::Status;
use many_protocol::{RequestMessage, ResponseMessage};
use many_server::ManyError;
use minicbor::Encode;
use reqwest::IntoUrl;
use tokio::runtime::Runtime;

use crate::ManyClient as AsyncClient;

#[derive(Debug, Clone)]
pub struct ManyClient<I: Identity> {
    client: AsyncClient<I>,
    runtime: Arc<Runtime>,
}

impl<I: Identity> ManyClient<I> {
    pub fn new<S: IntoUrl>(url: S, to: Address, identity: I) -> Result<Self, String> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        Self::new_with_runtime(url, to, identity, Arc::new(runtime))
    }

    pub fn new_with_runtime<S: IntoUrl>(
        url: S,
        to: Address,
        identity: I,
        runtime: Arc<Runtime>,
    ) -> Result<Self, String> {
        let client = AsyncClient::new(url, to, identity)?;

        Ok(Self { client, runtime })
    }

    pub fn send_message(&self, message: RequestMessage) -> Result<ResponseMessage, ManyError> {
        self.runtime.block_on(self.client.send_message(message))
    }

    pub fn call_raw<M>(&self, method: M, argument: &[u8]) -> Result<ResponseMessage, ManyError> where M: Into<String> {
        self.runtime.block_on(self.client.call_raw(method, argument))
    }

    pub fn call<M, A>(&self, method: M, argument: A) -> Result<ResponseMessage, ManyError>
    where
        M: Into<String>,
        A: Encode<()>, {
        self.runtime.block_on(self.client.call(method, argument))
    }

    pub fn call_<M, A>(&self, method: M, argument: A) -> Result<Vec<u8>, ManyError>
    where
        M: Into<String>,
        A: Encode<()>,
    {
        self.runtime.block_on(self.client.call_(method, argument))
    }

    pub fn status(&self) -> Result<Status, ManyError> {
        self.runtime.block_on(self.client.status())
    }
}
