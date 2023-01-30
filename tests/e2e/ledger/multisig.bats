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

@test "$SUITE: Ledger shows a balance and can send tokens" {
    check_consistency --pem=1 --balance=$START_BALANCE --id="$(identity 1)" 8000

    call_ledger --pem=1 --port=8000 send "$(identity 3)" 1000 MFX
    check_consistency --pem=3 --balance=1000 --id="$(identity 3)" 8000
    check_consistency --pem=1 --balance=$((START_BALANCE - 1000)) --id="$(identity 1)" 8000

    check_consistency --pem=2 --balance=$START_BALANCE --id="$(identity 2)" 8000
}

@test "$SUITE: Ledger can do account creation and multisig transactions" {
    local account_id
    local tx_id

    check_consistency --pem=1 --balance=$START_BALANCE --id="$(identity 1)" 8000
    account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canMultisigApprove"] }, 2: [[1, { 0: 2 }]] }')

    call_ledger --pem=1 --port=8000 send "$account_id" 1000000 MFX
    check_consistency --pem=1 --balance=1000000 --id="$account_id" 8000

    call_ledger --pem=1 --port=8000 multisig submit "$account_id" send "$(identity 3)" 100 MFX
    tx_id=$(echo "$output" | grep -oE "[0-9a-f]+$")
    # Cannot execute if not approved.
    call_ledger --pem=1 --port=8000 multisig execute "$tx_id"
    assert_output --partial "This transaction cannot be executed yet."

    call_ledger --pem=2 --port=8000 multisig approve "$tx_id"

    # Cannot execute if not submitted.
    call_ledger --pem=2 --port=8000 multisig execute "$tx_id"
    assert_output --partial "This transaction cannot be executed yet."

    call_ledger --pem=1 --port=8000 multisig execute "$tx_id"

    check_consistency --pem=1 --balance=999900 --id="$account_id" 8000
    check_consistency --pem=3 --balance=100 --id="$(identity 3)" 8000
}

@test "$SUITE: can revoke" {
    local account_id
    local tx_id

    check_consistency --pem=1 --balance=$START_BALANCE --id="$(identity 1)" 8000
    account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canMultisigApprove"] }, 2: [[1, { 0: 2 }]] }')

    call_ledger --pem=1 --port=8000 send "$account_id" 1000000 MFX
    check_consistency --pem=1 --balance=1000000 --id="$account_id" 8000

    call_ledger --pem=1 --port=8000 multisig submit "$account_id" send "$(identity 3)" 100 MFX
    tx_id=$(echo "$output" | grep -oE "[0-9a-f]+$")

    call_ledger --pem=2 --port=8000 multisig approve "$tx_id"
    call_ledger --pem=1 --port=8000 multisig revoke "$tx_id"

    call_ledger --pem=1 --port=8000 multisig execute "$tx_id"
    assert_output --partial "This transaction cannot be executed yet."

    call_ledger --pem=1 --port=8000 multisig approve "$tx_id"
    call_ledger --pem=2 --port=8000 multisig revoke "$tx_id"
    call_ledger --pem=1 --port=8000 multisig execute "$tx_id"
    assert_output --partial "This transaction cannot be executed yet."

    call_ledger --pem=2 --port=8000 multisig approve "$tx_id"
    call_ledger --pem=1 --port=8000 multisig execute "$tx_id"

    check_consistency --pem=3 --balance=100 --id="$(identity 3)" 8000
}
