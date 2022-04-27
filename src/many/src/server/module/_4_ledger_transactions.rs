use crate::ManyError;
use many_macros::many_module;

mod info;
mod list;

pub use info::*;
pub use list::*;

#[many_module(name = LedgerTransactionsModule, id = 4, namespace = ledger, many_crate = crate)]
pub trait LedgerTransactionsModuleBackend: Send {
    fn transactions(&self, args: TransactionsArgs) -> Result<TransactionsReturns, ManyError>;
    fn list(&self, args: ListArgs) -> Result<ListReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use minicbor::bytes::ByteVec;

    use std::{
        ops::Bound,
        sync::{Arc, Mutex},
    };

    use crate::{
        server::module::testutils::{call_module_cbor, call_module_cbor_diag},
        types::{
            identity::cose::tests::generate_random_eddsa_identity,
            ledger::{
                TokenAmount, Transaction, TransactionContent, TransactionId, TransactionKind,
            },
            CborRange, SortOrder, Timestamp, TransactionFilter, VecOrSingle,
        },
        Identity,
    };

    // TODO: Use derive?
    impl Clone for TransactionContent {
        fn clone(&self) -> Self {
            if let TransactionContent::Send {
                from,
                to,
                symbol,
                amount,
            } = self
            {
                TransactionContent::Send {
                    from: *from,
                    to: *to,
                    symbol: symbol.clone(),
                    amount: amount.clone(),
                }
            } else {
                todo!()
            }
        }
    }

    // TODO: Use derive?
    impl Clone for Transaction {
        fn clone(&self) -> Self {
            Transaction {
                id: self.id.clone(),
                time: self.time,
                content: self.content.clone(),
            }
        }
    }

    struct LedgerTransactionsImpl(Vec<Transaction>);
    impl std::default::Default for LedgerTransactionsImpl {
        fn default() -> Self {
            Self(vec![
                Transaction {
                    id: TransactionId(ByteVec::from(vec![1, 1, 1, 1])),
                    time: Timestamp::now(),
                    content: TransactionContent::Send {
                        from: Identity::anonymous(),
                        to: Identity::anonymous(),
                        symbol: "FOOBAR".to_string(),
                        amount: TokenAmount::from(1000u64),
                    },
                },
                Transaction {
                    id: TransactionId(ByteVec::from(vec![2, 2, 2, 2])),
                    time: Timestamp::now(),
                    content: TransactionContent::Send {
                        from: Identity::anonymous(),
                        to: Identity::anonymous(),
                        symbol: "BARFOO".to_string(),
                        amount: TokenAmount::from(5000u64),
                    },
                },
            ])
        }
    }

    impl LedgerTransactionsModuleBackend for LedgerTransactionsImpl {
        fn list(&self, args: ListArgs) -> Result<ListReturns, ManyError> {
            let count = args.count.unwrap_or(100) as usize;
            let mut transactions: Vec<Transaction> = match args.order {
                Some(SortOrder::Indeterminate) | Some(SortOrder::Ascending) | None => {
                    self.0.iter().take(count).cloned().collect()
                }
                Some(SortOrder::Descending) => self.0.iter().rev().take(count).cloned().collect(),
            };

            // TODO: This is a dumb implementation. We should use iterators.
            let transactions = match args.filter {
                Some(filter) => {
                    transactions = if let Some(account) = filter.account {
                        transactions
                            .iter()
                            .filter(|tx| match &tx.content {
                                TransactionContent::Send {
                                    from,
                                    to: _,
                                    symbol: _,
                                    amount: _,
                                } => account.0.iter().any(|id| id == from),
                                _ => false,
                            })
                            .cloned()
                            .collect()
                    } else {
                        transactions
                    };

                    transactions = if let Some(kind) = filter.kind {
                        transactions
                            .iter()
                            .filter(|tx| kind.0.iter().any(|k| &tx.kind() == k))
                            .cloned()
                            .collect()
                    } else {
                        transactions
                    };

                    transactions = if let Some(symbol) = filter.symbol {
                        transactions
                            .iter()
                            .filter(|tx| symbol.0.iter().any(|s| tx.symbol() == s))
                            .cloned()
                            .collect()
                    } else {
                        transactions
                    };

                    transactions = if let Some(id_range) = filter.id_range {
                        transactions
                            .iter()
                            .filter(|tx| id_range.contains(&tx.id))
                            .cloned()
                            .collect()
                    } else {
                        transactions
                    };

                    transactions = if let Some(date_range) = filter.date_range {
                        transactions
                            .iter()
                            .filter(|tx| {
                                println!("Tx time: {:?}", &tx.time);
                                println!("Start: {:?}", date_range.start);
                                println!("End: {:?}", date_range.end);
                                println!("Contain?: {}", date_range.contains(&tx.time));
                                date_range.contains(&tx.time)})
                            .cloned()
                            .collect()
                    } else {
                        transactions
                    };

                    transactions
                }
                None => transactions,
            };

            Ok(ListReturns {
                nb_transactions: transactions.len() as u64,
                transactions,
            })
        }

        fn transactions(&self, _args: TransactionsArgs) -> Result<TransactionsReturns, ManyError> {
            Ok(TransactionsReturns {
                nb_transactions: self.0.len() as u64,
            })
        }
    }

    #[test]
    fn list_ascending() {
        let module_impl = Arc::new(Mutex::new(LedgerTransactionsImpl::default()));
        let module = super::LedgerTransactionsModule::new(module_impl.clone());

        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Ascending),
            filter: None,
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 2);
        assert_eq!(
            list_returns.transactions[0].id,
            module_impl.lock().unwrap().0[0].id
        );
        assert!(list_returns.transactions[0].time < module_impl.lock().unwrap().0[0].time);

        // TODO: Check content
    }

    #[test]
    fn list_descending() {
        let module_impl = Arc::new(Mutex::new(LedgerTransactionsImpl::default()));
        let module = super::LedgerTransactionsModule::new(module_impl.clone());

        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Descending),
            filter: None,
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 2);
        assert_eq!(
            list_returns.transactions[0].id,
            module_impl.lock().unwrap().0[1].id
        );
        assert!(list_returns.transactions[0].time < module_impl.lock().unwrap().0[1].time);

        // TODO: Check content
    }

    #[test]
    fn list_indeterminate() {
        let module_impl = Arc::new(Mutex::new(LedgerTransactionsImpl::default()));
        let module = super::LedgerTransactionsModule::new(module_impl.clone());

        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Indeterminate),
            filter: None,
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 2);
        assert_eq!(
            list_returns.transactions[0].id,
            module_impl.lock().unwrap().0[0].id
        );
        assert!(list_returns.transactions[0].time < module_impl.lock().unwrap().0[0].time);

        // TODO: Check content
    }

    #[test]
    fn list_filter_account() {
        let module_impl = Arc::new(Mutex::new(LedgerTransactionsImpl::default()));
        let module = super::LedgerTransactionsModule::new(module_impl);

        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Indeterminate),
            filter: Some(TransactionFilter {
                account: Some(VecOrSingle::from(vec![Identity::anonymous()])),
                ..Default::default()
            }),
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 2);

        let id = generate_random_eddsa_identity();
        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Indeterminate),
            filter: Some(TransactionFilter {
                account: Some(VecOrSingle::from(vec![id.identity])),
                ..Default::default()
            }),
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 0);
    }

    #[test]
    fn list_filter_kind() {
        let module_impl = Arc::new(Mutex::new(LedgerTransactionsImpl::default()));
        let module = super::LedgerTransactionsModule::new(module_impl);

        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Indeterminate),
            filter: Some(TransactionFilter {
                kind: Some(VecOrSingle::from(vec![TransactionKind::Send])),
                ..Default::default()
            }),
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 2);

        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Indeterminate),
            filter: Some(TransactionFilter {
                kind: Some(VecOrSingle::from(vec![TransactionKind::Burn])),
                ..Default::default()
            }),
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 0);
    }

    #[test]
    fn list_filter_symbol() {
        let module_impl = Arc::new(Mutex::new(LedgerTransactionsImpl::default()));
        let module = super::LedgerTransactionsModule::new(module_impl);

        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Indeterminate),
            filter: Some(TransactionFilter {
                symbol: Some(VecOrSingle::from(vec!["FOOBAR".to_string()])),
                ..Default::default()
            }),
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 1);

        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Indeterminate),
            filter: Some(TransactionFilter {
                symbol: Some(VecOrSingle::from(vec![
                    "FOOBAR".to_string(),
                    "BARFOO".to_string(),
                ])),
                ..Default::default()
            }),
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 2);
    }

    #[test]
    fn list_filter_id_range() {
        let module_impl = Arc::new(Mutex::new(LedgerTransactionsImpl::default()));
        let module = super::LedgerTransactionsModule::new(module_impl);

        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Indeterminate),
            filter: Some(TransactionFilter {
                id_range: Some(CborRange {
                    start: Bound::Included(TransactionId::from(vec![1, 1, 1, 1])),
                    end: Bound::Included(TransactionId::from(vec![2, 2, 2, 2])),
                }),
                ..Default::default()
            }),
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 2);
        

        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::default()),
            filter: Some(TransactionFilter {
                id_range: Some(CborRange {
                    start: Bound::Included(TransactionId::from(vec![1, 1, 1, 1])),
                    end: Bound::Excluded(TransactionId::from(vec![2, 2, 2, 2])),
                }),
                ..Default::default()
            }),
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 1);
        

        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Indeterminate),
            filter: Some(TransactionFilter {
                id_range: Some(CborRange {
                    start: Bound::Excluded(TransactionId::from(vec![1, 1, 1, 1])),
                    end: Bound::Unbounded,
                }),
                ..Default::default()
            }),
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 1);


        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Indeterminate),
            filter: Some(TransactionFilter {
                id_range: Some(CborRange {
                    start: Bound::Unbounded,
                    end: Bound::Excluded(TransactionId::from(vec![2, 2, 2, 2])),
                }),
                ..Default::default()
            }),
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 1);


        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Indeterminate),
            filter: Some(TransactionFilter {
                id_range: Some(CborRange::default()),
                ..Default::default()
            }),
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 2);
    }

    #[test]
    fn list_filter_date_range() {
        let module_impl = Arc::new(Mutex::new(LedgerTransactionsImpl::default()));
        let module = super::LedgerTransactionsModule::new(module_impl);

        // TODO: The following test fails because MANY doesn't handle fractional
        // seconds properly We need to use floating-point values instead of
        // unsigned integer in Timestamp CBOR encoding/decoding
        //
        // let data = ListArgs {
        //     count: Some(100),
        //     order: Some(SortOrder::Indeterminate),
        //     filter: Some(TransactionFilter {
        //         date_range: Some(CborRange {
        //             start: Bound::Included(Timestamp::new(0).unwrap()),
        //             end: Bound::Included(Timestamp::now()),
        //         }),
        //         ..Default::default()
        //     }),
        // };
        // let data = minicbor::to_vec(data).unwrap();

        // let list_returns: ListReturns =
        //     minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        // assert_eq!(list_returns.nb_transactions, 2);


        let data = ListArgs {
            count: Some(100),
            order: Some(SortOrder::Indeterminate),
            filter: Some(TransactionFilter {
                date_range: Some(CborRange {
                    start: Bound::Included(Timestamp::new(0).unwrap()),
                    end: Bound::Included(Timestamp::new(1).unwrap()),
                }),
                ..Default::default()
            }),
        };
        let data = minicbor::to_vec(data).unwrap();

        let list_returns: ListReturns =
            minicbor::decode(&call_module_cbor(&module, "ledger.list", data).unwrap()).unwrap();

        assert_eq!(list_returns.nb_transactions, 0);
    }

    #[test]
    fn transactions() {
        let module_impl = Arc::new(Mutex::new(LedgerTransactionsImpl::default()));
        let module = super::LedgerTransactionsModule::new(module_impl);

        let transactions_returns: TransactionsReturns = minicbor::decode(
            &call_module_cbor_diag(&module, "ledger.transactions", "null").unwrap(),
        )
        .unwrap();

        assert_eq!(transactions_returns.nb_transactions, 2);
    }
}
