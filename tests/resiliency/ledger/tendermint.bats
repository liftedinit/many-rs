GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
START_BALANCE=100000000000
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz
MAKEFILE="Makefile.ledger"

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    (
      cd "$GIT_ROOT/docker/" || exit
      make -f $MAKEFILE clean
      make -f $MAKEFILE start-nodes-detached ID_WITH_BALANCES="$(identity 1):$START_BALANCE" || {
        echo '# Could not start nodes...' >&3
        exit 1
      }
    ) > /dev/null

    # Give time to the servers to start.
    wait_for_server 8000 8001 8002 8003
}

function teardown() {
    stop_background_run
}

@test "$SUITE: will check transactions for timestamp" {
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000000 MFX
    call_ledger --pem=1 --port=8000 send "$(identity 3)" 1 MFX
    check_consistency --pem=1 --balance=1000000 --id="$(identity 2)" 8000
    check_consistency --pem=1 --balance=1 --id="$(identity 3)" 8000

    # Create a transaction in hexadecimal with a very old timestamp and
    msg_hex="$(many message --hex --pem "$(pem 2)" --timestamp 1 ledger.send "{ 1: \"$(identity 3)\", 2: 1000, 3: \"$MFX_ADDRESS\" }")"
    echo invalid: $msg_hex >&3

    # Send the transaction directly to tendermint to bypass the MANY server
    # code (like it would be done if we used the mempool directly).
    curl "http://localhost:26600/broadcast_tx_commit?tx=0x$msg_hex" >&3 || true

    echo "Waiting for the invalid transaction to be rejected..." >&3
    # It should not have executed.
    check_consistency --pem=1 --balance=1000000 --id="$(identity 2)" 8000
    check_consistency --pem=1 --balance=1 --id="$(identity 3)" 8000

    # Create another transaction with a valid timestamp to falsify the result
    # above.
    msg_hex="$(many message --hex --pem "$(pem 2)" ledger.send "{ 1: \"$(identity 3)\", 2: 1000, 3: \"$MFX_ADDRESS\" }")"
    echo valid: $msg_hex >&3

    # Send the transaction directly to tendermint to bypass the MANY server
    # code (like it would be done if we used the mempool directly).
    curl "http://localhost:26600/broadcast_tx_commit?tx=0x$msg_hex" >&3 || true

    echo "Waiting for the valid transaction to be processed..." >&3
    # It should have executed.
    check_consistency --pem=1 --balance=999000 --id="$(identity 2)" 8000
    check_consistency --pem=1 --balance=1001 --id="$(identity 3)" 8000

    # Create a transaction in hexadecimal with a very old timestamp and
    msg_hex="$(many message --hex --pem "$(pem 2)" --timestamp 1 ledger.send "{ 1: \"$(identity 3)\", 2: 1000, 3: \"$MFX_ADDRESS\" }")"
    echo invalid: $msg_hex >&3

    # Send the transaction directly to tendermint to bypass the MANY server
    # code (like it would be done if we used the mempool directly).
    curl "http://localhost:26600/broadcast_tx_commit?tx=0x$msg_hex" >&3 || true

    echo "Waiting for the invalid transaction to be rejected..." >&3
    # It should not have executed and should be the same balances above.
    check_consistency --pem=1 --balance=999000 --id="$(identity 2)" 8000
    check_consistency --pem=1 --balance=1001 --id="$(identity 3)" 8000
}
