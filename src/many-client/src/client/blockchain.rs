use many_client_macros::many_client;
pub use many_modules::blockchain::{
    BlockArgs, BlockReturns, InfoReturns, TransactionArgs, TransactionReturns,
};
use many_protocol::ManyError;

use crate::ManyClient;

#[many_client(BlockchainClient, "blockchain")]
trait BlockchainClientTrait {
    fn info(&self) -> Result<InfoReturns, ManyError>;
    fn block(&self, args: BlockArgs) -> Result<BlockReturns, ManyError>;
    fn transaction(&self, args: TransactionArgs) -> Result<TransactionReturns, ManyError>;
}

#[derive(Debug, Clone)]
pub struct BlockchainClient(ManyClient);
