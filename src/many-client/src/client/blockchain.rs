use many_modules::blockchain::{
    BlockArgs, BlockReturns, InfoReturns, TransactionArgs, TransactionReturns,
};
use many_protocol::ManyError;
use many_types::blockchain::{SingleBlockQuery, SingleTransactionQuery};

use crate::ManyClient;

#[derive(Clone, Debug)]
pub struct BlockchainClient {
    client: ManyClient,
}

impl BlockchainClient {
    pub fn new(client: ManyClient) -> Self {
        BlockchainClient { client }
    }

    pub async fn info(&self) -> Result<InfoReturns, ManyError> {
        let response = self.client.call_("blockchain.info", ()).await?;
        minicbor::decode(&response).map_err(ManyError::deserialization_error)
    }

    pub async fn block(&self, query: SingleBlockQuery) -> Result<BlockReturns, ManyError> {
        let arguments = BlockArgs { query };
        let response = self.client.call_("blockchain.block", arguments).await?;
        minicbor::decode(&response).map_err(ManyError::deserialization_error)
    }

    pub async fn transaction(
        &self,
        query: SingleTransactionQuery,
    ) -> Result<TransactionReturns, ManyError> {
        let arguments = TransactionArgs { query };
        let response = self
            .client
            .call_("blockchain.transaction", arguments)
            .await?;
        minicbor::decode(&response).map_err(ManyError::deserialization_error)
    }
}
