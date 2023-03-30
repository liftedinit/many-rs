use {
    super::{InnerStorage, Operation, Query},
    crate::error,
    crate::storage::{key_for_account_balance, LedgerStorage, IDENTITY_ROOT, SYMBOLS_ROOT},
    many_error::ManyError,
    many_identity::Address,
    many_protocol::context::Context,
    many_types::{
        ledger::{Symbol, TokenAmount},
        ProofOperation,
    },
    std::collections::{BTreeMap, BTreeSet},
};

impl LedgerStorage {
    pub fn with_balances(
        mut self,
        identity: &Address,
        symbols: &BTreeMap<Symbol, String>,
        initial_balances: &BTreeMap<Address, BTreeMap<Symbol, TokenAmount>>,
    ) -> Result<Self, ManyError> {
        let mut batch = Vec::new();
        match self.persistent_store {
            InnerStorage::V1(_) => {
                for (k, v) in initial_balances.iter() {
                    for (symbol, tokens) in v.iter() {
                        if !symbols.contains_key(symbol) {
                            return Err(ManyError::unknown(format!(
                                r#"Unknown symbol "{symbol}" for identity {k}"#
                            ))); // TODO: Custom error
                        }

                        let key = key_for_account_balance(k, symbol);
                        batch.push((key, Operation::from(merk_v1::Op::Put(tokens.to_vec()))));
                    }
                }

                batch.push((
                    IDENTITY_ROOT.as_bytes().to_vec(),
                    Operation::from(merk_v1::Op::Put(identity.to_vec())),
                ));
                batch.push((
                    SYMBOLS_ROOT.as_bytes().to_vec(),
                    Operation::from(merk_v1::Op::Put(
                        minicbor::to_vec(symbols).map_err(ManyError::serialization_error)?,
                    )),
                ));
            }
            InnerStorage::V2(_) => {
                for (k, v) in initial_balances.iter() {
                    for (symbol, tokens) in v.iter() {
                        if !symbols.contains_key(symbol) {
                            return Err(ManyError::unknown(format!(
                                r#"Unknown symbol "{symbol}" for identity {k}"#
                            ))); // TODO: Custom error
                        }

                        let key = key_for_account_balance(k, symbol);
                        batch.push((key, Operation::from(merk_v2::Op::Put(tokens.to_vec()))));
                    }
                }

                batch.push((
                    IDENTITY_ROOT.as_bytes().to_vec(),
                    Operation::from(merk_v2::Op::Put(identity.to_vec())),
                ));
                batch.push((
                    SYMBOLS_ROOT.as_bytes().to_vec(),
                    Operation::from(merk_v2::Op::Put(
                        minicbor::to_vec(symbols).map_err(ManyError::serialization_error)?,
                    )),
                ));
            }
        }

        self.persistent_store
            .apply(batch.as_slice())
            .map(|_| self)
            .map_err(Into::into)
    }

    fn get_all_balances(
        &self,
        identity: &Address,
    ) -> Result<
        (
            BTreeMap<Symbol, TokenAmount>,
            impl IntoIterator<Item = Vec<u8>>,
        ),
        ManyError,
    > {
        Ok(if identity.is_anonymous() {
            // Anonymous cannot hold funds.
            (BTreeMap::new(), vec![])
        } else {
            let mut result = BTreeMap::new();
            for symbol in self.get_symbols()? {
                self.persistent_store
                    .get(&key_for_account_balance(identity, &symbol))
                    .map_err(error::storage_get_failed)?
                    .map(|value| result.insert(symbol, TokenAmount::from(value)))
                    .map(|_| ())
                    .unwrap_or_default()
            }

            (
                result,
                self.get_symbols()?
                    .into_iter()
                    .map(|symbol| key_for_account_balance(identity, &symbol))
                    .collect(),
            )
        })
    }

    pub fn get_multiple_balances(
        &self,
        identity: &Address,
        symbols: &BTreeSet<Symbol>,
    ) -> Result<
        (
            BTreeMap<Symbol, TokenAmount>,
            impl IntoIterator<Item = Vec<u8>>,
        ),
        ManyError,
    > {
        self.get_all_balances(identity).map(|(balances, keys)| {
            (
                if symbols.is_empty() {
                    balances
                } else {
                    balances
                        .into_iter()
                        .filter(|(k, _v)| symbols.contains(k))
                        .collect()
                },
                keys,
            )
        })
    }

    pub fn prove_state(
        &self,
        context: impl AsRef<Context>,
        keys: impl IntoIterator<Item = Vec<u8>>,
    ) -> Result<(), ManyError> {
        context.as_ref().prove(|| {
            match self.persistent_store {
                InnerStorage::V1(_) => self
                    .persistent_store
                    .prove(Query::from(merk_v1::proofs::query::Query::from(
                        keys.into_iter()
                            .map(merk_v1::proofs::query::QueryItem::Key)
                            .collect::<Vec<_>>(),
                    )))
                    .and_then(|proof| {
                        merk_v1::proofs::Decoder::new(proof.as_slice())
                            .map(|fallible_operation| {
                                fallible_operation.map(|operation| match operation {
                                    merk_v1::proofs::Op::Child => ProofOperation::Child,
                                    merk_v1::proofs::Op::Parent => ProofOperation::Parent,
                                    merk_v1::proofs::Op::Push(merk_v1::proofs::Node::Hash(
                                        hash,
                                    )) => ProofOperation::NodeHash(hash.to_vec()),
                                    merk_v1::proofs::Op::Push(merk_v1::proofs::Node::KV(
                                        key,
                                        value,
                                    )) => ProofOperation::KeyValuePair(key.into(), value.into()),
                                    merk_v1::proofs::Op::Push(merk_v1::proofs::Node::KVHash(
                                        hash,
                                    )) => ProofOperation::KeyValueHash(hash.to_vec()),
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(Into::into)
                    }),
                InnerStorage::V2(_) => self
                    .persistent_store
                    .prove(Query::from(merk_v2::proofs::query::Query::from(
                        keys.into_iter()
                            .map(merk_v2::proofs::query::QueryItem::Key)
                            .collect::<Vec<_>>(),
                    )))
                    .and_then(|proof| {
                        merk_v2::proofs::Decoder::new(proof.as_slice())
                            .map(|fallible_operation| {
                                fallible_operation.map(|operation| match operation {
                                    merk_v2::proofs::Op::Child => ProofOperation::Child,
                                    merk_v2::proofs::Op::Parent => ProofOperation::Parent,
                                    merk_v2::proofs::Op::Push(merk_v2::proofs::Node::Hash(
                                        hash,
                                    )) => ProofOperation::NodeHash(hash.to_vec()),
                                    merk_v2::proofs::Op::Push(merk_v2::proofs::Node::KV(
                                        key,
                                        value,
                                    )) => ProofOperation::KeyValuePair(key.into(), value.into()),
                                    merk_v2::proofs::Op::Push(merk_v2::proofs::Node::KVHash(
                                        hash,
                                    )) => ProofOperation::KeyValueHash(hash.to_vec()),
                                })
                            })
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(Into::into)
                    }),
            }
            .map_err(|error| ManyError::unknown(error.to_string()))
        })
    }
}
