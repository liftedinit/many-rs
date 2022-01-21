use crate::identity::cose::CoseKeyIdentity;
use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::{Attribute, Status, StatusBuilder};
use crate::server::module::{OmniModule, OmniModuleInfo};
use crate::transport::OmniRequestHandler;
use crate::OmniError;
use async_trait::async_trait;
use std::collections::BTreeSet;

pub mod function;
pub mod module;

use crate::server::module::base::BaseServerModule;

#[derive(Debug, Clone)]
pub struct OmniModuleList {}

#[derive(Debug, Default)]
pub struct OmniServer {
    modules: Vec<Box<dyn OmniModule>>,
    method_cache: BTreeSet<String>,
    identity: CoseKeyIdentity,
    name: String,
}

impl OmniServer {
    pub fn new<N: ToString>(name: N, identity: CoseKeyIdentity) -> Self {
        Self {
            name: name.to_string(),
            identity,
            ..Default::default()
        }
        .with_module(BaseServerModule)
    }

    pub fn with_module<M>(mut self, module: M) -> Self
    where
        M: OmniModule + 'static,
    {
        let info = module.info();
        let OmniModuleInfo {
            attributes,
            endpoints,
            ..
        } = info;
        for a in attributes {
            let id = a.id;

            if let Some(m) = self
                .modules
                .iter()
                .find(|m| m.info().attributes.iter().any(|a| a.id == id))
            {
                panic!(
                    "Module {} already implements attribute {}.",
                    m.info().name,
                    id
                );
            }
        }

        for e in endpoints {
            if self.method_cache.contains(e.as_str()) {
                unreachable!(
                    "Method '{}' already implemented, but there was no attribute conflict.",
                    e
                );
            }
        }

        // Update the cache.
        for e in endpoints {
            self.method_cache.insert(e.clone());
        }
        self.modules.push(Box::new(module));
        self
    }

    fn status(&self) -> Status {
        let mut attributes: Vec<Attribute> = self
            .modules
            .iter()
            .flat_map(|m| m.info().attributes.clone())
            .collect();
        attributes.sort();

        StatusBuilder::default()
            .name(self.name.clone())
            .version(1)
            .public_key(self.identity.public_key())
            .identity(self.identity.identity)
            .internal_version(std::env!("CARGO_PKG_VERSION").to_string())
            .attributes(attributes)
            .build()
            .unwrap()
    }

    fn endpoints(&self) -> Vec<&str> {
        self.method_cache.iter().map(|x| x.as_str()).collect()
    }
}

#[async_trait]
impl OmniRequestHandler for OmniServer {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        let to = message.to;
        let method = message.method.as_str();

        // Verify that the message is for this server, if it's not anonymous.
        if to.is_anonymous() || self.identity.identity == to {
            // Verify the endpoint.
            if self.method_cache.contains(method) {
                Ok(())
            } else {
                Err(OmniError::invalid_method_name(method.to_string()))
            }
        } else {
            Err(OmniError::unknown_destination(
                to.to_string(),
                self.identity.identity.to_string(),
            ))
        }
    }

    async fn execute(&self, message: RequestMessage) -> Result<ResponseMessage, OmniError> {
        let method = message.method.as_str();

        if let Some(payload) = match message.method.as_str() {
            "status" => Some(
                self.status()
                    .to_bytes()
                    .map_err(OmniError::serialization_error)?,
            ),
            "heartbeat" => Some(Vec::new()),
            "echo" => Some(message.data.clone()),
            "endpoints" => Some(
                minicbor::to_vec(self.endpoints())
                    .map_err(|e| OmniError::serialization_error(e.to_string()))?,
            ),
            _ => None,
        } {
            return Ok(ResponseMessage::from_request(
                &message,
                &self.identity.identity,
                Ok(payload),
            ));
        }

        for m in &self.modules {
            let endpoints = &m.info().endpoints;
            if endpoints.contains(&method.to_string()) {
                m.validate(&message)?;

                return m.execute(message).await.map(|mut r| {
                    r.from = self.identity.identity;
                    r
                });
            }
        }

        Err(OmniError::invalid_method_name(method.to_string()))
    }
}
