use crate::{
    error,
    storage::{AclMap, KvStoreStorage},
};
use many_error::{ManyError, Reason};
use many_identity::Address;
use many_modules::abci_backend::{
    AbciBlock, AbciCommitInfo, AbciInfo, AbciInit, BeginBlockReturn, EndpointInfo, InitChainReturn,
    ManyAbciModuleBackend,
};
use many_modules::account::Role;
use many_modules::kvstore::{
    DisableArgs, DisableReturn, GetArgs, GetReturns, InfoArg, InfoReturns,
    KvStoreCommandsModuleBackend, KvStoreModuleBackend, KvStoreTransferModuleBackend, PutArgs,
    PutReturn, QueryArgs, QueryReturns, TransferArgs, TransferReturn,
};
use many_types::{Either, Timestamp};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::path::Path;
use tracing::info;

pub mod account;
pub mod allow_addrs;
mod event;

// The initial state schema, loaded from JSON.
#[derive(serde::Deserialize, Debug, Default)]
pub struct InitialStateJson {
    acl: AclMap,
    identity: Address,
    hash: Option<String>,
}

/// A simple kv-store.
#[derive(Debug)]
pub struct KvStoreModuleImpl {
    storage: KvStoreStorage,
}

/// The KvStoreMetadata mimics the QueryReturns structure but adds serde capabilities
#[derive(Clone, Debug, minicbor::Encode, minicbor::Decode, serde::Deserialize)]
#[serde(remote = "QueryReturns")]
#[cbor(map)]
pub struct KvStoreMetadata {
    #[n(0)]
    pub owner: Address,

    #[n(1)]
    #[serde(skip_deserializing)]
    pub disabled: Option<Either<bool, Reason<u64>>>,
}

#[derive(Debug, serde::Deserialize, minicbor::Encode, minicbor::Decode)]
#[serde(transparent)]
#[cbor(transparent)]
pub struct KvStoreMetadataWrapper(
    #[serde(with = "KvStoreMetadata")]
    #[n(0)]
    QueryReturns,
);

impl KvStoreModuleImpl {
    pub fn load<P: AsRef<Path>>(
        persistent_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let storage =
            KvStoreStorage::load(persistent_store_path, blockchain).map_err(ManyError::unknown)?;

        Ok(Self { storage })
    }

    pub fn new<P: AsRef<Path>>(
        initial_state: InitialStateJson,
        persistence_store_path: P,
        blockchain: bool,
    ) -> Result<Self, ManyError> {
        let storage = KvStoreStorage::new(
            initial_state.acl,
            initial_state.identity,
            persistence_store_path,
            blockchain,
        )
        .map_err(ManyError::unknown)?;

        if let Some(h) = initial_state.hash {
            // Verify the hash.
            let actual = hex::encode(storage.hash());
            if actual != h {
                return Err(error::invalid_initial_hash(h, actual));
            }
        }

        info!(
            height = storage.get_height(),
            hash = hex::encode(storage.hash()).as_str()
        );

        Ok(Self { storage })
    }
}

// This module is always supported, but will only be added when created using an ABCI
// flag.
impl ManyAbciModuleBackend for KvStoreModuleImpl {
    #[rustfmt::skip]
    fn init(&mut self) -> Result<AbciInit, ManyError> {
        Ok(AbciInit {
            endpoints: BTreeMap::from([
                ("kvstore.info".to_string(), EndpointInfo { is_command: false }),
                ("kvstore.get".to_string(), EndpointInfo { is_command: false }),
                ("kvstore.query".to_string(), EndpointInfo { is_command: false }),
                ("kvstore.put".to_string(), EndpointInfo { is_command: true }),
                ("kvstore.disable".to_string(), EndpointInfo { is_command: true }),
                ("kvstore.transfer".to_string(), EndpointInfo { is_command: true }),

                // Accounts
                ("account.create".to_string(), EndpointInfo { is_command: true }),
                ("account.setDescription".to_string(), EndpointInfo { is_command: true }),
                ("account.listRoles".to_string(), EndpointInfo { is_command: false }),
                ("account.getRoles".to_string(), EndpointInfo { is_command: false }),
                ("account.addRoles".to_string(), EndpointInfo { is_command: true }),
                ("account.removeRoles".to_string(), EndpointInfo { is_command: true }),
                ("account.info".to_string(), EndpointInfo { is_command: false }),
                ("account.disable".to_string(), EndpointInfo { is_command: true }),
                ("account.addFeatures".to_string(), EndpointInfo { is_command: true }),

                // Events
                ("events.info".to_string(), EndpointInfo { is_command: false }),
                ("events.list".to_string(), EndpointInfo { is_command: false }),
            ]),
        })
    }

    fn init_chain(&mut self) -> Result<InitChainReturn, ManyError> {
        info!("abci.init_chain()",);
        Ok(InitChainReturn {})
    }

    fn begin_block(&mut self, info: AbciBlock) -> Result<BeginBlockReturn, ManyError> {
        let time = info.time;
        info!(
            "abci.block_begin(): time={:?} curr_height={}",
            time,
            self.storage.get_height()
        );

        if let Some(time) = time {
            let time = Timestamp::new(time)?;
            self.storage.set_time(time);
        }

        Ok(BeginBlockReturn {})
    }

    fn info(&self) -> Result<AbciInfo, ManyError> {
        let storage = &self.storage;

        info!(
            "abci.info(): height={} hash={}",
            storage.get_height(),
            hex::encode(storage.hash()).as_str()
        );
        Ok(AbciInfo {
            height: storage.get_height(),
            hash: storage.hash().into(),
        })
    }

    fn commit(&mut self) -> Result<AbciCommitInfo, ManyError> {
        let result = self.storage.commit();

        info!(
            "abci.commit(): retain_height={} hash={}",
            result.retain_height,
            hex::encode(result.hash.as_slice()).as_str()
        );
        Ok(result)
    }
}

impl KvStoreModuleBackend for KvStoreModuleImpl {
    fn info(&self, _sender: &Address, _args: InfoArg) -> Result<InfoReturns, ManyError> {
        // Hash the storage.
        let hash = self.storage.hash();

        Ok(InfoReturns { hash: hash.into() })
    }

    fn get(&self, _sender: &Address, args: GetArgs) -> Result<GetReturns, ManyError> {
        let value = self.storage.get(&args.key)?;
        Ok(GetReturns {
            value: value.map(|x| x.into()),
        })
    }

    fn query(&self, _sender: &Address, args: QueryArgs) -> Result<QueryReturns, ManyError> {
        minicbor::decode(
            &self
                .storage
                .get_metadata(&args.key)?
                .ok_or_else(error::key_not_found)?,
        )
        .map_err(|e| ManyError::deserialization_error(e.to_string()))
    }
}

impl KvStoreCommandsModuleBackend for KvStoreModuleImpl {
    fn put(&mut self, sender: &Address, args: PutArgs) -> Result<PutReturn, ManyError> {
        let key: Vec<u8> = args.key.into();
        let owner = if let Some(alternative_owner) = args.alternative_owner {
            self.validate_alternative_owner(
                sender,
                &alternative_owner,
                [Role::CanKvStorePut, Role::Owner],
            )?;
            alternative_owner
        } else {
            *sender
        };

        self.verify_acl(&owner, key.clone())?;

        let meta = KvStoreMetadata {
            owner,
            disabled: Some(Either::Left(false)),
        };
        self.storage.put(&meta, &key, args.value.into())?;
        Ok(PutReturn {})
    }

    fn disable(&mut self, sender: &Address, args: DisableArgs) -> Result<DisableReturn, ManyError> {
        if self.storage.get(&args.key)?.is_none() {
            return Err(error::cannot_disable_empty_key());
        }
        let key: Vec<u8> = args.key.into();
        let owner = if let Some(ref alternative_owner) = args.alternative_owner {
            self.validate_alternative_owner(
                sender,
                alternative_owner,
                [Role::CanKvStoreDisable, Role::Owner],
            )?;
            alternative_owner
        } else {
            sender
        };

        self.verify_acl(owner, key.clone())?;

        let maybe_reason = if let Some(reason) = args.reason {
            Either::Right(reason)
        } else {
            Either::Left(true)
        };

        let meta = KvStoreMetadata {
            owner: *owner,
            disabled: Some(maybe_reason),
        };

        self.storage.disable(&meta, &key)?;
        Ok(DisableReturn {})
    }
}

impl KvStoreTransferModuleBackend for KvStoreModuleImpl {
    fn transfer(
        &mut self,
        sender: &Address,
        args: TransferArgs,
    ) -> Result<TransferReturn, ManyError> {
        if self.storage.get(&args.key)?.is_none() {
            return Err(error::key_not_found());
        }
        if args.new_owner.is_anonymous() {
            return Err(error::anon_alt_denied());
        }

        let key: Vec<u8> = args.key.into();
        let metadata: KvStoreMetadata = minicbor::decode(
            &self
                .storage
                .get_metadata(&key)?
                .ok_or_else(error::key_not_found)?,
        )
        .map_err(|e| ManyError::deserialization_error(e.to_string()))?;

        let owner = if let Some(ref alternative_owner) = args.alternative_owner {
            self.validate_alternative_owner(
                sender,
                alternative_owner,
                [Role::CanKvStoreTransfer, Role::Owner],
            )?;
            alternative_owner
        } else {
            sender
        };

        self.verify_acl(owner, key.clone())?;

        // We allow transferring a disabled key, and keep the same reason.
        let meta = KvStoreMetadata {
            owner: args.new_owner,
            disabled: metadata.disabled,
        };
        self.storage.transfer(&key, *owner, meta)?;

        Ok(TransferReturn {})
    }
}
