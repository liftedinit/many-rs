use std::collections::BTreeMap;

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

    pub async fn symbols(&self) -> Result<Vec<Symbol>, ManyError> {
        let response = self.client.call_("ledger.info", ()).await?;
        let decoded: InfoReturns =
            minicbor::decode(&response).map_err(ManyError::deserialization_error)?;
        Ok(decoded.symbols)
    }

    pub async fn balance(
        &self,
        account: Address,
        symbols: Vec<Symbol>,
    ) -> Result<BTreeMap<Symbol, TokenAmount>, ManyError> {
        let argument = BalanceArgs {
            account: Some(account),
            symbols: Some(VecOrSingle(symbols)),
        };
        let data = self.client.call_("ledger.balance", argument).await?;
        let response: BalanceReturns =
            minicbor::decode(&data).map_err(ManyError::deserialization_error)?;
        Ok(response.balances)
    }

    pub async fn send(
        &self,
        from: CoseKeyIdentity,
        to: Address,
        amount: TokenAmount,
        symbol: Symbol,
    ) -> Result<(), ManyError> {
        let argument = SendArgs {
            from: Some(from.identity),
            to,
            amount,
            symbol,
        };
        self.client.call_("ledger.send", argument).await?;
        Ok(())
    }
}
