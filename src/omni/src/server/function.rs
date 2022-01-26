use crate::message::{RequestMessage, ResponseMessage};
use crate::transport::{OmniRequestHandler, SimpleRequestHandler, SimpleRequestHandlerAdapter};
use crate::OmniError;
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::fmt::Formatter;
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct FunctionMapRequestHandler {
    handlers: BTreeMap<String, Arc<dyn OmniRequestHandler>>,
}

impl FunctionMapRequestHandler {
    pub fn empty() -> Self {
        Default::default()
    }

    pub fn with_handler<NS, H>(mut self, method_name: NS, handler: H) -> Self
    where
        NS: ToString,
        H: OmniRequestHandler + 'static,
    {
        self.handlers
            .insert(method_name.to_string(), Arc::new(handler));
        self
    }

    pub fn with_method<F>(self, method: &str, handler: F) -> Self
    where
        F: Fn(&[u8]) -> Result<Vec<u8>, OmniError> + Send + Sync + 'static,
    {
        struct Handler<F: Fn(&[u8]) -> Result<Vec<u8>, OmniError> + Send + Sync>(pub F);
        impl<F: Fn(&[u8]) -> Result<Vec<u8>, OmniError> + Send + Sync> std::fmt::Debug for Handler<F> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.write_str("Handler")
            }
        }

        #[async_trait]
        impl<F> SimpleRequestHandler for Handler<F>
        where
            F: Fn(&[u8]) -> Result<Vec<u8>, OmniError> + Send + Sync,
        {
            async fn handle(&self, _method: &str, payload: &[u8]) -> Result<Vec<u8>, OmniError> {
                self.0(payload)
            }
        }

        self.with_handler(method, SimpleRequestHandlerAdapter(Handler(handler)))
    }
}

impl std::fmt::Debug for FunctionMapRequestHandler {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("ModuleRequestHandler ")?;
        let mut list = f.debug_map();
        for (ns, h) in &self.handlers {
            list.entry(ns, h);
        }
        list.finish()
    }
}

#[async_trait]
impl OmniRequestHandler for FunctionMapRequestHandler {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        let method = message.method.as_str();
        if let Some(h) = self.handlers.get(method) {
            h.validate(message)
        } else {
            Err(OmniError::invalid_method_name(method.to_string()))
        }
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let method = message.method.as_str();
        if let Some(h) = self.handlers.get(method) {
            h.execute(message).await
        } else {
            Err(OmniError::invalid_method_name(method.to_string()))
        }
    }
}
