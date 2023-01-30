//! https://github.com/liftedinit/many-framework/issues/205

use crate::migration::MIGRATIONS;
use crate::storage::InnerStorage;
use linkme::distributed_slice;
use many_error::ManyError;
use many_migration::InnerMigration;
use many_protocol::ResponseMessage;
use many_types::Timestamp;
use minicbor::{Decode, Encode};

#[derive(Decode, Encode)]
pub struct Block9400Tx<'a> {
    #[n[0]]
    #[cbor(encode_with = "minicbor::bytes::encode")]
    #[cbor(decode_with = "minicbor::bytes::decode")]
    pub tx_id: &'a [u8],

    #[b(1)]
    pub response: ResponseMessage,
}

impl<'a> Block9400Tx<'a> {
    pub const fn new(tx_id: &'a [u8], response: ResponseMessage) -> Self {
        Self { tx_id, response }
    }
}

const BLOCK_9400_TX_ID: &str = "241e00000001";
const BLOCK_9400_NEW_TIMESTAMP: u64 = 1658348752;

fn block_9400(b: &[u8]) -> Option<Vec<u8>> {
    let maybe_tx = minicbor::decode::<Block9400Tx>(b);
    if let Ok(tx) = maybe_tx {
        tracing::trace!("Found a valid Block9400Tx, checking Tx ID");
        if hex::encode(tx.tx_id).as_str() == BLOCK_9400_TX_ID {
            tracing::trace!("Block 9400 Tx ID is valid, migrating...");
            let new_response = ResponseMessage {
                timestamp: Some(Timestamp::new(BLOCK_9400_NEW_TIMESTAMP).unwrap()),
                ..tx.response
            };
            if let Ok(ret) = minicbor::to_vec(new_response) {
                tracing::trace!("Block 9400 successfully migrated.");
                return Some(ret);
            }
        }
    }
    tracing::trace!("Skipping block 9400 migration.");
    None
}

#[distributed_slice(MIGRATIONS)]
static B9400: InnerMigration<InnerStorage, ManyError> = InnerMigration::new_hotfix(
    block_9400,
    "Block 9400",
    r#"Fix Block 9400 timestamp. Lifted Initiative Ledger network.
        https://github.com/liftedinit/many-framework/issues/205"#,
);
