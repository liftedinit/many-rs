# Resiliency test verifying the Token Migration
# New token metadata are stored in the DB after the migration
# Token endpoints should be activated after the migration

GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MIGRATION_ROOT="$GIT_ROOT/staging/ledger_migrations.json"
MAKEFILE="Makefile.ledger"
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    # jq doesn't support bigint
    jq '(.migrations[] | select(.name == "Token Migration")).block_height |= 30 |
        (.migrations[] | select(.name == "Token Migration")).disabled |= empty |
        (.migrations[] | select(.name == "Disable Token Mint Migration")).block_height |= 35 |
        (.migrations[] | select(.name == "Disable Token Mint Migration")).disabled |= empty |
        (.migrations[] | select(.name == "Token Migration")) |= . + {
            "token_identity": "'$(identity 1)'",
            "token_next_subresource": 0,
            "symbol": "'${MFX_ADDRESS}'",
            "symbol_name": "Manifest Network Token",
            "symbol_decimals": 9,
            "symbol_total": 100000000000000,
            "symbol_circulating": 100000000000000,
            "symbol_maximum": null,
            "symbol_owner": "'${MFX_ADDRESS}'"
        }' \
        "$MIGRATION_ROOT" > "$BATS_TEST_ROOTDIR/migrations.json"

    (
      cd "$GIT_ROOT/docker/" || exit
      make -f $MAKEFILE clean
      make -f $MAKEFILE start-nodes-detached \
          ID_WITH_BALANCES="$(identity 1):1000000" \
          MIGRATIONS="$BATS_TEST_ROOTDIR/migrations.json" || {
        echo '# Could not start nodes...' >&3
        exit 1
      }
    ) > /dev/null

    # Give time to the servers to start.
    wait_for_server 8000 8001 8002 8003
}

function teardown() {
    (
      cd "$GIT_ROOT/docker/" || exit 1
      make -f $MAKEFILE stop-nodes
    ) 2> /dev/null

    # Fix for BATS verbose run/test output gathering
    cd "$GIT_ROOT/tests/resiliency/ledger" || exit 1
}

@test "$SUITE: Disable Token Minting Migration" {
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003

    # Token endpoints should be disabled
    call_ledger --pem=1 --port=8000 token info ${MFX_ADDRESS}
    assert_output --partial "Invalid method name"

    # Enable Token Migration
    wait_for_block 30

    # Token endpoints should be enabled. Mint some tokens
    call_ledger --pem=1 --port=8000 token mint MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\'''
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003
    check_consistency --pem=2 --balance=123 8000 8001 8002 8003
    check_consistency --pem=3 --balance=456 8000 8001 8002 8003

    # Disable Token Minting
    wait_for_block 35

    # Token endpoints should be disabled
    call_ledger --pem=1 --port=8000 token mint MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\'''
    assert_output --partial "Token minting is disabled on this network"
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003
    check_consistency --pem=2 --balance=123 8000 8001 8002 8003
    check_consistency --pem=3 --balance=456 8000 8001 8002 8003
}