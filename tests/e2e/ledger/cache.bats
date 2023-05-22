GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
START_BALANCE=1000000
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    start_ledger --cache \
        --pem "$(pem 0)" \
        "--balance-only-for-testing=$(identity 1):$START_BALANCE:$MFX_ADDRESS"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: using --cache_db will detect duplicate transactions" {
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 2000 MFX
    check_consistency --pem=1 --balance=998000 --id="$(identity 1)" 8000
    check_consistency --pem=2 --balance=2000 --id="$(identity 2)" 8000

    msg_hex="$(many message --hex --pem "$(pem 2)" ledger.send "{ 1: \"$(identity 3)\", 2: 1000, 3: \"$MFX_ADDRESS\" }")"

    # Send the transaction to MANY-ABCI. It should succeed.
    many message --server http://localhost:8000 --from-hex="$msg_hex" 2>&3 >&3

    check_consistency --pem=1 --balance=998000 --id="$(identity 1)" 8000
    check_consistency --pem=2 --balance=1000 --id="$(identity 2)" 8000
    check_consistency --pem=1 --balance=1000 --id="$(identity 3)" 8000

    # Send the same transaction to MANY-ABCI, again. It should FAIL because it's
    # a duplicate.
    run many message --server http://localhost:8000 --from-hex="$msg_hex" 2>&3 >&3
    assert_output --partial "This message was already processed"
}
