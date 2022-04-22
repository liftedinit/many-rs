use crate::cbor::CborAny;
use crate::cose_helpers::public_key;
use crate::protocol::attributes::AttributeSet;
use crate::{Identity, ManyError};
use coset::{CborSerializable, CoseKey};
use derive_builder::Builder;
use many_macros::many_module;
use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Decode, Encode)]
#[cbor(transparent)]
pub struct Endpoints(#[n(0)] pub BTreeSet<String>);

#[derive(Clone, Debug, Builder)]
pub struct Status {
    pub version: u8,
    pub name: String,
    #[builder(setter(into, strip_option), default)]
    pub public_key: Option<CoseKey>,
    pub identity: Identity,
    pub attributes: AttributeSet,
    #[builder(setter(into, strip_option), default)]
    pub server_version: Option<String>,

    #[builder(setter(into, strip_option), default)]
    pub timeout: Option<u64>,

    #[builder(default)]
    pub extras: BTreeMap<String, CborAny>,
}

impl Status {
    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        minicbor::to_vec(self).map_err(|e| format!("{}", e))
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| format!("{}", e))
    }
}

// TODO: MISSING ENTRIES!!
impl Encode for Status {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        #[rustfmt::skip]
        e.begin_map()?
            .u8(0)?.u8(self.version)?
            .u8(1)?.str(self.name.as_str())?;

        if let Some(ref pk) = self.public_key {
            e.u8(2)?.bytes(&public_key(pk).unwrap().to_vec().unwrap())?;
        }

        e.u8(3)?
            .encode(&self.identity)?
            .u8(4)?
            .encode(&self.attributes)?;

        if let Some(ref sv) = self.server_version {
            e.u8(5)?.str(sv.as_str())?;
        }

        if let Some(ref timeout) = self.timeout {
            e.u8(7)?.encode(timeout)?;
        }

        for (k, v) in &self.extras {
            e.str(k.as_str())?.encode(v)?;
        }

        e.end()?;

        Ok(())
    }
}

impl<'b> Decode<'b> for Status {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        let mut builder = StatusBuilder::default();
        let len = d.map()?;
        let mut i = 0;
        let mut extras = BTreeMap::new();

        loop {
            match d.datatype()? {
                Type::Break => {
                    d.skip()?;
                    break;
                }
                Type::U8 | Type::U16 | Type::U32 | Type::U64 => {
                    match d.u8()? {
                        0 => builder.version(d.decode()?),
                        1 => builder.name(d.decode()?),
                        2 => {
                            let bytes = d.bytes()?;
                            let key: CoseKey = CoseKey::from_slice(bytes).map_err(|_e| {
                                minicbor::decode::Error::Message("Invalid cose key.")
                            })?;
                            builder.public_key(key)
                        }
                        3 => builder.identity(d.decode()?),
                        4 => builder.attributes(d.decode()?),
                        5 => builder.server_version(d.decode::<String>()?),
                        7 => builder.timeout(d.decode::<u64>()?),
                        _ => &mut builder,
                    };
                }
                Type::String | Type::StringIndef => {
                    let k = d.str_iter()?.collect::<Result<Vec<_>, _>>()?.join("");
                    let v: CborAny = d.decode()?;
                    extras.insert(k, v);
                }
                _ => {}
            }

            i += 1;
            if len.map_or(false, |x| i >= x) {
                break;
            }
        }

        builder
            .extras(extras)
            .build()
            .map_err(|_e| minicbor::decode::Error::Message("could not build"))
    }
}

#[many_module(name = BaseModule, id = 0, many_crate = crate)]
pub trait BaseModuleBackend: Send {
    fn endpoints(&self) -> Result<Endpoints, ManyError>;
    fn heartbeat(&self) -> Result<(), ManyError> {
        Ok(())
    }
    fn status(&self) -> Result<Status, ManyError>;
}

// TODO: Refactor those with `call_method()` from the Account PR
#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{BaseModule, Status};
    use crate::{
        message::{RequestMessage, RequestMessageBuilder, ResponseMessage},
        server::{module::ManyModuleInfo, tests::execute_request, MANYSERVER_DEFAULT_TIMEOUT},
        types::identity::{cose::tests::generate_random_eddsa_identity, CoseKeyIdentity},
        ManyModule, ManyServer,
    };
    use proptest::prelude::*;

    const SERVER_VERSION: u8 = 1;

    prop_compose! {
        /// Generate MANY server with arbitrary name composed of arbitrary non-control characters.
        fn arb_server()(name in "\\PC*") -> (CoseKeyIdentity, Arc<Mutex<ManyServer>>, String, ManyModuleInfo) {
            let id = generate_random_eddsa_identity();
            let server = ManyServer::new(name.clone(), id.clone());
            let base_module = BaseModule::new(server.clone());
            let module_info = base_module.info().clone();

            {
                let mut s = server.lock().unwrap();
                s.version = Some(SERVER_VERSION.to_string());
                s.add_module(base_module);
            }

            (id, server, name, module_info)
        }
    }

    proptest! {
        #[test]
        fn status((id, server, name, module_info) in arb_server()) {
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(SERVER_VERSION)
                .from(id.identity)
                .to(id.identity)
                .method("status".to_string())
                .data("null".as_bytes().to_vec())
                .build()
                .unwrap();

            let response_message = execute_request(id.clone(), server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            let bytes = response_message.data.unwrap();
            let status: Status = minicbor::decode(&bytes).unwrap();

            assert_eq!(status.version, SERVER_VERSION);
            assert_eq!(status.name, name);
            assert_eq!(status.public_key, id.public_key());
            assert_eq!(status.identity, id.identity);
            assert!(status.attributes.has_id(module_info.attribute.id));
            assert_eq!(status.server_version, Some(SERVER_VERSION.to_string()));
            assert_eq!(status.timeout, Some(MANYSERVER_DEFAULT_TIMEOUT));

            let status_bytes = status.to_bytes().unwrap();
            assert_eq!(status_bytes, bytes);

            let status_2 = Status::from_bytes(&status_bytes).unwrap();
            assert_eq!(status.version, status_2.version);
            assert_eq!(status.name, status_2.name);
            assert_eq!(status.public_key, status_2.public_key);
            assert_eq!(status.identity, status_2.identity);
            assert!(status_2.attributes.has_id(module_info.attribute.id));
            assert_eq!(status.server_version, status_2.server_version);
            assert_eq!(status.timeout, status_2.timeout);
        }

        #[test]
        fn endpoints((id, server, _, module_info) in arb_server()) {
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(SERVER_VERSION)
                .from(id.identity)
                .to(id.identity)
                .method("endpoints".to_string())
                .data("null".as_bytes().to_vec())
                .build()
                .unwrap();

            let response_message = execute_request(id, server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            let bytes = response_message.data.unwrap();
            let endpoints: Vec<String> = minicbor::decode(&bytes).unwrap();

            assert_eq!(module_info.endpoints, endpoints);
        }

        #[test]
        fn heartbeat((id, server, _, _) in arb_server()) {
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(SERVER_VERSION)
                .from(id.identity)
                .to(id.identity)
                .method("heartbeat".to_string())
                .data("null".as_bytes().to_vec())
                .build()
                .unwrap();

            let response_message = execute_request(id, server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            assert!(response_message.data.is_ok());
        }
    }
}
