GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
START_BALANCE=100000000000
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    (
      cd "$GIT_ROOT"
      cargo build --features balance_testing
    )

    start_ledger --pem "$(pem 0)" \
          "--balance-only-for-testing=$(identity 1):$START_BALANCE:$MFX_ADDRESS" \
          "--balance-only-for-testing=$(identity 2):$START_BALANCE:$MFX_ADDRESS"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: ledger can send tokens on behalf of an account" {
    account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canLedgerTransact"] }, 2: [0] }')
    call_ledger --pem=1 --port=8000 send "$account_id" 1000000 MFX
    check_consistency --pem=1 --balance=1000000 --id="$account_id" 8000

    call_ledger --pem=1 --port=8000 send --account="$account_id" "$(identity 4)" 2000 MFX
    check_consistency --pem=4 --balance=2000 --id="$(identity 4)" 8000
    check_consistency --pem=1 --balance=998000 --id="$account_id" 8000

    call_ledger --pem=2 --port=8000 send --account="$account_id" "$(identity 4)" 2000 MFX
    check_consistency --pem=4 --balance=4000 --id="$(identity 4)" 8000
    check_consistency --pem=1 --balance=996000 --id="$account_id" 8000

    call_ledger --pem=4 --port=8000 send --account="$account_id" "$(identity 4)" 2000 MFX
    assert_output --partial "Sender needs role 'canLedgerTransact' to perform this operation."
}
