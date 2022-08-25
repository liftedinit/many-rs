use many_client_macros::many_client;
use many_modules::ledger::{BalanceArgs, BalanceReturns, InfoReturns, SendArgs};
pub use many_types::ledger::{Symbol, TokenAmount};

#[many_client(
    namespace = "ledger",
    methods(
        info(returns = "InfoReturns"),
        balance(params = "BalanceArgs", returns = "BalanceReturns"),
        send(params = "SendArgs"),
    )
)]
pub struct LedgerClient;
