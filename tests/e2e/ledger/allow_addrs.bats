GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
START_BALANCE=1000000
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

    local ALLOW_ADDRS_CONFIG=$(generate_allow_addrs_config 1 2)

    start_ledger --pem "$(pem 0)" \
          --allow-addrs "$ALLOW_ADDRS_CONFIG" \
          "--balance-only-for-testing=$(identity 1):$START_BALANCE:$MFX_ADDRESS"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: allow addrs" {
    check_consistency --pem=1 --balance=1000000 8000

    call_ledger --pem=1 --port=8000 send "$(identity 2)" 2000 MFX
    check_consistency --pem=1 --balance=998000 8000
    check_consistency --pem=2 --balance=2000 8000

    call_ledger --pem=2 --port=8000 send "$(identity 3)" 1000 MFX
    check_consistency --pem=1 --balance=998000 8000
    check_consistency --pem=2 --balance=1000 8000
    check_consistency --pem=3 --balance=1000 8000

    call_ledger --pem=3 --port=8000 send "$(identity 1)" 500 MFX
    assert_output --partial "The identity of the from field is invalid or unexpected."

    check_consistency --pem=1 --balance=998000 8000
    check_consistency --pem=2 --balance=1000 8000
    check_consistency --pem=3 --balance=1000 8000
}
