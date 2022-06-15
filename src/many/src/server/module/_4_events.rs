use crate::ManyError;
use many_macros::many_module;

#[cfg(test)]
use mockall::{automock, predicate::*};

mod info;
mod list;

pub use info::*;
pub use list::*;

#[many_module(name = EventsModule, id = 4, namespace = events, many_crate = crate)]
#[cfg_attr(test, automock)]
pub trait EventsModuleBackend: Send {
    fn info(&self, args: InfoArgs) -> Result<InfoReturn, ManyError>;
    fn list(&self, args: ListArgs) -> Result<ListReturns, ManyError>;
}

#[cfg(test)]
mod tests {
    use minicbor::bytes::ByteVec;
    use mockall::predicate;
    use std::sync::{Arc, Mutex};

    use crate::server::module::testutils::{call_module, call_module_cbor};
    use crate::types::events::TransactionKind;
    use crate::types::events::{Transaction, TransactionId, TransactionInfo};
    use crate::types::ledger::TokenAmount;
    use crate::types::Timestamp;
    use crate::Identity;

    use super::*;

    #[test]
    fn info() {
        let mut mock = MockEventsModuleBackend::new();
        mock.expect_info()
            .with(predicate::eq(InfoArgs {}))
            .times(1)
            .returning(|_args| {
                Ok(InfoReturn {
                    total: 12,
                    event_types: vec![TransactionKind::Send],
                })
            });
        let module = super::EventsModule::new(Arc::new(Mutex::new(mock)));

        let info_returns: InfoReturn =
            minicbor::decode(&call_module(1, &module, "events.info", "null").unwrap()).unwrap();

        assert_eq!(info_returns.total, 12);
        assert_eq!(info_returns.event_types, &[TransactionKind::Send]);
    }

    #[test]
    fn list() {
        let data = ListArgs {
            count: Some(1),
            order: None,
            filter: None,
        };
        let mut mock = MockEventsModuleBackend::new();
        mock.expect_list()
            .with(predicate::eq(data.clone()))
            .times(1)
            .returning(|_args| {
                Ok(ListReturns {
                    nb_events: 1,
                    events: vec![Transaction {
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
        let module = super::EventsModule::new(Arc::new(Mutex::new(mock)));

        let list_returns: ListReturns = minicbor::decode(
            &call_module_cbor(1, &module, "events.list", minicbor::to_vec(data).unwrap()).unwrap(),
        )
        .unwrap();

        assert_eq!(list_returns.nb_events, 1);
        assert_eq!(list_returns.events.len(), 1);
    }
}
