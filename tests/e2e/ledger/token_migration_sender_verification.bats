# e2e tests for the token feature set
# The Token Migration needs to be active for this feature set to be enabled.

GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz
START_BALANCE=100000000000
MIGRATION_ROOT="$GIT_ROOT/staging/ledger_migrations.json"
load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    jq '(.migrations[] | select(.name == "Token Migration")).block_height |= 0 |
        (.migrations[] | select(.name == "Token Migration")).disabled |= empty' \
        "$MIGRATION_ROOT" > "$BATS_TEST_ROOTDIR/migrations.json"

    # Dummy image
     echo -n -e '\x68\x65\x6c\x6c\x6f' > "$BATS_TEST_ROOTDIR/image.png"

    # Activating the Token Migration from block 0 will modify the ledger staging hash
    # The symbol metadata will be stored in the DB
    cp "$GIT_ROOT/staging/ledger_state.json5" "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # The MFX address in the staging file is now `identity 1`
    sed -i.bak 's/'${MFX_ADDRESS}'/'$(subresource 1 1)'/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Make `identity 1` the token identity
    sed -i.bak 's/token_identity: ".*"/token_identity: "'"$(identity 1)"'"/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Use token identity subresource 0 as the first token symbol
    sed -i.bak 's/token_next_subresource: 2/token_next_subresource: 0/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Skip hash check
    sed -i.bak 's/hash/\/\/hash/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    start_ledger --state="$BATS_TEST_ROOTDIR/ledger_state.json5" \
        --pem "$(pem 0)" \
        --balance-only-for-testing="$(identity 8):$START_BALANCE:$(subresource 1 1)" \
        --migrations-config "$BATS_TEST_ROOTDIR/migrations.json"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: can't create as identity 2" {
    create_token --pem=2 --error=invalid_sender --port=8000
}

