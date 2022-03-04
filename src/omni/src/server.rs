use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::Attribute;
use crate::server::module::{base, OmniModule, OmniModuleInfo};
use crate::transport::LowLevelOmniRequestHandler;
use crate::types::identity::cose::CoseKeyIdentity;
use crate::OmniError;
use async_trait::async_trait;
use minicose::CoseSign1;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

pub mod module;

trait OmniServerFallback: LowLevelOmniRequestHandler + base::BaseModuleBackend {}

impl<M: LowLevelOmniRequestHandler + base::BaseModuleBackend + 'static> OmniServerFallback for M {}

#[derive(Debug, Clone)]
pub struct OmniModuleList {}

#[derive(Debug, Default)]
pub struct OmniServer {
    modules: Vec<Arc<dyn OmniModule + Send>>,
    method_cache: BTreeSet<String>,
    identity: CoseKeyIdentity,
    name: String,
    version: Option<String>,
    timeout: u64,
    fallback: Option<Arc<dyn OmniServerFallback + Send + 'static>>,
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
            s2.add_module(base::BaseModule::new(s.clone()));
        }

        s
    }

    pub fn new<N: ToString>(name: N, identity: CoseKeyIdentity) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            name: name.to_string(),
            identity,
            timeout: 300,
            ..Default::default()
        }))
    }

    pub fn set_fallback_module<M>(&mut self, module: M) -> &mut Self
    where
        M: LowLevelOmniRequestHandler + base::BaseModuleBackend + 'static,
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

    pub fn validate_time(&self, message: &RequestMessage) -> Result<(), OmniError> {
        if self.timeout != 0 {
            let ts = message
                .timestamp
                .ok_or_else(|| OmniError::required_field_missing("timestamp".to_string()))?;
            let now = SystemTime::now();

            let diff = now.duration_since(ts).map_err(|_| {
                tracing::error!("ERR: System time error");
                OmniError::timestamp_out_of_range()
            })?;
            if diff.as_secs() >= self.timeout {
                tracing::error!(
                    "ERR: Timestamp outside of timeout: {} >= {}",
                    diff.as_secs(),
                    self.timeout
                );
                return Err(OmniError::timestamp_out_of_range());
            }
        }
        Ok(())
    }

    pub fn find_module(&self, message: &RequestMessage) -> Option<Arc<dyn OmniModule + Send>> {
        self.modules
            .iter()
            .find(|x| x.info().endpoints.contains(&message.method))
            .cloned()
    }
}

impl base::BaseModuleBackend for OmniServer {
    fn endpoints(&self) -> Result<base::Endpoints, OmniError> {
        let mut endpoints: BTreeSet<String> = self.method_cache.iter().cloned().collect();

        if let Some(fb) = &self.fallback {
            endpoints = endpoints
                .union(&fb.endpoints()?.0.iter().cloned().collect::<BTreeSet<_>>())
                .cloned()
                .collect();
        }

        Ok(base::Endpoints(endpoints))
    }

    fn status(&self) -> Result<base::Status, OmniError> {
        let mut attributes: BTreeSet<Attribute> = self
            .modules
            .iter()
            .map(|m| m.info().attribute.clone())
            .collect();

        let mut builder = base::StatusBuilder::default();

        builder
            .name(self.name.clone())
            .version(1)
            .identity(self.identity.identity)
            .timeout(self.timeout)
            .extras(BTreeMap::new());

        if let Some(pk) = self.identity.public_key() {
            builder.public_key(pk);
        }
        if let Some(sv) = self.version.clone() {
            builder.server_version(sv);
        }

        if let Some(fb) = &self.fallback {
            let fb_status = fb.status()?;
            if fb_status.identity != self.identity.identity
                || fb_status.version != 1
                || (fb_status.server_version != self.version && self.version.is_some())
            {
                tracing::error!(
                    "fallback status differs from internal status: {} != {} || {:?} != {:?}",
                    fb_status.identity,
                    self.identity.identity,
                    fb_status.server_version,
                    self.version
                );
                return Err(OmniError::internal_server_error());
            }

            if let Some(sv) = fb_status.server_version {
                builder.server_version(sv);
            }

            builder.name(fb_status.name).extras(fb_status.extras);

            attributes = attributes
                .into_iter()
                .chain(fb_status.attributes.into_iter())
                .collect();
        }

        builder.attributes(attributes.into_iter().collect());

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
            request
                .and_then(|message| {
                    this.validate_time(&message)?;
                    Ok(message)
                })
                .and_then(|message| {
                    this.validate_id(&message)?;
                    Ok(message)
                })
                .map(|message| {
                    let maybe_module = this.find_module(&message);
                    (message, maybe_module)
                })
                .and_then(|(message, maybe_module)| {
                    if let Some(ref m) = maybe_module {
                        m.validate(&message)?;
                    }
                    Ok((message, maybe_module))
                })
                .map(|(message, maybe_module)| {
                    (
                        cose_id.clone(),
                        message,
                        maybe_module,
                        this.fallback.clone(),
                    )
                })
                .map_err(|omni_err| ResponseMessage::error(&cose_id.identity, omni_err))
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
                (None, Some(fb)) => {
                    LowLevelOmniRequestHandler::execute(fb.as_ref(), envelope).await
                }
                (None, None) => {
                    let response = ResponseMessage::error(
                        &cose_id.identity,
                        OmniError::could_not_route_message(),
                    );
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
