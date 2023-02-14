use crate::error;
use crate::storage::iterator::LedgerIterator;
use crate::storage::LedgerStorage;
use many_error::ManyError;
use many_modules::events;
use many_modules::events::EventId;
use many_types::{CborRange, SortOrder};
use merk::Op;

pub(crate) const EVENTS_ROOT: &[u8] = b"/events/";
pub(crate) const EVENT_COUNT_ROOT: &[u8] = b"/events_count";

// Left-shift the height by this amount of bits
pub(crate) const HEIGHT_EVENTID_SHIFT: u64 = 32;

/// Number of bytes in an event ID when serialized. Keys smaller than this
/// will have `\0` prepended, and keys larger will be cut to this number of
/// bytes.
pub(crate) const EVENT_ID_KEY_SIZE_IN_BYTES: usize = 32;

/// Returns the storage key for an event in the kv-store.
pub(super) fn key_for_event(id: events::EventId) -> Vec<u8> {
    let id = id.as_ref();
    let id = if id.len() > EVENT_ID_KEY_SIZE_IN_BYTES {
        &id[0..EVENT_ID_KEY_SIZE_IN_BYTES]
    } else {
        id
    };

    let mut exp_id = [0u8; EVENT_ID_KEY_SIZE_IN_BYTES];
    exp_id[(EVENT_ID_KEY_SIZE_IN_BYTES - id.len())..].copy_from_slice(id);
    vec![EVENTS_ROOT.to_vec(), exp_id.to_vec()].concat()
}

impl LedgerStorage {
    pub(crate) fn new_event_id(&mut self) -> events::EventId {
        self.latest_tid += 1;
        self.latest_tid.clone()
    }

    pub fn nb_events(&self) -> Result<u64, ManyError> {
        self.persistent_store
            .get(EVENT_COUNT_ROOT)
            .map_err(error::storage_get_failed)?
            .map_or(Ok(0), |x| {
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(x.as_slice());
                Ok(u64::from_be_bytes(bytes))
            })
    }

    pub(crate) fn log_event(&mut self, content: events::EventInfo) -> Result<(), ManyError> {
        let current_nb_events = self.nb_events()?;
        let event = events::EventLog {
            id: self.new_event_id(),
            time: self.now(),
            content,
        };

        self.persistent_store
            .apply(&[
                (
                    key_for_event(event.id.clone()),
                    Op::Put(minicbor::to_vec(&event).map_err(ManyError::serialization_error)?),
                ),
                (
                    EVENT_COUNT_ROOT.to_vec(),
                    Op::Put((current_nb_events + 1).to_be_bytes().to_vec()),
                ),
            ])
            .map_err(error::storage_apply_failed)?;

        self.maybe_commit()?;
        Ok(())
    }

    pub fn iter_multisig(&self, order: SortOrder) -> LedgerIterator {
        LedgerIterator::all_multisig(&self.persistent_store, order)
    }

    pub fn iter_events(&self, range: CborRange<EventId>, order: SortOrder) -> LedgerIterator {
        LedgerIterator::events_scoped_by_id(&self.persistent_store, range, order)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use many_modules::events::EventId;

    #[test]
    fn event_key_size() {
        let golden_size = key_for_event(events::EventId::from(0)).len();

        assert_eq!(golden_size, key_for_event(EventId::from(u64::MAX)).len());

        // Test at 1 byte, 2 bytes and 4 bytes boundaries.
        for i in [u8::MAX as u64, u16::MAX as u64, u32::MAX as u64] {
            assert_eq!(golden_size, key_for_event(EventId::from(i - 1)).len());
            assert_eq!(golden_size, key_for_event(EventId::from(i)).len());
            assert_eq!(golden_size, key_for_event(EventId::from(i + 1)).len());
        }

        assert_eq!(
            golden_size,
            key_for_event(EventId::from(b"012345678901234567890123456789".to_vec())).len()
        );

        // Trim the Event ID if it's too long.
        assert_eq!(
            golden_size,
            key_for_event(EventId::from(
                b"0123456789012345678901234567890123456789".to_vec()
            ))
            .len()
        );
        assert_eq!(
            key_for_event(EventId::from(b"01234567890123456789012345678901".to_vec())).len(),
            key_for_event(EventId::from(
                b"0123456789012345678901234567890123456789012345678901234567890123456789".to_vec()
            ))
            .len()
        )
    }
}
