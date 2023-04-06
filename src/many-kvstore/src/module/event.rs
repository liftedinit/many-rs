use super::KvStoreModuleImpl;
use many_error::ManyError;
use many_identity::Address;
use many_modules::events;
use many_types::{CborRange, Timestamp, VecOrSingle};

const MAXIMUM_EVENT_COUNT: usize = 100;

impl events::EventsModuleBackend for KvStoreModuleImpl {
    fn info(&self, _args: events::InfoArgs) -> Result<events::InfoReturn, ManyError> {
        use strum::IntoEnumIterator;
        Ok(events::InfoReturn {
            total: self.storage.nb_events(),
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
        let nb_events = storage.nb_events();
        let iter = storage.iter(
            filter.id_range.unwrap_or_default(),
            order.unwrap_or_default(),
        );

        let iter = Box::new(iter.map(|item| {
            let (_k, v) = item.map_err(|e| ManyError::unknown(e.to_string()))?;
            minicbor::decode::<events::EventLog>(v.as_slice())
                .map_err(|e| ManyError::deserialization_error(e.to_string()))
        }));

        let iter = filter_account(iter, filter.account);
        let iter = filter_event_kind(iter, filter.kind);
        let iter = filter_date(iter, filter.date_range.unwrap_or_default());

        let events: Vec<events::EventLog> = iter.take(count).collect::<Result<_, _>>()?;

        Ok(events::ListReturns { nb_events, events })
    }
}

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
