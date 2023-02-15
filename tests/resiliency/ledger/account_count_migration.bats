GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MIGRATION_ROOT="$GIT_ROOT/tests/ledger_migrations.json"
MAKEFILE="Makefile.ledger"

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    jq '(.migrations[] | select(.name == "Account Count Data Attribute")).block_height |= 20 |
        (.migrations[] | select(.name == "Account Count Data Attribute")).disabled |= empty' \
        "$MIGRATION_ROOT" > "$BATS_TEST_ROOTDIR/migrations.json"

    (
      cd "$GIT_ROOT/docker/" || exit
      make -f $MAKEFILE clean
      make -f $MAKEFILE start-nodes-dettached \
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

@test "$SUITE: Account Count" {
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003

    run many_message --pem=0 data.info
    assert_output "[[]]"

    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX
    check_consistency --pem=1 --balance=999000 --id="$(identity 1)" 8000 8001 8002 8003
    check_consistency --pem=2 --balance=1000 --id="$(identity 2)" 8000 8001 8002 8003

    wait_for_block 20

    # Test the new commands
    run many_message --pem=0 data.info
    assert_output --partial "[[0, [2, 0]], [0, [2, 1]]]"
    run many_message --pem=0 data.getInfo "[[[0, [2, 0]], [0, [2, 1]]]]"
    assert_output --partial "[0, [2, 0]]: [[0, []], \"accountTotalCount\"]"
    assert_output --partial "[0, [2, 1]]: [[0, []], \"nonZeroAccountTotalCount\"]"
    run many_message --pem=0 data.query "[[[0, [2, 0]], [0, [2, 1]]]]"
    assert_output --partial "[0, [2, 0]]: [0, [3]],"
    assert_output --partial "[0, [2, 1]]: [0, [3]],"

    # Check if the chain is still consistent
    check_consistency --pem=1 --balance=999000 --id="$(identity 1)" 8000 8001 8002 8003
    check_consistency --pem=2 --balance=1000 --id="$(identity 2)" 8000 8001 8002 8003

    call_ledger --pem=2 --port=8000 send "$(identity 1)" 1000 MFX
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003
    check_consistency --pem=2 --balance=0 --id="$(identity 2)" 8000 8001 8002 8003
    run many_message --pem=0 data.query "[[[0, [2, 0]], [0, [2, 1]]]]"
    assert_output --partial "[0, [2, 0]]: [0, [3]],"
    assert_output --partial "[0, [2, 1]]: [0, [2]],"
}
