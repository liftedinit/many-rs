GIT_ROOT="$BATS_TEST_DIRNAME/../../../"

load '../../test_helper/load'
load '../../test_helper/kvstore'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    start_kvstore --cache --pem "$(pem 0)"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: using --cache_db will detect duplicate transactions" {
    call_kvstore --pem=1 --port=8000 put "010203" "foobar"
    call_kvstore --pem=1 --port=8000 get "010203"
    assert_output --partial "foobar"

    msg_hex="$(many message --hex --pem "$(pem 1)" kvstore.put "{ 0: '0', 1: 'hello' }")"

    # Send the transaction to MANY-ABCI. It should succeed.
    many message --server http://localhost:8000 --from-hex="$msg_hex" 2>&3 >&3

    call_kvstore --pem=1 --port=8000 get "0"
    assert_output --partial "hello"

    # Send the same transaction to MANY-ABCI, again. It should FAIL because it's
    # a duplicate.
    run many message --server http://localhost:8000 --from-hex="$msg_hex" 2>&3 >&3
    assert_output --partial "This message was already processed"

    # Verify the test can be falsified.
    msg_hex="$(many message --hex --pem "$(pem 1)" kvstore.put "{ 0: '0', 1: 'world' }")"
    many message --server http://localhost:8000 --from-hex="$msg_hex" 2>&3 >&3
    call_kvstore --pem=1 --port=8000 get "0"
    assert_output --partial "world"
}
