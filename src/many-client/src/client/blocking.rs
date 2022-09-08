use std::sync::Arc;

use many_identity::{Address, Identity};
use many_modules::base::Status;
use many_protocol::{RequestMessage, ResponseMessage};
use many_server::ManyError;
use minicbor::Encode;
use reqwest::IntoUrl;
use tokio::runtime::{self, Handle, Runtime};

use crate::ManyClient as AsyncClient;

#[derive(Debug, Clone)]
enum RuntimeChoice {
    Runtime(Arc<Runtime>),
    Handle(Handle),
}

#[derive(Debug, Clone)]
pub struct ManyClient<I: Identity> {
    client: AsyncClient<I>,
    runtime_choice: RuntimeChoice,
}

impl<I: Identity> ManyClient<I> {
    fn handle(&self) -> &Handle {
        match &self.runtime_choice {
            RuntimeChoice::Runtime(r) => r.handle(),
            RuntimeChoice::Handle(h) => h,
        }
    }

    pub fn new<S: IntoUrl>(url: S, to: Address, identity: I) -> Result<Self, String> {
        let client = AsyncClient::new(url, to, identity)?;
        match Handle::try_current() {
            Ok(h) => Ok(Self {
                client,
                runtime_choice: RuntimeChoice::Handle(h),
            }),
            Err(_) => Ok(Self {
                client,
                runtime_choice: RuntimeChoice::Runtime(Arc::new(
                    runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| e.to_string())?,
                )),
            }),
        }
    }

    pub fn send_message(&self, message: RequestMessage) -> Result<ResponseMessage, ManyError> {
        self.handle().block_on(self.client.send_message(message))
    }

    pub fn call_raw<M>(&self, method: M, argument: &[u8]) -> Result<ResponseMessage, ManyError>
    where
        M: Into<String>,
    {
        self.handle()
            .block_on(self.client.call_raw(method, argument))
    }

    pub fn call<M, A>(&self, method: M, argument: A) -> Result<ResponseMessage, ManyError>
    where
        M: Into<String>,
        A: Encode<()>,
    {
        self.handle().block_on(self.client.call(method, argument))
    }

    pub fn call_<M, A>(&self, method: M, argument: A) -> Result<Vec<u8>, ManyError>
    where
        M: Into<String>,
        A: Encode<()>,
    {
        self.handle().block_on(self.client.call_(method, argument))
    }

    pub fn status(&self) -> Result<Status, ManyError> {
        self.handle().block_on(self.client.status())
    }
}
