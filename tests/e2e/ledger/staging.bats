GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    (
      cd "$GIT_ROOT"
      cargo build --features balance_testing
    )

    cp "$GIT_ROOT/staging/ledger_state.json5" "$BATS_TEST_ROOTDIR/ledger_state.json5"
    sed -i.bak 's/token_identity: ".*"/token_identity: "'"$(identity 1)"'"/' "$BATS_TEST_ROOTDIR/ledger_state.json5"
    sed -i.bak 's/account_identity: ".*"/account_identity: "'"$(identity 1)"'"/' "$BATS_TEST_ROOTDIR/ledger_state.json5"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: Make sure token and account identities are different" {
    run "$GIT_ROOT/target/debug/many-ledger" \
        --pem $(pem 1) \
        -v \
        --clean \
        --persistent "ledger.db" \
        --state "$BATS_TEST_ROOTDIR/ledger_state.json5"
    assert_output --partial "Token and account identities must be different."
}
