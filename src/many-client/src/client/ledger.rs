use many_client_macros::many_client;
pub use many_identity::Identity;
pub use many_modules::ledger::{BalanceArgs, BalanceReturns, InfoReturns, SendArgs, SendReturns};
use many_server::ManyError;
pub use many_types::ledger::{Symbol, TokenAmount};

use crate::ManyClient;

#[many_client(LedgerClient, "ledger")]
trait LedgerClientTrait {
    fn info(&self) -> Result<InfoReturns, ManyError>;
    fn balance(&self, args: BalanceArgs) -> Result<BalanceReturns, ManyError>;
    fn send(&self, args: SendArgs) -> Result<SendReturns, ManyError>;
}

#[derive(Debug, Clone)]
pub struct LedgerClient<I: Identity>(ManyClient<I>);
