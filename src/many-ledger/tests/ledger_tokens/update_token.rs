use many_ledger_test_macros::*;
use many_ledger_test_utils::cucumber::{
    verify_error_code, verify_error_role, verify_error_ticker, AccountWorld, LedgerWorld,
    SomeError, SomeId, SomePermission, TokenWorld,
};
use many_ledger_test_utils::Setup;

use cucumber::{given, then, when, World};
use many_error::ManyError;
use many_identity::Address;
use many_ledger::migration::tokens::TOKEN_MIGRATION;
use many_ledger::module::LedgerModuleImpl;
use many_modules::events::{EventFilter, EventKind, EventsModuleBackend, ListArgs};
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_modules::ledger::{LedgerTokensModuleBackend, TokenInfoArgs, TokenUpdateArgs};
use many_types::cbor::CborNull;
use many_types::ledger::{TokenInfo, TokenMaybeOwner};
use many_types::Memo;
use std::path::Path;

#[derive(World, Debug, Default, LedgerWorld, TokenWorld, AccountWorld)]
#[world(init = Self::new)]
struct UpdateWorld {
    setup: Setup,
    args: TokenUpdateArgs,
    info: TokenInfo,
    ext_info: TokenExtendedInfo,
    account: Address,
    error: Option<ManyError>,
}

impl UpdateWorld {
    fn new() -> Self {
        Self {
            setup: Setup::new_with_migrations(false, [(0, &TOKEN_MIGRATION)], true),
            ..Default::default()
        }
    }
}

fn fail_update_token(w: &mut UpdateWorld, sender: &Address) {
    w.error = Some(
        LedgerTokensModuleBackend::update(&mut w.setup.module_impl, sender, w.args.clone())
            .expect_err("Token update was supposed to fail, it succeeded instead."),
    );
}

#[given(expr = "a token account")]
fn given_token_account(w: &mut UpdateWorld) {
    many_ledger_test_utils::cucumber::given_token_account(w);
}

#[given(expr = "{id} as the account owner")]
fn given_account_id_owner(w: &mut UpdateWorld, id: SomeId) {
    many_ledger_test_utils::cucumber::given_account_id_owner(w, id);
}

#[given(expr = "{id} has {permission} permission")]
fn given_account_part_of_can_create(w: &mut UpdateWorld, id: SomeId, permission: SomePermission) {
    many_ledger_test_utils::cucumber::given_account_part_of_can_create(w, id, permission);
}

#[given(expr = "a default token owned by {id}")]
fn create_default_token(w: &mut UpdateWorld, id: SomeId) {
    many_ledger_test_utils::cucumber::create_default_token(w, id);
    w.args.symbol = w.info.symbol;
}

#[given(expr = "a new ticker {word}")]
fn given_new_ticker(w: &mut UpdateWorld, ticker: String) {
    w.args.ticker = Some(ticker);
}

#[given(expr = "a new name {word}")]
fn given_new_name(w: &mut UpdateWorld, name: String) {
    w.args.name = Some(name);
}

#[given(expr = "a new decimal {int}")]
fn given_new_decimal(w: &mut UpdateWorld, decimal: u64) {
    w.args.decimals = Some(decimal);
}

#[given(expr = "a token owner {word}")]
fn given_new_owner(w: &mut UpdateWorld, owner: Address) {
    w.args.owner = Some(TokenMaybeOwner::Left(owner));
}

#[given(expr = "a memo {string}")]
fn given_memo(w: &mut UpdateWorld, memo: String) {
    w.args.memo = Some(Memo::try_from(memo).unwrap());
}

#[given(expr = "removing the token owner")]
fn given_rm_owner(w: &mut UpdateWorld) {
    w.args.owner = Some(TokenMaybeOwner::Right(CborNull));
}

#[when(expr = "I update the token as {id}")]
fn when_update_ticker(w: &mut UpdateWorld, id: SomeId) {
    let id = id.as_address(w);
    w.setup
        .module_impl
        .update(&id, w.args.clone())
        .expect("Unable to update token ticker");

    let res = LedgerTokensModuleBackend::info(
        &w.setup.module_impl,
        &w.setup.id,
        TokenInfoArgs {
            symbol: w.info.symbol,
            ..Default::default()
        },
    )
    .expect("Unable to fetch token info");
    w.info = res.info;
}

#[then(expr = "the token new ticker is {word}")]
fn then_new_ticker(w: &mut UpdateWorld, ticker: String) {
    assert_eq!(w.info.summary.ticker, ticker);
}

#[then(expr = "the token new name is {word}")]
fn then_new_name(w: &mut UpdateWorld, name: String) {
    assert_eq!(w.info.summary.name, name);
}

#[then(expr = "the token new decimal is {int}")]
fn then_new_decimal(w: &mut UpdateWorld, decimal: u64) {
    assert_eq!(w.info.summary.decimals, decimal);
}

#[then(expr = "the memo is {string}")]
fn then_memo(w: &mut UpdateWorld, memo: String) {
    let res = EventsModuleBackend::list(
        &w.setup.module_impl,
        ListArgs {
            filter: Some(EventFilter {
                kind: Some(vec![EventKind::TokenUpdate].into()),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .expect("Unable to list TokenUpdate event");
    let memo = Memo::try_from(memo).unwrap();
    assert!(res.nb_events >= 1);
    let event = res.events.into_iter().next().expect("Expected an event");
    assert!(event.content.memo().is_some());
    assert_eq!(event.content.memo().unwrap(), &memo);
}

#[then(expr = "the token new owner is {id}")]
fn then_new_owner(w: &mut UpdateWorld, owner: SomeId) {
    let owner = owner.as_address(w);
    assert_eq!(w.info.owner, Some(owner));
}

#[then(expr = "the token owner is removed")]
fn then_rm_owner(w: &mut UpdateWorld) {
    assert!(w.info.owner.is_none());
}

#[then(expr = "updating the token as {id} fails with {error}")]
fn then_update_token_fail_acl(w: &mut UpdateWorld, id: SomeId, error: SomeError) {
    let id = id.as_address(w);
    fail_update_token(w, &id);
    verify_error_code(w, error.as_many_code())
}

#[then(expr = "the error role is {word}")]
fn then_error_role(w: &mut UpdateWorld, role: String) {
    verify_error_role(w, role.as_str());
}

#[then(expr = "the error ticker is {word}")]
fn error_ticker_is(w: &mut UpdateWorld, ticker: String) {
    verify_error_ticker(w, ticker);
}

#[tokio::main]
async fn main() {
    // Support both Cargo and Bazel paths
    let features = ["tests/features", "src/many-ledger/tests/features"]
        .into_iter()
        .find(|&p| Path::new(p).exists())
        .expect("Cucumber test features not found");

    UpdateWorld::run(Path::new(features).join("ledger_tokens/update_token.feature")).await;
}
