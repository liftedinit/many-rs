GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
START_BALANCE=100000000000
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    start_ledger --pem "$(pem 0)" \
        --cache \
        "--balance-only-for-testing=$(identity 1):$START_BALANCE:$MFX_ADDRESS"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: using --cache_db will detect duplicate transactions" {
    account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canLedgerTransact"] }, 2: [0] }')
    call_ledger --pem=1 --port=8000 send "$account_id" 1000000 MFX
    check_consistency --pem=1 --balance=1000000 --id="$account_id" 8000

    call_ledger --pem=1 --port=8000 send --account="$account_id" "$(identity 4)" 2000 MFX
    check_consistency --pem=4 --balance=2000 --id="$(identity 4)" 8000
    check_consistency --pem=1 --balance=998000 --id="$account_id" 8000

    msg_hex="$(many message --hex --pem "$(pem 2)" ledger.send "{ 1: \"$(identity 3)\", 2: 1000, 3: \"$MFX_ADDRESS\" }")"

    # Send the transaction to MANY-ABCI. It should succeed.
    many message --server http://localhost:8000 --from-hex="$msg_hex" 2>&3 >&3

    check_consistency --pem=1 --balance=999000 --id="$(identity 2)" 8000
    check_consistency --pem=1 --balance=1001 --id="$(identity 3)" 8000

    # Send the same transaction to MANY-ABCI, again. It should FAIL because it's
    # a duplicate.
    run many message --server http://localhost:8000 --from-hex="$msg_hex" 2>&3 >&3
    assert_output --partial "tx already exists in cache"
}
