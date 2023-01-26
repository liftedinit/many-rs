use crate::error;
use crate::storage::{key_for_account_balance, LedgerStorage};
use many_error::ManyError;
use many_identity::Address;
use many_protocol::context::Context;
use many_types::{
    ledger::{Symbol, TokenAmount},
    ProofOperation,
};
use merk::{
    proofs::{
        Decoder,
        Node::{Hash, KVHash, KV},
        Op::{Child, Parent, Push},
        Query,
    },
    BatchEntry, Op,
};
use std::collections::{BTreeMap, BTreeSet};

impl LedgerStorage {
    pub fn with_balances(
        mut self,
        symbols: &BTreeMap<Symbol, String>,
        initial_balances: &BTreeMap<Address, BTreeMap<Symbol, TokenAmount>>,
    ) -> Result<Self, ManyError> {
        let mut batch: Vec<BatchEntry> = Vec::new();
        for (k, v) in initial_balances.iter() {
            for (symbol, tokens) in v.iter() {
                if !symbols.contains_key(symbol) {
                    return Err(ManyError::unknown(format!(
                        r#"Unknown symbol "{symbol}" for identity {k}"#
                    ))); // TODO: Custom error
                }

                let key = key_for_account_balance(k, symbol);
                batch.push((key, Op::Put(tokens.to_vec())));
            }
        }

        self.persistent_store
            .apply(batch.as_slice())
            .map_err(error::storage_apply_failed)?;

        Ok(self)
    }

    fn get_all_balances(
        &self,
        identity: &Address,
        context: impl AsRef<Context>,
    ) -> Result<BTreeMap<Symbol, TokenAmount>, ManyError> {
        Ok(if identity.is_anonymous() {
            // Anonymous cannot hold funds.
            BTreeMap::new()
        } else {
            let mut result = BTreeMap::new();
            let mut query = Query::new();
            for symbol in self.get_symbols()? {
                let key = key_for_account_balance(identity, &symbol);
                self.persistent_store
                    .get(&key)
                    .map_err(error::storage_get_failed)?
                    .map(|value| result.insert(symbol, TokenAmount::from(value)))
                    .map(|_| ())
                    .unwrap_or_default();
                query.insert_key(key)
            }
            context
                .as_ref()
                .prove(|| {
                    self.persistent_store
                        .prove(query)
                        .and_then(|proof| {
                            Decoder::new(proof.as_slice())
                                .map(|fallible_operation| {
                                    fallible_operation.map(|operation| match operation {
                                        Child => ProofOperation::Child,
                                        Parent => ProofOperation::Parent,
                                        Push(Hash(hash)) => ProofOperation::NodeHash(hash.to_vec()),
                                        Push(KV(key, value)) => {
                                            ProofOperation::KeyValuePair(key.into(), value.into())
                                        }
                                        Push(KVHash(hash)) => {
                                            ProofOperation::KeyValueHash(hash.to_vec())
                                        }
                                    })
                                })
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .map_err(|error| ManyError::unknown(error.to_string()))
                })
                .map(|error| Err(ManyError::unknown(error.to_string())))
                .unwrap_or(Ok(()))?;

            result
        })
    }

    pub fn get_multiple_balances(
        &self,
        identity: &Address,
        symbols: &BTreeSet<Symbol>,
        context: impl AsRef<Context>,
    ) -> Result<BTreeMap<Symbol, TokenAmount>, ManyError> {
        let balances = self.get_all_balances(identity, context)?;
        Ok(if symbols.is_empty() {
            balances
        } else {
            balances
                .into_iter()
                .filter(|(k, _v)| symbols.contains(k))
                .collect()
        })
    }
}
