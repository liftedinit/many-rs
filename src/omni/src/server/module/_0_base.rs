use crate::cbor::CborAny;
use crate::protocol::Attribute;
use crate::{Identity, OmniError};
use derive_builder::Builder;
use minicbor::data::Type;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
use minicose::CoseKey;
use omni_module::omni_module;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Decode, Encode)]
#[cbor(transparent)]
pub struct Endpoints(#[n(0)] pub BTreeSet<String>);

#[derive(Clone, Debug, Builder)]
pub struct Status {
    pub version: u8,
    pub name: String,
    #[builder(setter(into, strip_option))]
    pub public_key: Option<CoseKey>,
    pub identity: Identity,
    pub attributes: Vec<Attribute>,
    #[builder(setter(into, strip_option))]
    pub server_version: Option<String>,

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

impl Encode for Status {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        #[rustfmt::skip]
        e.begin_map()?
            .u8(0)?.u8(self.version)?
            .u8(1)?.str(self.name.as_str())?;

        if let Some(ref pk) = self.public_key {
            e.u8(2)?
                .bytes(&pk.to_public_key().unwrap().to_bytes().unwrap())?;
        }

        e.u8(3)?
            .encode(&self.identity)?
            .u8(4)?
            .encode(self.attributes.as_slice())?;

        if let Some(ref sv) = self.server_version {
            e.u8(5)?.str(sv.as_str())?;
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
                            let key: CoseKey = CoseKey::from_bytes(bytes).map_err(|_e| {
                                minicbor::decode::Error::Message("Invalid cose key.")
                            })?;
                            builder.public_key(key)
                        }
                        3 => builder.identity(d.decode()?),
                        4 => builder.attributes(d.decode()?),
                        5 => builder.server_version(d.decode::<String>()?),
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

#[omni_module(name = BaseModule, id = 0, omni_crate = crate)]
pub trait BaseModuleBackend: Send {
    fn endpoints(&self) -> Result<Endpoints, OmniError>;
    fn heartbeat(&self) -> Result<(), OmniError> {
        Ok(())
    }
    fn status(&self) -> Result<Status, OmniError>;
}

pub struct StaticBaseModuleImpl {
    endpoints: Endpoints,
    status: Status,
}

impl StaticBaseModuleImpl {
    pub fn module(endpoints: Endpoints, status: Status) -> BaseModule<StaticBaseModuleImpl> {
        BaseModule::new(Arc::new(Mutex::new(StaticBaseModuleImpl {
            endpoints,
            status,
        })))
    }
}

impl BaseModuleBackend for StaticBaseModuleImpl {
    fn endpoints(&self) -> Result<Endpoints, OmniError> {
        Ok(self.endpoints.clone())
    }

    fn status(&self) -> Result<Status, OmniError> {
        Ok(self.status.clone())
    }
}
