use many_identity::{Address, CoseKeyIdentity};
use many_modules::ledger::{BalanceArgs, BalanceReturns, InfoReturns, SendArgs};
use many_protocol::ManyError;
pub use many_types::ledger::{Symbol, TokenAmount};
use many_types::VecOrSingle;

use crate::ManyClient;

#[derive(Clone, Debug)]
pub struct LedgerClient {
    client: ManyClient,
}

impl LedgerClient {
    pub fn new(client: ManyClient) -> Self {
        LedgerClient { client }
    }

    pub async fn info(&self) -> Result<InfoReturns, ManyError> {
        let response = self.client.call_("ledger.info", ()).await?;
        minicbor::decode(&response).map_err(ManyError::deserialization_error)
    }

    pub async fn balance(
        &self,
        account: Option<Address>,
        symbols: Option<Vec<Symbol>>,
    ) -> Result<BalanceReturns, ManyError> {
        let argument = BalanceArgs {
            account,
            symbols: symbols.map(VecOrSingle),
        };
        let data = self.client.call_("ledger.balance", argument).await?;
        minicbor::decode(&data).map_err(ManyError::deserialization_error)
    }

    pub async fn send(
        &self,
        from: CoseKeyIdentity,
        to: Address,
        amount: TokenAmount,
        symbol: Symbol,
    ) -> Result<(), ManyError> {
        let client = ManyClient::new(
            self.client.url.clone(),
            CoseKeyIdentity::anonymous().identity,
            from.clone(),
        )
        .map_err(|_| ManyError::could_not_route_message())?;
        let argument = SendArgs {
            from: Some(from.identity),
            to,
            amount,
            symbol,
        };
        client.call_("ledger.send", argument).await?;
        Ok(())
    }
}
