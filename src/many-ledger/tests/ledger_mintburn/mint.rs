use cucumber::{given, then, when, World};
use many_error::ManyError;
use many_identity::Address;
use many_ledger::migration::tokens::TOKEN_MIGRATION;
use many_ledger::module::LedgerModuleImpl;
use many_ledger_test_macros::*;
use many_ledger_test_utils::cucumber::{
    refresh_token_info, verify_error_addr, verify_error_code, AccountWorld, LedgerWorld, SomeError,
    SomeId, TokenWorld,
};
use many_ledger_test_utils::Setup;
use many_modules::events::{EventFilter, EventKind, EventsModuleBackend, ListArgs};
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_modules::ledger::{
    BalanceArgs, LedgerMintBurnModuleBackend, LedgerModuleBackend, TokenMintArgs,
};
use many_types::ledger::{TokenAmount, TokenInfo};
use many_types::Memo;
use std::path::Path;

#[derive(World, Debug, Default, LedgerWorld, TokenWorld, AccountWorld)]
#[world(init = Self::new)]
struct MintWorld {
    setup: Setup,
    args: TokenMintArgs,
    info: TokenInfo,
    ext_info: TokenExtendedInfo,
    account: Address,
    error: Option<ManyError>,
}

impl MintWorld {
    fn new() -> Self {
        Self {
            setup: Setup::new_with_migrations(false, [(0, &TOKEN_MIGRATION)], true),
            ..Default::default()
        }
    }
}

fn fail_mint_token(w: &mut MintWorld, sender: &Address) {
    w.error = Some(
        LedgerMintBurnModuleBackend::mint(&mut w.setup.module_impl, sender, w.args.clone())
            .expect_err("Token minting was supposed to fail, it succeeded instead."),
    );
}

#[given(expr = "a default token owned by {id}")]
fn create_default_token(w: &mut MintWorld, id: SomeId) {
    many_ledger_test_utils::cucumber::create_default_token(w, id);
    w.args.symbol = w.info.symbol;
}

#[given(expr = "a default token of unlimited supply owned by {id}")]
fn create_default_token_unlimited(w: &mut MintWorld, id: SomeId) {
    many_ledger_test_utils::cucumber::create_default_token_unlimited(w, id);
    w.args.symbol = w.info.symbol;
}

#[given(expr = "a distribution of {int} tokens to {id}")]
fn distribution_of(w: &mut MintWorld, amount: u64, id: SomeId) {
    w.args.distribution.insert(id.as_address(w), amount.into());
}

#[given(expr = "a memo {string}")]
fn a_memo(w: &mut MintWorld, memo: String) {
    w.args.memo = Some(Memo::try_from(memo).expect("Unable to create memo"));
}

#[when(expr = "I mint the tokens as {id}")]
fn mint_tokens(w: &mut MintWorld, id: SomeId) {
    let sender = id.as_address(w);
    LedgerMintBurnModuleBackend::mint(&mut w.setup.module_impl, &sender, w.args.clone())
        .expect("Unable to mint tokens");
    refresh_token_info(w);
}

#[then(expr = "{id} has {int} tokens")]
fn id_has_tokens(w: &mut MintWorld, id: SomeId, amount: u64) {
    let addr = id.as_address(w);
    let res = LedgerModuleBackend::balance(
        &w.setup.module_impl,
        &Address::anonymous(),
        BalanceArgs {
            account: Some(addr),
            symbols: Some(vec![w.info.symbol].into()),
        },
    )
    .unwrap_or_else(|_| panic!("Unable to fetch balance for {addr}"));
    let amount: TokenAmount = amount.into();
    let zero = TokenAmount::zero();
    let balance = res.balances.get(&w.info.symbol).unwrap_or(&zero);
    assert_eq!(*balance, amount);
}

#[then(expr = "the circulating supply is {int} tokens")]
fn circulating_supply(w: &mut MintWorld, amount: u64) {
    let amount: TokenAmount = amount.into();
    assert_eq!(w.info.supply.circulating, amount);
}

#[then(expr = "the total supply is {int} tokens")]
fn total_supply(w: &mut MintWorld, amount: u64) {
    let amount: TokenAmount = amount.into();
    assert_eq!(w.info.supply.total, amount);
}

#[then(expr = "the memo is {string}")]
fn memo_is(w: &mut MintWorld, memo: String) {
    let res = EventsModuleBackend::list(
        &w.setup.module_impl,
        ListArgs {
            filter: Some(EventFilter {
                kind: Some(vec![EventKind::TokenMint].into()),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .expect("Unable to list TokenMint event");
    let memo = Memo::try_from(memo).unwrap();
    assert!(res.nb_events >= 1);
    let event = res.events.into_iter().next().expect("Expected an event");
    assert!(event.content.memo().is_some());
    assert_eq!(event.content.memo().unwrap(), &memo);
}

#[then(expr = "minting as {id} fails with {error}")]
fn minting_fails(w: &mut MintWorld, id: SomeId, error: SomeError) {
    let id = id.as_address(w);
    fail_mint_token(w, &id);
    verify_error_code(w, error.as_many_code())
}

#[then(expr = "the error address is {id}")]
fn error_address_is(w: &mut MintWorld, id: SomeId) {
    verify_error_addr(w, id.as_address(w));
}

#[tokio::main]
async fn main() {
    // Support both Cargo and Bazel paths
    let features = ["tests/features", "src/many-ledger/tests/features"]
        .into_iter()
        .find(|&p| Path::new(p).exists())
        .expect("Cucumber test features not found");

    MintWorld::run(Path::new(features).join("ledger_mintburn/mint.feature")).await;
}
