GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MIGRATION_ROOT="$GIT_ROOT/tests/ledger_migrations.json"
MAKEFILE="Makefile.ledger"

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    jq '(.migrations[] | select(.name == "Dummy Hotfix")).block_height |= 20 |
        (.migrations[] | select(.name == "Dummy Hotfix")).disabled |= empty' \
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

@test "$SUITE: Dummy Hotfix" {
    local account_id
    local tx_id

    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003

    # Create a new account where 1 is the owner and 2 can approve transactions
    account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canMultisigApprove"] }, 2: [[1, { 0: 2 }]] }')

    # Transfer MFX to the account
    call_ledger --pem=1 --port=8000 send "$account_id" 1000000 MFX
    check_consistency --pem=1 --balance=1000000 --id="$account_id" 8000 8001 8002 8003

    # Submit a new multisig as 1
    call_ledger --pem=1 --port=8000 multisig submit --execute-automatically true "$account_id" send "$(identity 3)" 1000 MFX
    assert_output --partial "Transaction Token"
    tx_id=$(echo "$output" | grep "Transaction Token" | grep -oE "[0-9a-f]+$")

    wait_for_block 19

    # Approve and execute the transaction
    # At this point the Dummy Hotfix should execute!
    call_ledger --pem=2 --port=8000 multisig approve "$tx_id"

    # Retrieve the send event from the multisig transaction
    # Check that the timestamp has been fixed
    run many_message --pem=1 events.list "{2: {1: [[9, [1, 3]]]}}"
    assert_output --partial "5: 1(1234567890_2)"
}
