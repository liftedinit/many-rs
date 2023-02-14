GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities
}

function teardown() {
    stop_background_run
}

@test "$SUITE: Make sure token and account identities are different" {
    cp "$GIT_ROOT/staging/ledger_state.json5" "$BATS_TEST_ROOTDIR/ledger_state.json5"
    sed -i.bak 's/token_identity: ".*"/token_identity: "'"$(identity 1)"'"/' "$BATS_TEST_ROOTDIR/ledger_state.json5"
    sed -i.bak 's/account_identity: ".*"/account_identity: "'"$(identity 1)"'"/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    run many-ledger \
        --pem $(pem 1) \
        -v \
        --clean \
        --persistent "ledger.db" \
        --state "$BATS_TEST_ROOTDIR/ledger_state.json5"
    assert_output --partial "Token and account identities must be different."
}

@test "$SUITE: Test configuration hash" {
    cp "$GIT_ROOT/staging/ledger_state.json5" "$BATS_TEST_ROOTDIR/ledger_state.json5"
    sed -i.bak '2i\  id_store_seed: 1000,' "$BATS_TEST_ROOTDIR/ledger_state.json5"
    sed -i.bak '3i\  id_store_keys: { "YQo=": "Ygo=", "Ywo=": "ZAo=" },' "$BATS_TEST_ROOTDIR/ledger_state.json5"
    sed -i.bak 's/fc0041ca4f7d959fe9e5a337e175bd8a68942cad76745711a3daf820a159f7eb/0a5c754ccb0327b9e3c3bf8980a8225e0b56ab7268ea05eea48f7294c3cb32bf/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    run_in_background many-ledger  \
        --pem $(pem 1) \
        -v \
        --clean \
        --persistent "ledger.db" \
        --state "$BATS_TEST_ROOTDIR/ledger_state.json5"
    wait_for_background_output "Running accept thread"
}
