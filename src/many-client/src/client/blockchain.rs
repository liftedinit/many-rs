use many_client_macros::many_client;
pub use many_identity::Identity;
pub use many_modules::blockchain::{
    BlockArgs, BlockReturns, InfoReturns, TransactionArgs, TransactionReturns,
};
use many_server::ManyError;
pub use many_types::blockchain::{
    Block, BlockIdentifier, SingleBlockQuery, SingleTransactionQuery,
};

use crate::ManyClient;

#[many_client(BlockchainClient, "blockchain")]
trait BlockchainClientTrait {
    fn info(&self) -> Result<InfoReturns, ManyError>;
    fn block(&self, args: BlockArgs) -> Result<BlockReturns, ManyError>;
    fn transaction(&self, args: TransactionArgs) -> Result<TransactionReturns, ManyError>;
}

#[derive(Debug, Clone)]
pub struct BlockchainClient<I: Identity>(ManyClient<I>);
