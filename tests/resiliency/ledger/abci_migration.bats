GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
#MIGRATION_ROOT="$GIT_ROOT/tests/ledger_migrations.json"
ABCI_MIGRATION_ROOT="$GIT_ROOT/tests/abci_migrations.json"
MAKEFILE="Makefile.ledger"

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    cp "$ABCI_MIGRATION_ROOT" "$BATS_TEST_ROOTDIR/abci_migrations.json"

    (
      cd "$GIT_ROOT/docker/" || exit
      make -f $MAKEFILE clean
      make -f $MAKEFILE start-nodes-detached \
          ID_WITH_BALANCES="$(identity 1):1000000" \
          ABCI_MIGRATIONS="$BATS_TEST_ROOTDIR/abci_migrations.json" || {
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

@test "$SUITE: Legacy Error Code Migration" {
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003

    # Verify that the ledger works, for falsibility, you know.
    run call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX
    check_consistency --pem=1 --balance=999000 --id="$(identity 1)" 8000 8001 8002 8003

    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000001 MFX
    assert_output --regexp "Code: -20003"
    assert_output --regexp "Insufficient funds."

    wait_for_block 20
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000002 MFX
    assert_output --regexp "Code: -1"
    assert_output --regexp "Insufficient funds."

    wait_for_block 30
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000003 MFX
    assert_output --regexp "Code: -1"
    assert_output --regexp "Insufficient funds."

    wait_for_block 40
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000004 MFX
    assert_output --regexp "Code: -20003"
    assert_output --regexp "Insufficient funds."
}
