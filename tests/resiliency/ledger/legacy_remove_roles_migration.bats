GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MIGRATION_ROOT="$GIT_ROOT/tests/ledger_migrations.json"
MAKEFILE="Makefile.ledger"

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    jq '(.migrations[] | select(.name == "LegacyRemoveRoles")).block_height |= 20 |
        (.migrations[] | select(.name == "LegacyRemoveRoles")).upper_block_height |= 40 |
        (.migrations[] | select(.name == "LegacyRemoveRoles")).disabled |= empty' \
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

@test "$SUITE: Legacy Remove Roles" {
    local account_id

    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003

    # Create a new account where 1 is the owner and 2 and 3 can approve transactions
    account_id=$(account_create --pem=1 '{2: [[1, { 0: 2 }]] }')
    run many_message --pem=1 account.addRoles '{0: "'"$account_id"'", 1: {"'"$(identity 2)"'": ["canMultisigApprove"]}}'
    run many_message --pem=1 account.addRoles '{0: "'"$account_id"'", 1: {"'"$(identity 3)"'": ["canMultisigApprove"]}}'
    run many_message --pem=1 account.addRoles '{0: "'"$account_id"'", 1: {"'"$(identity 4)"'": ["canMultisigApprove"]}}'

    run many_message --pem=1 account.info '{0: "'"$account_id"'"}'
    assert_output --partial "$(identity_hex 2)"
    assert_output --partial "$(identity_hex 3)"
    assert_output --partial "$(identity_hex 4)"

    # Remove role using new behavior
    run many_message --pem=1 account.removeRoles '{0: "'"$account_id"'", 1: {"'"$(identity 2)"'": ["canMultisigApprove"]}}'
    run many_message --pem=1 account.info '{0: "'"$account_id"'"}'
    refute_output --partial "$(identity_hex 2)"

    wait_for_block 20

    # Remove role using legacy behavior
    run many_message --pem=1 account.removeRoles '{0: "'"$account_id"'", 1: {"'"$(identity 3)"'": ["canMultisigApprove"]}}'
    run many_message --pem=1 account.info '{0: "'"$account_id"'"}'
    assert_output --regexp ".*$(identity_hex 3).*: \[\],"
    refute_output --partial "$(identity_hex 2)"

    wait_for_block 40

    # Remove role using new behavior again
    run many_message --pem=1 account.removeRoles '{0: "'"$account_id"'", 1: {"'"$(identity 4)"'": ["canMultisigApprove"]}}'
    run many_message --pem=1 account.info '{0: "'"$account_id"'"}'
    refute_output --partial "$(identity_hex 2)"
    assert_output --regexp ".*$(identity_hex 3).*: \[\],"
    refute_output --partial "$(identity_hex 4)"
}
