use many_client_macros::many_client;
use many_modules::blockchain::{
    BlockArgs, BlockReturns, InfoReturns, TransactionArgs, TransactionReturns,
};

#[many_client(
    namespace = "blockchain",
    methods(
        info(returns = "InfoReturns"),
        block(params = "BlockArgs", returns = "BlockReturns"),
        transaction(params = "TransactionArgs", returns = "TransactionReturns")
    )
)]
pub struct BlockchainClient;
