use crate::message::{RequestMessage, ResponseMessage};
use crate::protocol::Attribute;
use crate::server::module::{base, ManyModule, ManyModuleInfo};
use crate::transport::LowLevelManyRequestHandler;
use crate::types::identity::cose::CoseKeyIdentity;
use crate::ManyError;
use async_trait::async_trait;
use coset::CoseSign1;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

pub mod module;

/// Validate that the timestamp of a message is within a timeout, either in the future
/// or the past.
fn _validate_time(
    message: &RequestMessage,
    now: SystemTime,
    timeout_in_secs: u64,
) -> Result<(), ManyError> {
    if timeout_in_secs == 0 {
        return Err(ManyError::timestamp_out_of_range());
    }
    let ts = message
        .timestamp
        .ok_or_else(|| ManyError::required_field_missing("timestamp".to_string()))?;

    // Get the absolute time difference.
    let (early, later) = if ts < now { (ts, now) } else { (now, ts) };
    let diff = later
        .duration_since(early)
        .map_err(|_| ManyError::timestamp_out_of_range())?;

    if diff.as_secs() >= timeout_in_secs {
        tracing::error!(
            "ERR: Timestamp outside of timeout: {} >= {}",
            diff.as_secs(),
            timeout_in_secs
        );
        return Err(ManyError::timestamp_out_of_range());
    }

    Ok(())
}

pub type ManyUrl = reqwest::Url;

trait ManyServerFallback: LowLevelManyRequestHandler + base::BaseModuleBackend {}

impl<M: LowLevelManyRequestHandler + base::BaseModuleBackend + 'static> ManyServerFallback for M {}

#[derive(Debug, Clone)]
pub struct ManyModuleList {}

pub const MANYSERVER_DEFAULT_TIMEOUT: u64 = 300;

#[derive(Debug, Default)]
pub struct ManyServer {
    modules: Vec<Arc<dyn ManyModule + Send>>,
    method_cache: BTreeSet<String>,
    identity: CoseKeyIdentity,
    name: String,
    version: Option<String>,
    timeout: u64,
    fallback: Option<Arc<dyn ManyServerFallback + Send + 'static>>,
    allowed_origins: Option<Vec<ManyUrl>>,
}

impl ManyServer {
    pub fn simple<N: ToString>(
        name: N,
        identity: CoseKeyIdentity,
        version: Option<String>,
        allow: Option<Vec<ManyUrl>>,
    ) -> Arc<Mutex<Self>> {
        let s = Self::new(name, identity, allow);
        {
            let mut s2 = s.lock().unwrap();
            s2.version = version;
            s2.add_module(base::BaseModule::new(s.clone()));
        }

        s
    }

    pub fn new<N: ToString>(
        name: N,
        identity: CoseKeyIdentity,
        allowed_origins: Option<Vec<ManyUrl>>,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            name: name.to_string(),
            identity,
            timeout: MANYSERVER_DEFAULT_TIMEOUT,
            allowed_origins,
            ..Default::default()
        }))
    }

    pub fn set_fallback_module<M>(&mut self, module: M) -> &mut Self
    where
        M: LowLevelManyRequestHandler + base::BaseModuleBackend + 'static,
    {
        self.fallback = Some(Arc::new(module));
        self
    }

    pub fn add_module<M>(&mut self, module: M) -> &mut Self
    where
        M: ManyModule + 'static,
    {
        let info = module.info();
        let ManyModuleInfo {
            attribute,
            endpoints,
            ..
        } = info;

        if let Some(Attribute { id, .. }) = attribute {
            if let Some(m) = self
                .modules
                .iter()
                .find(|m| m.info().attribute.as_ref().map(|x| x.id) == Some(*id))
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

    pub fn validate_id(&self, message: &RequestMessage) -> Result<(), ManyError> {
        let to = &message.to;

        // Verify that the message is for this server, if it's not anonymous.
        if to.is_anonymous() || &self.identity.identity == to {
            Ok(())
        } else {
            Err(ManyError::unknown_destination(
                to.to_string(),
                self.identity.identity.to_string(),
            ))
        }
    }

    pub fn find_module(&self, message: &RequestMessage) -> Option<Arc<dyn ManyModule + Send>> {
        self.modules
            .iter()
            .find(|x| x.info().endpoints.contains(&message.method))
            .cloned()
    }
}

impl base::BaseModuleBackend for ManyServer {
    fn endpoints(&self) -> Result<base::Endpoints, ManyError> {
        let mut endpoints: BTreeSet<String> = self.method_cache.iter().cloned().collect();

        if let Some(fb) = &self.fallback {
            endpoints = endpoints
                .union(&fb.endpoints()?.0.iter().cloned().collect::<BTreeSet<_>>())
                .cloned()
                .collect();
        }

        Ok(base::Endpoints(endpoints))
    }

    fn status(&self) -> Result<base::Status, ManyError> {
        let mut attributes: BTreeSet<Attribute> = self
            .modules
            .iter()
            .filter_map(|m| m.info().attribute.clone())
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
                return Err(ManyError::internal_server_error());
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
            .map_err(|x| ManyError::unknown(x.to_string()))
    }
}

#[async_trait]
impl LowLevelManyRequestHandler for Arc<Mutex<ManyServer>> {
    async fn execute(&self, envelope: CoseSign1) -> Result<CoseSign1, String> {
        let request = {
            let this = self.lock().unwrap();
            crate::message::decode_request_from_cose_sign1(
                envelope.clone(),
                this.allowed_origins.clone(),
            )
        };
        let mut id = None;

        let response = {
            let this = self.lock().unwrap();
            let cose_id = this.identity.clone();

            request
                .and_then(|message| {
                    id = message.id;
                    _validate_time(&message, SystemTime::now(), this.timeout)?;
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
                        m.validate_envelope(&envelope, &message)?;
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
                .map_err(|many_err| ResponseMessage::error(&cose_id.identity, id, many_err))
        };

        match response {
            Ok((cose_id, message, maybe_module, fallback)) => match (maybe_module, fallback) {
                (Some(m), _) => {
                    let mut response = match m.execute(message).await {
                        Ok(response) => response,
                        Err(many_err) => ResponseMessage::error(&cose_id.identity, id, many_err),
                    };
                    response.from = cose_id.identity;
                    crate::message::encode_cose_sign1_from_response(response, &cose_id)
                }
                (None, Some(fb)) => {
                    LowLevelManyRequestHandler::execute(fb.as_ref(), envelope).await
                }
                (None, None) => {
                    let response = ResponseMessage::error(
                        &cose_id.identity,
                        id,
                        ManyError::could_not_route_message(),
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

#[cfg(test)]
mod tests {
    use semver::{BuildMetadata, Prerelease, Version};
    use std::time::Duration;

    use super::*;
    use crate::cose_helpers::public_key;
    use crate::message::{
        decode_response_from_cose_sign1, encode_cose_sign1_from_request, RequestMessage,
        RequestMessageBuilder,
    };
    use crate::server::module::base::Status;
    use crate::types::identity::cose::testsutils::generate_random_eddsa_identity;
    use crate::Identity;
    use proptest::prelude::*;

    const ALPHA_NUM_DASH_REGEX: &str = "[a-zA-Z0-9-]";

    prop_compose! {
        fn arb_semver()((major, minor, patch) in (any::<u64>(), any::<u64>(), any::<u64>()), pre in ALPHA_NUM_DASH_REGEX, build in ALPHA_NUM_DASH_REGEX) -> Version {
            Version {
                major,
                minor,
                patch,
                pre: Prerelease::new(&pre).unwrap(),
                build: BuildMetadata::new(&build).unwrap(),
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10))]
        #[test]
        fn simple_status(name in "\\PC*", version in arb_semver()) {
            let id = generate_random_eddsa_identity();
            let server = ManyServer::simple(name.clone(), id.clone(), Some(version.to_string()), None);

            // Test status() using a message instead of a direct call
            //
            // This will test other ManyServer methods as well
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(1)
                .from(id.identity)
                .to(id.identity)
                .method("status".to_string())
                .data("null".as_bytes().to_vec())
                .build()
                .unwrap();

            let envelope = encode_cose_sign1_from_request(request, &id).unwrap();
            let response = smol::block_on(async { server.execute(envelope).await }).unwrap();
            let response_message = decode_response_from_cose_sign1(response, None).unwrap();

            let status: Status = minicbor::decode(&response_message.data.unwrap()).unwrap();

            assert_eq!(status.version, 1);
            assert_eq!(status.name, name);
            assert_eq!(status.public_key, Some(public_key(&id.key.unwrap()).unwrap()));
            assert_eq!(status.identity, id.identity);
            assert!(status.attributes.has_id(0));
            assert_eq!(status.server_version, Some(version.to_string()));
            assert_eq!(status.timeout, Some(MANYSERVER_DEFAULT_TIMEOUT));
            assert_eq!(status.extras, BTreeMap::new());
        }
    }

    #[test]
    fn validate_time() {
        let timestamp = SystemTime::now();
        let request: RequestMessage = RequestMessageBuilder::default()
            .version(1)
            .from(Identity::anonymous())
            .to(Identity::anonymous())
            .method("status".to_string())
            .data("null".as_bytes().to_vec())
            .timestamp(timestamp)
            .build()
            .unwrap();

        // Okay with the same
        assert!(_validate_time(&request, timestamp, 100).is_ok());
        // Okay with the past
        assert!(_validate_time(&request, timestamp - Duration::from_secs(10), 100).is_ok());
        // Okay with the future
        assert!(_validate_time(&request, timestamp + Duration::from_secs(10), 100).is_ok());

        // NOT okay with the past too much
        assert!(_validate_time(&request, timestamp - Duration::from_secs(101), 100).is_err());
        // NOT okay with the future too much
        assert!(_validate_time(&request, timestamp + Duration::from_secs(101), 100).is_err());
    }
}
