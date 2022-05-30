use crate::ManyError;
use many_macros::many_module;

#[cfg(test)]
use mockall::{automock, predicate::*};

mod info;
mod list;

pub use info::*;
pub use list::*;

#[many_module(name = LedgerTransactionsModule, id = 4, namespace = ledger, many_crate = crate)]
#[cfg_attr(test, automock)]
pub trait LedgerTransactionsModuleBackend: Send {
    fn transactions(&self, args: TransactionsArgs) -> Result<TransactionsReturns, ManyError>;
    fn list(&self, args: ListArgs) -> Result<ListReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use minicbor::bytes::ByteVec;
    use mockall::predicate;

    use crate::{
        server::module::testutils::{call_module, call_module_cbor},
        types::{
            ledger::{TokenAmount, Transaction, TransactionId, TransactionInfo},
            Timestamp,
        },
        Identity,
    };

    use super::*;

    #[test]
    fn transactions() {
        let mut mock = MockLedgerTransactionsModuleBackend::new();
        mock.expect_transactions()
            .with(predicate::eq(TransactionsArgs {}))
            .times(1)
            .returning(|_args| {
                Ok(TransactionsReturns {
                    nb_transactions: 12,
                })
            });
        let module = super::LedgerTransactionsModule::new(Arc::new(Mutex::new(mock)));

        let transactions_returns: TransactionsReturns =
            minicbor::decode(&call_module(1, &module, "ledger.transactions", "null").unwrap())
                .unwrap();

        assert_eq!(transactions_returns.nb_transactions, 12);
    }

    #[test]
    fn list() {
        let data = ListArgs {
            count: Some(1),
            order: None,
            filter: None,
        };
        let mut mock = MockLedgerTransactionsModuleBackend::new();
        mock.expect_list()
        .with(predicate::eq(data.clone()))
        .times(1).returning(|_args| {
            Ok(ListReturns {
                nb_transactions: 1,
                transactions: vec![Transaction {
                    id: TransactionId(ByteVec::from(vec![1, 1, 1, 1])),
                    time: Timestamp::now(),
                    content: TransactionInfo::Send {
                        from: Identity::anonymous(),
                        to: Identity::anonymous(),
                        symbol: Default::default(),
                        amount: TokenAmount::from(1000u64),
                    },
                }],
            })
        });
        let module = super::LedgerTransactionsModule::new(Arc::new(Mutex::new(mock)));

        let list_returns: ListReturns = minicbor::decode(
            &call_module_cbor(1, &module, "ledger.list", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();

        assert_eq!(list_returns.nb_transactions, 1);
        assert_eq!(list_returns.transactions.len(), 1);
    }
}
