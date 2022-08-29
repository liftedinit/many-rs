use crate::transport::LowLevelManyRequestHandler;
use async_trait::async_trait;
use coset::{CoseKey, CoseSign1};
use many_error::ManyError;
use many_identity::{Identity, Verifier};
use many_modules::{base, ManyModule, ManyModuleInfo};
use many_protocol::{RequestMessage, ResponseMessage};
use many_types::attributes::Attribute;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Debug, Formatter};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

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
        .ok_or_else(|| ManyError::required_field_missing("timestamp".to_string()))?
        .as_system_time()?;

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

trait ManyServerFallback: LowLevelManyRequestHandler + base::BaseModuleBackend {}

impl<M: LowLevelManyRequestHandler + base::BaseModuleBackend + 'static> ManyServerFallback for M {}

#[derive(Debug, Clone)]
pub struct ManyModuleList {}

pub const MANYSERVER_DEFAULT_TIMEOUT: u64 = 300;

pub struct ManyServer {
    modules: Vec<Arc<dyn ManyModule + Send>>,
    method_cache: BTreeSet<String>,
    identity: Box<dyn Identity>,
    verifier: Box<dyn Verifier>,
    public_key: Option<CoseKey>,
    name: String,
    version: Option<String>,
    timeout: u64,
    fallback: Option<Arc<dyn ManyServerFallback + Send + 'static>>,

    time_fn: Option<Arc<dyn Fn() -> Result<SystemTime, ManyError> + Send + Sync>>,
}

impl ManyServer {
    /// Create a test server. This should never be used in prod.
    #[cfg(feature = "testing")]
    pub fn test(identity: impl Identity + 'static) -> Arc<Mutex<Self>> {
        Self::simple(
            "test-many-server",
            identity,
            many_identity::AcceptAllVerifier,
            None,
        )
    }

    pub fn simple(
        name: impl ToString,
        identity: impl Identity + 'static,
        verifier: impl Verifier + 'static,
        version: Option<String>,
    ) -> Arc<Mutex<Self>> {
        let public_key = identity.public_key();
        let s = Self::new(name, identity, verifier, public_key);
        {
            let mut s2 = s.lock().unwrap();
            s2.version = version;
            s2.add_module(base::BaseModule::new(s.clone()));
        }

        s
    }

    pub fn new<N: ToString>(
        name: N,
        identity: impl Identity + 'static,
        verifier: impl Verifier + 'static,
        public_key: Option<CoseKey>,
    ) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            modules: vec![],
            name: name.to_string(),
            identity: Box::new(identity),
            verifier: Box::new(verifier),
            public_key,
            timeout: MANYSERVER_DEFAULT_TIMEOUT,
            fallback: None,
            method_cache: Default::default(),
            version: None,
            time_fn: None,
        }))
    }

    pub fn set_timeout(&mut self, timeout_in_secs: u64) {
        self.timeout = timeout_in_secs;
    }

    pub fn set_time_fn<T>(&mut self, time_fn: T)
    where
        T: Fn() -> Result<SystemTime, ManyError> + Send + Sync + 'static,
    {
        self.time_fn = Some(Arc::new(time_fn));
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
        if to.is_anonymous() || &self.identity.address() == to {
            Ok(())
        } else {
            Err(ManyError::unknown_destination(
                to.to_string(),
                self.identity.address().to_string(),
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

impl Debug for ManyServer {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManyServer").finish()
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
            .identity(self.identity.address())
            .timeout(self.timeout)
            .extras(BTreeMap::new());

        if let Some(ref pk) = self.public_key {
            builder.public_key(pk.clone());
        }

        if let Some(sv) = self.version.clone() {
            builder.server_version(sv);
        }

        if let Some(fb) = &self.fallback {
            let fb_status = fb.status()?;
            if fb_status.identity != self.identity.address()
                || fb_status.version != 1
                || (fb_status.server_version != self.version && self.version.is_some())
            {
                tracing::error!(
                    "fallback status differs from internal status: {} != {} || {:?} != {:?}",
                    fb_status.identity,
                    self.identity.address(),
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
            many_protocol::decode_request_from_cose_sign1(&envelope, &this.verifier)
        };
        let mut id = None;

        let response = {
            let this = self.lock().unwrap();
            let address = this.identity.address();

            (|| {
                let message = request?;

                let now = this
                    .time_fn
                    .as_ref()
                    .map_or_else(|| Ok(SystemTime::now()), |f| f())?;

                id = message.id;

                _validate_time(&message, now, this.timeout)?;

                this.validate_id(&message)?;

                let maybe_module = this.find_module(&message);
                if let Some(ref m) = maybe_module {
                    m.validate(&message, &envelope)?;
                };

                Ok((address, message, maybe_module, this.fallback.clone()))
            })()
            .map_err(|many_err| ResponseMessage::error(address, id, many_err))
        };

        match response {
            Ok((address, message, maybe_module, fallback)) => match (maybe_module, fallback) {
                (Some(m), _) => {
                    let mut response = match m.execute(message).await {
                        Ok(response) => response,
                        Err(many_err) => ResponseMessage::error(address, id, many_err),
                    };
                    response.from = address;

                    let this = self.lock().unwrap();
                    many_protocol::encode_cose_sign1_from_response(response, &this.identity)
                        .map_err(|e| e.to_string())
                }
                (None, Some(fb)) => {
                    LowLevelManyRequestHandler::execute(fb.as_ref(), envelope).await
                }
                (None, None) => {
                    let this = self.lock().unwrap();
                    let identity = &this.identity;
                    let address = identity.address();

                    let response =
                        ResponseMessage::error(address, id, ManyError::could_not_route_message());
                    many_protocol::encode_cose_sign1_from_response(response, identity)
                        .map_err(|e| e.to_string())
                }
            },
            Err(response) => {
                let this = self.lock().unwrap();
                many_protocol::encode_cose_sign1_from_response(response, &this.identity)
                    .map_err(|e| e.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use semver::{BuildMetadata, Prerelease, Version};
    use std::sync::RwLock;
    use std::time::Duration;

    use super::*;
    use many_identity::{AcceptAllVerifier, Address, AnonymousIdentity};
    use many_identity_dsa::ed25519::generate_random_ed25519_identity;
    use many_modules::base::Status;
    use many_protocol::{
        decode_response_from_cose_sign1, encode_cose_sign1_from_request, RequestMessageBuilder,
    };
    use many_types::Timestamp;
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
        #[test]
        fn simple_status(name in "\\PC*", version in arb_semver()) {
            let server_id = generate_random_ed25519_identity();
            let id = generate_random_ed25519_identity();
            let server_address = server_id.address();
            let server_public_key = server_id.public_key();
            let server = ManyServer::simple(&name, server_id, AcceptAllVerifier, Some(version.to_string()));

            // Test status() using a message instead of a direct call
            //
            // This will test other ManyServer methods as well
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(1)
                .from(id.address())
                .to(server_address)
                .method("status".to_string())
                .data("null".as_bytes().to_vec())
                .build()
                .unwrap();

            let envelope = encode_cose_sign1_from_request(request, &id).unwrap();
            let response = smol::block_on(async { server.execute(envelope).await }).unwrap();
            let response_message = decode_response_from_cose_sign1(&response, None, &AcceptAllVerifier).unwrap();

            let status: Status = minicbor::decode(&response_message.data.unwrap()).unwrap();

            assert_eq!(status.version, 1);
            assert_eq!(status.name, name);
            assert_eq!(status.public_key, Some(server_public_key));
            assert_eq!(status.identity, server_address);
            assert!(status.attributes.has_id(0));
            assert_eq!(status.server_version, Some(version.to_string()));
            assert_eq!(status.timeout, Some(MANYSERVER_DEFAULT_TIMEOUT));
            assert_eq!(status.extras, BTreeMap::new());
        }
    }

    #[test]
    fn validate_from_anonymous_fail() {
        let request: RequestMessage = RequestMessageBuilder::default()
            .from(Address::anonymous())
            .to(Address::anonymous())
            .method("status".to_string())
            .build()
            .unwrap();
        let id = generate_random_ed25519_identity();
        let server = ManyServer::test(AnonymousIdentity);
        let envelope = encode_cose_sign1_from_request(request, &id).unwrap();
        let response_e = smol::block_on(server.execute(envelope)).unwrap();
        let response =
            decode_response_from_cose_sign1(&response_e, None, &AcceptAllVerifier).unwrap();
        assert!(response.data.is_err());
    }

    #[test]
    fn validate_from_different_fail() {
        let request: RequestMessage = RequestMessageBuilder::default()
            .from(generate_random_ed25519_identity().address())
            .to(Address::anonymous())
            .method("status".to_string())
            .build()
            .unwrap();
        let id = generate_random_ed25519_identity();
        let server = ManyServer::test(AnonymousIdentity);
        let envelope = encode_cose_sign1_from_request(request, &id).unwrap();
        let response_e = smol::block_on(server.execute(envelope)).unwrap();
        let response =
            decode_response_from_cose_sign1(&response_e, None, &AcceptAllVerifier).unwrap();
        assert!(response.data.is_err());
    }

    #[test]
    fn validate_time() {
        let timestamp = SystemTime::now();
        let request: RequestMessage = RequestMessageBuilder::default()
            .version(1)
            .from(Address::anonymous())
            .to(Address::anonymous())
            .method("status".to_string())
            .data("null".as_bytes().to_vec())
            .timestamp(Timestamp::from_system_time(timestamp).unwrap())
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

    #[test]
    fn server_manages_time() {
        fn create_request(timestamp: SystemTime, nonce: u8) -> CoseSign1 {
            let request: RequestMessage = RequestMessageBuilder::default()
                .method("status".to_string())
                .timestamp(Timestamp::from_system_time(timestamp).unwrap())
                .nonce(nonce.to_le_bytes().to_vec())
                .build()
                .unwrap();
            encode_cose_sign1_from_request(request, &AnonymousIdentity).unwrap()
        }

        let server = ManyServer::test(AnonymousIdentity);
        let timestamp = SystemTime::now();
        let now = Arc::new(RwLock::new(timestamp));
        let get_now = {
            let n = now.clone();
            move || Ok(*n.read().unwrap())
        };

        // timestamp is now, so this should be fairly close to it and should pass.
        let response_e = smol::block_on(server.execute(create_request(timestamp, 0))).unwrap();
        let response =
            decode_response_from_cose_sign1(&response_e, None, &AcceptAllVerifier).unwrap();
        assert!(response.data.is_ok());

        // Set time to present.
        {
            server.lock().unwrap().set_time_fn(get_now);
        }
        let response_e = smol::block_on(server.execute(create_request(timestamp, 1))).unwrap();
        let response =
            decode_response_from_cose_sign1(&response_e, None, &AcceptAllVerifier).unwrap();
        assert!(response.data.is_ok());

        // Set time to 10 minutes past.
        {
            *now.write().unwrap() = timestamp - Duration::from_secs(60 * 60 * 10);
        }
        let response_e = smol::block_on(server.execute(create_request(timestamp, 2))).unwrap();
        let response =
            decode_response_from_cose_sign1(&response_e, None, &AcceptAllVerifier).unwrap();
        assert!(response.data.is_err());
        assert_eq!(
            response.data.unwrap_err().code(),
            ManyError::timestamp_out_of_range().code()
        );

        // Set request timestamp 10 minutes in the past.
        let response_e = smol::block_on(server.execute(create_request(
            timestamp - Duration::from_secs(60 * 60 * 10),
            3,
        )))
        .unwrap();
        let response =
            decode_response_from_cose_sign1(&response_e, None, &AcceptAllVerifier).unwrap();
        assert!(response.data.is_ok());
    }
}
