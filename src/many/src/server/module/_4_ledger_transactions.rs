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
    use proptest::prelude::*;

    use std::sync::{Arc, Mutex};

    use crate::{
        message::RequestMessage,
        message::{RequestMessageBuilder, ResponseMessage},
        server::tests::execute_request,
        types::{
            identity::{cose::tests::generate_random_eddsa_identity, CoseKeyIdentity},
            ledger::{TokenAmount, Transaction, TransactionContent, TransactionId},
            SortOrder, Timestamp,
        },
        Identity, ManyServer,
    };

    const SERVER_VERSION: u8 = 1;

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
            let transactions: Vec<Transaction> = match args.order {
                Some(SortOrder::Indeterminate) | Some(SortOrder::Ascending) | None => {
                    self.0.iter().take(count).cloned().collect()
                }
                Some(SortOrder::Descending) => self.0.iter().rev().take(count).cloned().collect(),
            };

            // TODO: TransactionFilter

            Ok(ListReturns {
                nb_transactions: self.0.len() as u64,
                transactions,
            })
        }

        fn transactions(&self, _args: TransactionsArgs) -> Result<TransactionsReturns, ManyError> {
            Ok(TransactionsReturns {
                nb_transactions: self.0.len() as u64,
            })
        }
    }

    // TODO: Refactor using Account PR helper
    prop_compose! {
        fn arb_server()(name in "\\PC*") -> (CoseKeyIdentity, Arc<Mutex<ManyServer>>) {
            let id = generate_random_eddsa_identity();
            let server = ManyServer::new(name, id.clone());
            let ledger_txs_impl = Arc::new(Mutex::new(LedgerTransactionsImpl::default()));
            let ledger_txs_module = LedgerTransactionsModule::new(ledger_txs_impl);

            {
                let mut s = server.lock().unwrap();
                s.version = Some(SERVER_VERSION.to_string());
                s.add_module(ledger_txs_module);
            }

            (id, server)
        }
    }

    proptest! {
        #[test]
        fn list_ascending((id, server) in arb_server()) {
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(SERVER_VERSION)
                .from(id.identity)
                .to(id.identity)
                .method("ledger.list".to_string())
                .data(cbor_diag::parse_diag(r#"{0: 100, 1: 1}"#).unwrap().to_bytes())
                .build()
                .unwrap();

            let response_message = execute_request(id, server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            let bytes = response_message.data.unwrap();
            let list_returns: ListReturns = minicbor::decode(&bytes).unwrap();

            assert_eq!(list_returns.nb_transactions, 2);

            // TODO: Use direct impl instead. This is not a good workaround
            assert_eq!(list_returns.transactions[0].id, LedgerTransactionsImpl::default().0[0].id);
            assert!(list_returns.transactions[0].time < LedgerTransactionsImpl::default().0[0].time);

            // TODO: Check content
        }

        #[test]
        fn transactions((id, server) in arb_server()) {
            let request: RequestMessage = RequestMessageBuilder::default()
                .version(SERVER_VERSION)
                .from(id.identity)
                .to(id.identity)
                .method("ledger.transactions".to_string())
                .data("null".as_bytes().to_vec())
                .build()
                .unwrap();

            let response_message = execute_request(id, server, request);

            let bytes = response_message.to_bytes().unwrap();
            let response_message: ResponseMessage = minicbor::decode(&bytes).unwrap();

            let bytes = response_message.data.unwrap();
            let transactions_returns: TransactionsReturns = minicbor::decode(&bytes).unwrap();

            assert_eq!(transactions_returns.nb_transactions, 2);
        }
    }
}
