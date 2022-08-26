use many_client_macros::many_client;
pub use many_modules::blockchain::{
    BlockArgs, BlockReturns, InfoReturns, TransactionArgs, TransactionReturns,
};
use many_protocol::ManyError;

use crate::ManyClient;

#[many_client(BlockchainClient, "blockchain")]
trait BlockchainClientTrait {
    async fn info(&self) -> Result<InfoReturns, ManyError>;
    async fn block(&self, args: BlockArgs) -> Result<BlockReturns, ManyError>;
    async fn transaction(&self, args: TransactionArgs) -> Result<TransactionReturns, ManyError>;
}

pub struct BlockchainClient(ManyClient);
