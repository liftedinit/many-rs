use async_trait::async_trait;
use coset::CoseSign1;
use many_error::ManyError;
use many_protocol::{RequestMessage, ResponseMessage};
use std::fmt::Debug;

pub mod http;

#[async_trait]
pub trait LowLevelManyRequestHandler: Send + Sync + Debug {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String>;
}

/// A simpler version of the [ManyRequestHandler] which only deals with methods and payloads.
#[async_trait]
pub trait SimpleRequestHandler: Send + Sync + Debug {
    fn validate(&self, _method: &str, _payload: &[u8]) -> Result<(), ManyError> {
        Ok(())
    }

    async fn handle(&self, method: &str, payload: &[u8]) -> Result<Vec<u8>, ManyError>;
}

#[async_trait]
pub trait ManyRequestHandler: Send + Sync + Debug {
    /// Validate that a message is okay with us.
    fn validate(&self, _message: &RequestMessage) -> Result<(), ManyError> {
        Ok(())
    }

    /// Handle an incoming request message, and returns the response message.
    /// This cannot fail. It should instead responds with a proper error response message.
    /// See the spec.
    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, ManyError>;
}

#[derive(Debug)]
pub struct SimpleRequestHandlerAdapter<I: SimpleRequestHandler>(pub I);

#[async_trait]
impl<I: SimpleRequestHandler> ManyRequestHandler for SimpleRequestHandlerAdapter<I> {
    fn validate(&self, message: &RequestMessage) -> Result<(), ManyError> {
        self.0
            .validate(message.method.as_str(), message.data.as_slice())
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, ManyError> {
        let payload = self
            .0
            .handle(message.method.as_str(), message.data.as_slice())
            .await;

        Ok(ResponseMessage {
            version: Some(1),
            from: message.to,
            data: payload,
            to: message.from,
            id: message.id,
            ..Default::default()
        })
    }
}
