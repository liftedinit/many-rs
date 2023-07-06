use crate::_3_kvstore::list::ListArgs;
use crate::kvstore::list::ListReturns;
use many_error::ManyError;
use many_identity::Address;
use many_macros::many_module;
use minicbor::{decode, encode};

#[cfg(test)]
use mockall::{automock, predicate::*};

pub mod get;
pub mod info;
pub mod list;
pub mod query;
pub use get::*;
pub use info::*;
pub use query::*;

#[many_module(name = KvStoreModule, id = 3, namespace = kvstore, many_modules_crate = crate)]
#[cfg_attr(test, automock)]
pub trait KvStoreModuleBackend: Send {
    fn info(&self, sender: &Address, args: InfoArg) -> Result<InfoReturns, ManyError>;
    fn get(&self, sender: &Address, args: GetArgs) -> Result<GetReturns, ManyError>;
    fn query(&self, sender: &Address, args: QueryArgs) -> Result<QueryReturns, ManyError>;
    fn list(&self, sender: &Address, args: ListArgs) -> Result<ListReturns, ManyError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyFilterType {
    Owner(Address),
    PreviousOwner(Address),
    Disabled(bool),
}

impl std::str::FromStr for KeyFilterType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(2, ':');
        let tag = parts.next().ok_or_else(|| "missing tag".to_string())?;
        let value = parts.next().ok_or_else(|| "missing value".to_string())?;

        match tag {
            "owner" => {
                let address = value
                    .parse()
                    .map_err(|e| format!("invalid address: {}", e))?;
                Ok(KeyFilterType::Owner(address))
            }
            "previous_owner" => {
                let address = value
                    .parse()
                    .map_err(|e| format!("invalid address: {}", e))?;
                Ok(KeyFilterType::PreviousOwner(address))
            }
            "disabled" => {
                let disabled = value.parse().map_err(|e| format!("invalid bool: {}", e))?;
                Ok(KeyFilterType::Disabled(disabled))
            }
            _ => Err(format!("unknown tag: {}", tag)),
        }
    }
}

impl<C> minicbor::Encode<C> for KeyFilterType {
    fn encode<W: encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        _: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        match self {
            KeyFilterType::Owner(address) => e.array(2)?.u32(0)?.encode(address).map(|_| ()),
            KeyFilterType::PreviousOwner(address) => {
                e.array(2)?.u32(1)?.encode(address).map(|_| ())
            }
            KeyFilterType::Disabled(disabled) => e.array(2)?.u32(2)?.bool(*disabled).map(|_| ()),
        }
    }
}

impl<'b, C> minicbor::Decode<'b, C> for KeyFilterType {
    fn decode(d: &mut minicbor::Decoder<'b>, _: &mut C) -> Result<Self, decode::Error> {
        if d.array()? != Some(2) {
            return Err(decode::Error::message("array of length 2 expected"));
        }

        match d.u32()? {
            0 => {
                let address = d.decode()?;
                Ok(KeyFilterType::Owner(address))
            }
            1 => {
                let address = d.decode()?;
                Ok(KeyFilterType::PreviousOwner(address))
            }
            2 => {
                let disabled = d.decode()?;
                Ok(KeyFilterType::Disabled(disabled))
            }
            _ => Err(decode::Error::message("unexpected tag")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::{call_module, call_module_cbor};
    use many_identity::testing::identity;
    use minicbor::bytes::ByteVec;
    use mockall::predicate;
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};

    #[test]
    fn info() {
        let mut mock = MockKvStoreModuleBackend::new();
        mock.expect_info()
            .with(predicate::eq(identity(1)), predicate::eq(InfoArg {}))
            .times(1)
            .return_const(Ok(InfoReturns {
                hash: ByteVec::from(vec![9u8; 8]),
            }));
        let module = super::KvStoreModule::new(Arc::new(Mutex::new(mock)));
        let info_returns: InfoReturns =
            minicbor::decode(&call_module(1, &module, "kvstore.info", "null").unwrap()).unwrap();

        assert_eq!(info_returns.hash, ByteVec::from(vec![9u8; 8]));
    }

    #[test]
    fn get() {
        let data = GetArgs {
            key: ByteVec::from(vec![5, 6, 7]),
        };
        let mut mock = MockKvStoreModuleBackend::new();
        mock.expect_get()
            .with(predicate::eq(identity(1)), predicate::eq(data.clone()))
            .times(1)
            .returning(|_id, _args| {
                Ok(GetReturns {
                    value: Some(ByteVec::from(vec![1, 2, 3, 4])),
                })
            });
        let module = super::KvStoreModule::new(Arc::new(Mutex::new(mock)));

        let get_returns: GetReturns = minicbor::decode(
            &call_module_cbor(1, &module, "kvstore.get", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();

        assert_eq!(get_returns.value, Some(ByteVec::from(vec![1, 2, 3, 4])));
    }

    #[test]
    fn query() {
        let data = QueryArgs {
            key: ByteVec::from(vec![5, 6, 7]),
        };
        let mut mock = MockKvStoreModuleBackend::new();
        mock.expect_query()
            .with(predicate::eq(identity(1)), predicate::eq(data.clone()))
            .times(1)
            .returning(|_id, _args| {
                Ok(QueryReturns {
                    owner: identity(666),
                    disabled: None,
                    previous_owner: None,
                })
            });
        let module = super::KvStoreModule::new(Arc::new(Mutex::new(mock)));

        let query_returns: QueryReturns = minicbor::decode(
            &call_module_cbor(1, &module, "kvstore.query", minicbor::to_vec(data).unwrap())
                .unwrap(),
        )
        .unwrap();

        assert_eq!(query_returns.owner, identity(666));
    }

    #[test]
    fn list() {
        let mut mock = MockKvStoreModuleBackend::new();
        mock.expect_list().times(1).returning(|_id, _args| {
            Ok(ListReturns {
                keys: vec![vec![1].into(), vec![2].into()],
            })
        });
        let module = super::KvStoreModule::new(Arc::new(Mutex::new(mock)));

        let list_returns: ListReturns =
            minicbor::decode(&call_module(1, &module, "kvstore.list", "{}").unwrap()).unwrap();

        assert_eq!(list_returns.keys, vec![vec![1].into(), vec![2].into()]);
    }

    #[test]
    fn key_filter_type_from_str() {
        let key_filter_type = KeyFilterType::from_str("owner:maa").unwrap();
        assert_eq!(key_filter_type, KeyFilterType::Owner(Address::anonymous()));
        let key_filter_type = KeyFilterType::from_str("previous_owner:maiyg").unwrap();
        assert_eq!(
            key_filter_type,
            KeyFilterType::PreviousOwner(Address::illegal())
        );
        let key_filter_type = KeyFilterType::from_str("disabled:true").unwrap();
        assert_eq!(key_filter_type, KeyFilterType::Disabled(true));
        let key_filter_type = KeyFilterType::from_str("disabled:false").unwrap();
        assert_eq!(key_filter_type, KeyFilterType::Disabled(false));
    }

    #[test]
    fn key_filter_type_encode_decode() {
        let key_filter_type = KeyFilterType::Owner(Address::anonymous());
        let encoded = minicbor::to_vec(key_filter_type).unwrap();
        let decoded: KeyFilterType = minicbor::decode(&encoded).unwrap();
        assert_eq!(decoded, key_filter_type);
        let key_filter_type = KeyFilterType::PreviousOwner(Address::illegal());
        let encoded = minicbor::to_vec(key_filter_type).unwrap();
        let decoded: KeyFilterType = minicbor::decode(&encoded).unwrap();
        assert_eq!(decoded, key_filter_type);
        let key_filter_type = KeyFilterType::Disabled(true);
        let encoded = minicbor::to_vec(key_filter_type).unwrap();
        let decoded: KeyFilterType = minicbor::decode(&encoded).unwrap();
        assert_eq!(decoded, key_filter_type);
        let key_filter_type = KeyFilterType::Disabled(false);
        let encoded = minicbor::to_vec(key_filter_type).unwrap();
        let decoded: KeyFilterType = minicbor::decode(&encoded).unwrap();
        assert_eq!(decoded, key_filter_type);
    }
}
