#![cfg(feature = "migration_testing")]

use crate::migration::MIGRATIONS;
use crate::storage::InnerStorage;
use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::InnerMigration;
use many_protocol::ResponseMessage;
use many_types::Timestamp;
use minicbor::{Decode, Encode};

#[derive(Decode, Encode)]
pub struct DummyHotfix<'a> {
    #[n[0]]
    #[cbor(encode_with = "minicbor::bytes::encode")]
    #[cbor(decode_with = "minicbor::bytes::decode")]
    pub tx_id: &'a [u8],

    #[b(1)]
    pub response: ResponseMessage,
}

impl<'a> DummyHotfix<'a> {
    pub const fn new(tx_id: &'a [u8], response: ResponseMessage) -> Self {
        Self { tx_id, response }
    }
}

const DUMMY_HOTFIX_TX_ID: &str = "0001";
const DUMMY_HOTFIX_NEW_TIMESTAMP: u64 = 1234567890;

fn dummy_hotfix(b: &[u8]) -> Option<Vec<u8>> {
    let maybe_tx = minicbor::decode::<DummyHotfix>(b);
    if let Ok(tx) = maybe_tx {
        tracing::trace!("Found a valid Dummy Hotfix, checking Tx ID");
        if hex::encode(tx.tx_id).as_str().ends_with(DUMMY_HOTFIX_TX_ID) {
            tracing::trace!("Dummy Hotfix Tx ID is valid, migrating...");
            let new_response = ResponseMessage {
                timestamp: Some(Timestamp::new(DUMMY_HOTFIX_NEW_TIMESTAMP).unwrap()),
                ..tx.response
            };
            if let Ok(ret) = minicbor::to_vec(new_response) {
                tracing::trace!("Dummy Hotfix successfully migrated.");
                return Some(ret);
            }
        }
    }
    tracing::trace!("Skipping Dummy Hotfix migration.");
    None
}

#[distributed_slice(MIGRATIONS)]
static DUMMY_HOTFIX: InnerMigration<InnerStorage, ManyError> =
    InnerMigration::new_hotfix(dummy_hotfix, "Dummy Hotfix", "For testing purpose only.");
