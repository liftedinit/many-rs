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

@test "$SUITE: request and response works" {
    account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canLedgerTransact"] }, 2: [0] }')
    call_ledger --pem=1 --port=8000 send "$account_id" 14159265359 MFX
    token=$(echo "$output" | grep -oE "token: .*" | colrm 1 7)

    run many message --server http://localhost:8000 blockchain.request "{ 0: { 0: h'$token' } }"
    # Check the content of the message above. We only check the method itself
    # and the amount, since addresses might change between test runs.
    assert_output --partial 6b6c65646765722e73656e64  # "ledger.send"
    assert_output --partial 021b000000034bf53e4f  # 2 => 14159265359

    run many message --server http://localhost:8000 blockchain.response "{ 0: { 0: h'$token' } }"
    # The response should be fairly consistent, since in this case the time is set
    # to epoch.
    assert_output --partial "8440a0582dd92712a302d92710581d$(identity_hex 1)0441f605c10040"
}
