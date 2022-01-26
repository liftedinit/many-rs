use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::Attribute;
use crate::server::module::base::{
    BaseModule, BaseModuleBackend, Endpoints, Status, StatusBuilder,
};
use crate::server::module::{OmniModule, OmniModuleInfo};
use crate::transport::OmniRequestHandler;
use crate::types::identity::cose::CoseKeyIdentity;
use crate::{Identity, OmniError};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

pub mod function;
pub mod module;

#[derive(Debug, Clone)]
pub struct OmniModuleList {}

#[derive(Debug, Default)]
pub struct OmniServer {
    modules: Vec<Arc<dyn OmniModule + Send>>,
    method_cache: BTreeSet<String>,
    identity: CoseKeyIdentity,
    name: String,
    version: Option<String>,
}

impl OmniServer {
    pub fn new<N: ToString>(
        name: N,
        identity: CoseKeyIdentity,
        version: Option<String>,
    ) -> Arc<Mutex<Self>> {
        let s = Arc::new(Mutex::new(Self {
            name: name.to_string(),
            identity,
            version,
            ..Default::default()
        }));

        {
            let s2 = s.clone();
            let mut backend = s.lock().unwrap();

            backend.add_module(BaseModule::new(s2));
        }
        s
    }

    pub fn add_module<M>(&mut self, module: M) -> &mut Self
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
        self.modules.push(Arc::new(module));
        self
    }
}

impl BaseModuleBackend for OmniServer {
    fn endpoints(&self) -> Result<Endpoints, OmniError> {
        Ok(Endpoints(self.method_cache.iter().cloned().collect()))
    }

    fn status(&self) -> Result<Status, OmniError> {
        let mut attributes: Vec<Attribute> = self
            .modules
            .iter()
            .flat_map(|m| m.info().attributes.clone())
            .collect();
        attributes.sort();

        let mut builder = StatusBuilder::default();

        builder
            .name(self.name.clone())
            .version(1)
            .identity(self.identity.identity)
            .attributes(attributes)
            .extras(BTreeMap::new());

        if let Some(pk) = self.identity.public_key() {
            builder.public_key(pk);
        }
        if let Some(sv) = self.version.clone() {
            builder.server_version(sv);
        }

        builder
            .build()
            .map_err(|x| OmniError::unknown(x.to_string()))
    }
}

impl OmniRequestHandler for Arc<Mutex<OmniServer>> {
    fn validate(&self, message: &RequestMessage) -> Result<(), OmniError> {
        let s = self.lock().unwrap();
        let to = message.to;
        let method = message.method.as_str();

        // Verify that the message is for this server, if it's not anonymous.
        if to.is_anonymous() || s.identity.identity == to {
            // Verify the endpoint.
            if s.method_cache.contains(method) {
                Ok(())
            } else {
                Err(OmniError::invalid_method_name(method.to_string()))
            }
        } else {
            Err(OmniError::unknown_destination(
                to.to_string(),
                s.identity.identity.to_string(),
            ))
        }
    }

    fn execute<'life0, 'async_trait>(
        &'life0 self,
        message: RequestMessage,
    ) -> Pin<Box<dyn Future<Output = Result<ResponseMessage, OmniError>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        async fn ex(
            method: String,
            module: Option<Arc<dyn OmniModule + Send>>,
            from: Identity,
            message: RequestMessage,
        ) -> Result<ResponseMessage, OmniError> {
            if let Some(m) = module {
                m.validate(&message)?;

                return m.execute(message).await.map(|mut r| {
                    r.from = from;
                    r
                });
            } else {
                Err(OmniError::invalid_method_name(method))
            }
        }

        let s = self.lock().unwrap();
        let method = message.method.clone();
        let from = s.identity.identity;

        let m = s
            .modules
            .iter()
            .find(|x| x.info().endpoints.contains(&method.to_string()))
            .cloned();

        Box::pin(ex(method, m, from, message))
    }
}
