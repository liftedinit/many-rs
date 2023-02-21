use many_ledger_test_macros::*;
use many_ledger_test_utils::cucumber::{
    refresh_token_info, verify_error_code, verify_error_role, AccountWorld, LedgerWorld, SomeError,
    SomeId, SomePermission, TokenWorld,
};
use many_ledger_test_utils::Setup;
use std::path::Path;

use cucumber::{given, then, when, World};
use many_error::ManyError;
use many_identity::Address;
use many_ledger::migration::tokens::TOKEN_MIGRATION;
use many_ledger::module::LedgerModuleImpl;
use many_modules::events::{EventFilter, EventKind, EventsModuleBackend, ListArgs};
use many_modules::ledger::extended_info::visual_logo::VisualTokenLogo;
use many_modules::ledger::extended_info::TokenExtendedInfo;
use many_modules::ledger::{LedgerTokensModuleBackend, TokenAddExtendedInfoArgs};
use many_types::ledger::TokenInfo;
use many_types::Memo;

#[derive(World, Debug, Default, LedgerWorld, TokenWorld, AccountWorld)]
#[world(init = Self::new)]
struct AddExtInfoWorld {
    setup: Setup,
    args: TokenAddExtendedInfoArgs,
    info: TokenInfo,
    ext_info: TokenExtendedInfo,
    account: Address,
    error: Option<ManyError>,
}

impl AddExtInfoWorld {
    fn new() -> Self {
        Self {
            setup: Setup::new_with_migrations(false, [(0, &TOKEN_MIGRATION)], true),
            ..Default::default()
        }
    }
}

fn fail_add_ext_info_token(w: &mut AddExtInfoWorld, sender: &Address) {
    w.error = Some(
        LedgerTokensModuleBackend::add_extended_info(
            &mut w.setup.module_impl,
            sender,
            w.args.clone(),
        )
        .expect_err("Token add extended info was supposed to fail, it succeeded instead."),
    );
}
#[given(expr = "a token account")]
fn given_token_account(w: &mut AddExtInfoWorld) {
    many_ledger_test_utils::cucumber::given_token_account(w);
}

#[given(expr = "{id} as the account owner")]
fn given_account_id_owner(w: &mut AddExtInfoWorld, id: SomeId) {
    many_ledger_test_utils::cucumber::given_account_id_owner(w, id);
}

#[given(expr = "{id} has {permission} permission")]
fn given_account_part_of_can_create(
    w: &mut AddExtInfoWorld,
    id: SomeId,
    permission: SomePermission,
) {
    many_ledger_test_utils::cucumber::given_account_part_of_can_create(w, id, permission);
}

#[given(expr = "a default token owned by {id}")]
fn create_default_token(w: &mut AddExtInfoWorld, id: SomeId) {
    many_ledger_test_utils::cucumber::create_default_token(w, id);
    w.args.symbol = w.info.symbol;
    refresh_token_info(w);
}

#[given(expr = "a memo {string}")]
fn given_memo(w: &mut AddExtInfoWorld, memo: String) {
    w.args.extended_info = TokenExtendedInfo::new()
        .with_memo(Memo::try_from(memo).expect("Unable to create memo"))
        .expect("Unable to set extended info memo");
}

#[given(expr = "an unicode logo {word}")]
fn given_unicode_logo(w: &mut AddExtInfoWorld, unicode_char: char) {
    let mut logo = VisualTokenLogo::new();
    logo.unicode_front(unicode_char);
    w.args.extended_info = TokenExtendedInfo::new()
        .with_visual_logo(logo)
        .expect("Unable to set extended info logo");
}

#[given(expr = "a {word} image logo {string}")]
fn given_string_logo(w: &mut AddExtInfoWorld, content_type: String, data: String) {
    let mut logo = VisualTokenLogo::new();
    logo.image_front(content_type, data.into_bytes());
    w.args.extended_info = TokenExtendedInfo::new()
        .with_visual_logo(logo)
        .expect("Unable to set extended info logo");
}

#[given(expr = "an event memo {string}")]
fn given_event_memo(w: &mut AddExtInfoWorld, memo: String) {
    w.args.memo = Some(Memo::try_from(memo).unwrap());
}

#[when(expr = "I add the extended info to the token as {id}")]
fn add_ext_info(w: &mut AddExtInfoWorld, id: SomeId) {
    let id = id.as_address(w);
    w.setup
        .module_impl
        .add_extended_info(&id, w.args.clone())
        .expect("Unable to add extended info");

    refresh_token_info(w);
}

#[then(expr = "the token has the memo {string}")]
fn then_has_memo(w: &mut AddExtInfoWorld, memo: String) {
    assert!(w.ext_info.memo().is_some());
    assert_eq!(w.ext_info.memo().unwrap(), &Memo::try_from(memo).unwrap());
}

#[then(expr = "the token has the unicode logo {word}")]
fn then_has_unicode_logo(w: &mut AddExtInfoWorld, unicode_char: char) {
    assert!(w.ext_info.visual_logo().is_some());
    let mut logo = VisualTokenLogo::new();
    logo.unicode_front(unicode_char);
    assert_eq!(w.ext_info.visual_logo().unwrap(), &logo);
}

#[then(expr = "the token has the {word} image logo {string}")]
fn then_has_image_logo(w: &mut AddExtInfoWorld, content_type: String, data: String) {
    assert!(w.ext_info.visual_logo().is_some());
    let mut logo = VisualTokenLogo::new();
    logo.image_front(content_type, data.into_bytes());
    assert_eq!(w.ext_info.visual_logo().unwrap(), &logo);
}

#[then(expr = "adding extended info to the token as {id} fails with {error}")]
fn then_add_ext_info_token_fail_acl(w: &mut AddExtInfoWorld, id: SomeId, error: SomeError) {
    let id = id.as_address(w);
    fail_add_ext_info_token(w, &id);
    verify_error_code(w, error.as_many_code())
}

#[then(expr = "the error role is {word}")]
fn then_error_role(w: &mut AddExtInfoWorld, role: String) {
    verify_error_role(w, role.as_str());
}

#[then(expr = "the event memo is {string}")]
fn then_memo(w: &mut AddExtInfoWorld, memo: String) {
    let res = EventsModuleBackend::list(
        &w.setup.module_impl,
        ListArgs {
            filter: Some(EventFilter {
                kind: Some(vec![EventKind::TokenAddExtendedInfo].into()),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .expect("Unable to list TokenAddExtendedInfo event");
    let memo = Memo::try_from(memo).unwrap();
    assert!(res.nb_events >= 1);
    let event = res.events.into_iter().next().expect("Expected an event");
    assert!(event.content.memo().is_some());
    assert_eq!(event.content.memo().unwrap(), &memo);
}

#[tokio::main]
async fn main() {
    // Support both Cargo and Bazel paths
    let features = ["tests/features", "src/many-ledger/tests/features"]
        .into_iter()
        .find(|&p| Path::new(p).exists())
        .expect("Cucumber test features not found");

    AddExtInfoWorld::run(Path::new(features).join("ledger_tokens/add_token_ext_info.feature"))
        .await;
}
