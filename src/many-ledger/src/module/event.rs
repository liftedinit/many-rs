use crate::module::LedgerModuleImpl;
use many_error::ManyError;
use many_identity::Address;
use many_modules::account::features::multisig::MultisigTransactionState;
use many_modules::events;
use many_modules::events::{
    EventFilterAttributeSpecific, EventFilterAttributeSpecificIndex, EventInfo, EventLog,
};
use many_types::{CborRange, Timestamp, VecOrSingle};
use std::collections::BTreeMap;

const MAXIMUM_EVENT_COUNT: usize = 100;

type EventLogResult = Result<events::EventLog, ManyError>;

fn filter_account<'a>(
    it: Box<dyn Iterator<Item = EventLogResult> + 'a>,
    account: Option<VecOrSingle<Address>>,
) -> Box<dyn Iterator<Item = EventLogResult> + 'a> {
    if let Some(account) = account {
        let account: Vec<Address> = account.into();
        Box::new(it.filter(move |t| match t {
            // Propagate the errors.
            Err(_) => true,
            Ok(t) => account.iter().any(|id| t.is_about(*id)),
        }))
    } else {
        it
    }
}

fn filter_event_kind<'a>(
    it: Box<dyn Iterator<Item = EventLogResult> + 'a>,
    event_kind: Option<VecOrSingle<events::EventKind>>,
) -> Box<dyn Iterator<Item = EventLogResult> + 'a> {
    if let Some(k) = event_kind {
        let k: Vec<events::EventKind> = k.into();
        Box::new(it.filter(move |t| match t {
            Err(_) => true,
            Ok(t) => k.contains(&t.kind()),
        }))
    } else {
        it
    }
}

fn filter_date<'a>(
    it: Box<dyn Iterator<Item = EventLogResult> + 'a>,
    range: CborRange<Timestamp>,
) -> Box<dyn Iterator<Item = EventLogResult> + 'a> {
    Box::new(it.filter(move |t| match t {
        // Propagate the errors.
        Err(_) => true,
        Ok(events::EventLog { time, .. }) => range.contains(time),
    }))
}

fn filter_attribute_specific<'a>(
    mut it: Box<dyn Iterator<Item = EventLogResult> + 'a>,
    attribute_specific: &'a BTreeMap<
        EventFilterAttributeSpecificIndex,
        EventFilterAttributeSpecific,
    >,
) -> Box<dyn Iterator<Item = EventLogResult> + 'a> {
    for x in attribute_specific.values() {
        match x {
            EventFilterAttributeSpecific::MultisigTransactionState(VecOrSingle(state)) => {
                it = Box::new(it.filter(|t| match t {
                    Err(_) => true,
                    Ok(EventLog {
                        content: EventInfo::AccountMultisigSubmit { .. },
                        ..
                    })
                    | Ok(EventLog {
                        content: EventInfo::AccountMultisigApprove { .. },
                        ..
                    }) => state.contains(&MultisigTransactionState::Pending),
                    Ok(EventLog {
                        content: EventInfo::AccountMultisigExecute { .. },
                        ..
                    }) => {
                        state.contains(&MultisigTransactionState::ExecutedAutomatically)
                            || state.contains(&MultisigTransactionState::ExecutedManually)
                    }
                    Ok(EventLog {
                        content: EventInfo::AccountMultisigWithdraw { .. },
                        ..
                    }) => state.contains(&MultisigTransactionState::Withdrawn),
                    Ok(EventLog {
                        content: EventInfo::AccountMultisigExpired { .. },
                        ..
                    }) => state.contains(&MultisigTransactionState::Expired),
                    _ => false,
                }))
            }
        }
    }
    it
}

impl events::EventsModuleBackend for LedgerModuleImpl {
    fn info(&self, _args: events::InfoArgs) -> Result<events::InfoReturn, ManyError> {
        use strum::IntoEnumIterator;
        Ok(events::InfoReturn {
            total: self.storage.nb_events()?,
            event_types: events::EventKind::iter().collect(),
        })
    }

    fn list(&self, args: events::ListArgs) -> Result<events::ListReturns, ManyError> {
        let events::ListArgs {
            count,
            order,
            filter,
        } = args;
        let filter = filter.unwrap_or_default();

        let count = count.map_or(MAXIMUM_EVENT_COUNT, |c| {
            std::cmp::min(c as usize, MAXIMUM_EVENT_COUNT)
        });

        let storage = &self.storage;
        let nb_events = storage.nb_events()?;
        let iter = storage.iter_events(
            filter.id_range.unwrap_or_default(),
            order.unwrap_or_default(),
        );

        let iter = Box::new(iter.map(|item| {
            let (_k, v) = item.map_err(ManyError::unknown)?;
            minicbor::decode::<events::EventLog>(v.as_slice())
                .map_err(ManyError::deserialization_error)
        }));

        let iter = filter_account(iter, filter.account);
        let iter = filter_event_kind(iter, filter.kind);
        let iter = filter_date(iter, filter.date_range.unwrap_or_default());
        let iter = filter_attribute_specific(iter, &filter.events_filter_attribute_specific);

        let events: Vec<events::EventLog> = iter.take(count).collect::<Result<_, _>>()?;

        Ok(events::ListReturns { nb_events, events })
    }
}
