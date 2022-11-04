use crate::EmptyReturn;
use coset::{CborSerializable, CoseKey};
use derive_builder::Builder;
use many_identity::Address;
use many_macros::many_module;
use many_types::attributes::AttributeSet;
use many_types::cbor::CborAny;
use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use std::collections::{BTreeMap, BTreeSet};

use many_error::ManyError;
#[cfg(test)]
use mockall::{automock, predicate::*};

#[derive(Clone, Debug, Decode, Encode)]
#[cbor(transparent)]
pub struct Endpoints(#[n(0)] pub BTreeSet<String>);

// TODO: Move this in it's own file, like other modules
pub type HeartbeatReturn = EmptyReturn;

#[derive(Clone, Debug, Builder)]
pub struct Status {
    pub version: u8,
    pub name: String,
    #[builder(setter(into, strip_option), default)]
    pub public_key: Option<CoseKey>,
    pub identity: Address,
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
        minicbor::to_vec(self).map_err(|e| e.to_string())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        minicbor::decode(bytes).map_err(|e| e.to_string())
    }
}

impl<C> Encode<C> for Status {
    fn encode<W: Write>(&self, e: &mut Encoder<W>, _: &mut C) -> Result<(), Error<W::Error>> {
        #[rustfmt::skip]
        e.begin_map()?
            .u8(0)?.u8(self.version)?
            .u8(1)?.str(self.name.as_str())?;

        if let Some(ref pk) = self.public_key {
            e.u8(2)?.bytes(&pk.clone().to_vec().unwrap())?;
        }

        e.u8(3)?
            .encode(self.identity)?
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

impl<'b, C> Decode<'b, C> for Status {
    fn decode(d: &mut Decoder<'b>, _: &mut C) -> Result<Self, minicbor::decode::Error> {
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
                                minicbor::decode::Error::message("Invalid cose key.")
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
            .map_err(|_e| minicbor::decode::Error::message("could not build"))
    }
}

#[many_module(name = BaseModule, id = 0, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait BaseModuleBackend: Send {
    fn endpoints(&self) -> Result<Endpoints, ManyError>;
    fn heartbeat(&self) -> Result<HeartbeatReturn, ManyError> {
        Ok(HeartbeatReturn {})
    }
    fn status(&self) -> Result<Status, ManyError>;
}

#[cfg(test)]
mod tests {
    use crate::testutils::call_module;
    use many_identity::Identity;
    use many_identity_dsa::ed25519::generate_random_ed25519_identity;
    use many_types::attributes::Attribute;
    use std::sync::{Arc, Mutex};

    use super::*;
    #[test]
    fn status() {
        let id = generate_random_ed25519_identity();
        let address = id.address();
        let public_key = id.public_key();
        let mut mock = MockBaseModuleBackend::new();
        let status = Status {
            version: 1,
            name: "Foobar".to_string(),
            public_key: Some(public_key),
            identity: address,
            attributes: AttributeSet::from_iter(
                [Attribute {
                    id: 0,
                    arguments: vec![],
                }]
                .into_iter(),
            ),
            server_version: Some("1.0.0".to_string()),
            timeout: Some(300),
            extras: BTreeMap::new(),
        };
        mock.expect_status()
            .times(1)
            .return_const(Ok(status.clone()));
        let module = super::BaseModule::new(Arc::new(Mutex::new(mock)));
        let results: Status =
            minicbor::decode(&call_module(1, &module, "status", "null").unwrap()).unwrap();

        assert_eq!(status.version, results.version);
        assert_eq!(status.name, results.name);
        assert_eq!(status.public_key, results.public_key);
        assert_eq!(status.identity, results.identity);
        assert_eq!(status.attributes, results.attributes);
        assert_eq!(status.server_version, results.server_version);
        assert_eq!(status.timeout, results.timeout);

        let results = Status::from_bytes(&status.to_bytes().unwrap()).unwrap();
        assert_eq!(status.version, results.version);
        assert_eq!(status.name, results.name);
        assert_eq!(status.public_key, results.public_key);
        assert_eq!(status.identity, results.identity);
        assert_eq!(status.attributes, results.attributes);
        assert_eq!(status.server_version, results.server_version);
        assert_eq!(status.timeout, results.timeout);
    }

    #[test]
    fn endpoints() {
        let mut mock = MockBaseModuleBackend::new();
        let endpoints = Endpoints(BTreeSet::from_iter(
            [
                "status".to_string(),
                "endpoints".to_string(),
                "heartbeat".to_string(),
            ]
            .into_iter(),
        ));
        mock.expect_endpoints()
            .times(1)
            .return_const(Ok(endpoints.clone()));
        let module = super::BaseModule::new(Arc::new(Mutex::new(mock)));
        let results: Endpoints =
            minicbor::decode(&call_module(1, &module, "endpoints", "null").unwrap()).unwrap();

        assert_eq!(endpoints.0, results.0);
    }

    #[test]
    fn heartbeat() {
        let mut mock = MockBaseModuleBackend::new();
        mock.expect_heartbeat()
            .times(1)
            .returning(|| Ok(HeartbeatReturn {}));
        let module = super::BaseModule::new(Arc::new(Mutex::new(mock)));
        let _: HeartbeatReturn =
            minicbor::decode(&call_module(1, &module, "heartbeat", "null").unwrap()).unwrap();
    }
}
