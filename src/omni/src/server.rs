use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::Attribute;
use crate::server::module::base::{
    BaseModule, BaseModuleBackend, Endpoints, Status, StatusBuilder,
};
use crate::server::module::{OmniModule, OmniModuleInfo};
use crate::transport::LowLevelOmniRequestHandler;
use crate::types::identity::cose::CoseKeyIdentity;
use crate::OmniError;
use async_trait::async_trait;
use minicose::CoseSign1;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

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
    fallback: Option<Arc<dyn LowLevelOmniRequestHandler + Send>>,
}

impl OmniServer {
    pub fn simple<N: ToString>(
        name: N,
        identity: CoseKeyIdentity,
        version: Option<String>,
    ) -> Arc<Mutex<Self>> {
        let s = Self::new(name, identity);
        {
            let mut s2 = s.lock().unwrap();
            s2.version = version;
            s2.add_module(BaseModule::new(s.clone()));
        }

        s
    }

    pub fn new<N: ToString>(name: N, identity: CoseKeyIdentity) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            name: name.to_string(),
            identity,
            ..Default::default()
        }))
    }

    pub fn set_fallback_module<M>(&mut self, module: M) -> &mut Self
    where
        M: LowLevelOmniRequestHandler + 'static,
    {
        self.fallback = Some(Arc::new(module));
        self
    }

    pub fn add_module<M>(&mut self, module: M) -> &mut Self
    where
        M: OmniModule + 'static,
    {
        let info = module.info();
        let OmniModuleInfo {
            attribute,
            endpoints,
            ..
        } = info;

        let id = attribute.id;

        if let Some(m) = self.modules.iter().find(|m| m.info().attribute.id == id) {
            panic!(
                "Module {} already implements attribute {}.",
                m.info().name,
                id
            );
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

    pub fn validate_id(&self, message: &RequestMessage) -> Result<(), OmniError> {
        let to = &message.to;

        // Verify that the message is for this server, if it's not anonymous.
        if to.is_anonymous() || &self.identity.identity == to {
            Ok(())
        } else {
            Err(OmniError::unknown_destination(
                to.to_string(),
                self.identity.identity.to_string(),
            ))
        }
    }

    pub fn find_module(&self, message: &RequestMessage) -> Option<Arc<dyn OmniModule + Send>> {
        self.modules
            .iter()
            .find(|x| x.info().endpoints.contains(&message.method))
            .cloned()
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
            .map(|m| m.info().attribute.clone())
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

#[async_trait]
impl LowLevelOmniRequestHandler for Arc<Mutex<OmniServer>> {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String> {
        let request = crate::message::decode_request_from_cose_sign1(envelope.clone());

        let response = {
            let this = self.lock().unwrap();
            let cose_id = this.identity.clone();
            eprintln!("id: {:?}", cose_id);
            request
                .and_then(|message| {
                    this.validate_id(&message)?;
                    Ok(message)
                })
                .and_then(|message| {
                    let maybe_module = this.find_module(&message);
                    Ok((message, maybe_module))
                })
                .and_then(|(message, maybe_module)| {
                    if let Some(ref m) = maybe_module {
                        m.validate(&message)?;
                    }
                    Ok((message, maybe_module))
                })
                .and_then(|(message, maybe_module)| {
                    Ok((
                        cose_id.clone(),
                        message,
                        maybe_module,
                        this.fallback.clone(),
                    ))
                })
                .or_else(|omni_err| Err(ResponseMessage::error(&cose_id.identity, omni_err)))
        };

        match response {
            Ok((cose_id, message, maybe_module, fallback)) => match (maybe_module, fallback) {
                (Some(m), _) => {
                    let mut response = match m.execute(message).await {
                        Ok(response) => response,
                        Err(omni_err) => ResponseMessage::error(&cose_id.identity, omni_err),
                    };
                    response.from = cose_id.identity;
                    crate::message::encode_cose_sign1_from_response(response, &cose_id)
                }
                (None, Some(fb)) => fb.execute(envelope).await,
                (None, None) => {
                    let err = OmniError::could_not_route_message();
                    let response = ResponseMessage::error(&cose_id.identity, err);
                    crate::message::encode_cose_sign1_from_response(response, &cose_id)
                }
            },
            Err(response) => {
                let this = self.lock().unwrap();
                crate::message::encode_cose_sign1_from_response(response, &this.identity)
            }
        }
    }
}
