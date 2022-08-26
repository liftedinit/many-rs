use many_client_macros::many_client;
pub use many_modules::ledger::{BalanceArgs, BalanceReturns, InfoReturns, SendArgs, SendReturns};
use many_protocol::ManyError;
pub use many_types::ledger::{Symbol, TokenAmount};

use crate::ManyClient;

#[many_client(LedgerClient, "ledger")]
trait LedgerClientTrait {
    async fn info(&self) -> Result<InfoReturns, ManyError>;
    async fn balance(&self, args: BalanceArgs) -> Result<BalanceReturns, ManyError>;
    async fn send(&self, args: SendArgs) -> Result<SendReturns, ManyError>;
}

pub struct LedgerClient(ManyClient);
