GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
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
      for i in {0..2}
      do
          make -f $MAKEFILE start-single-node-detached-${i} ID_WITH_BALANCES="$(identity 1):1000000:$MFX_ADDRESS" || {
            echo '# Could not start nodes...' >&3
            exit 1
          }
      done
    ) > /dev/null

    # Give time to the servers to start.
    wait_for_server 8000 8001 8002
}

function teardown() {
    (
      cd "$GIT_ROOT/docker/" || exit 1
      make -f $MAKEFILE stop-nodes
    ) 2> /dev/null

    # Fix for BATS verbose run/test output gathering
    cd "$GIT_ROOT/tests/resiliency/ledger" || exit 1
}

@test "$SUITE: Node can catch up" {
    # Check consistency with nodes [0, 2] up
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX
    check_consistency --pem=1 --balance=999000 --id="$(identity 1)" 8000 8001 8002
    check_consistency --pem=2 --balance=1000 --id="$(identity 2)" 8000 8001 8002

    call_ledger --pem=1 --port=8001 send "$(identity 2)" 1000 MFX
    call_ledger --pem=1 --port=8001 send "$(identity 2)" 1000 MFX
    check_consistency --pem=1 --balance=997000 --id="$(identity 1)" 8000 8001 8002
    check_consistency --pem=2 --balance=3000 --id="$(identity 2)" 8000 8001 8002

    call_ledger --pem=1 --port=8002 send "$(identity 2)" 1000 MFX
    call_ledger --pem=1 --port=8002 send "$(identity 2)" 1000 MFX
    call_ledger --pem=1 --port=8002 send "$(identity 2)" 1000 MFX
    check_consistency --pem=1 --balance=994000 --id="$(identity 1)" 8000 8001 8002
    check_consistency --pem=2 --balance=6000 --id="$(identity 2)" 8000 8001 8002

    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX
    check_consistency --pem=1 --balance=990000 --id="$(identity 1)" 8000 8001 8002
    check_consistency --pem=2 --balance=10000 --id="$(identity 2)" 8000 8001 8002

    cd "$GIT_ROOT/docker/" || exit 1

    sleep 300

    # At this point, start the 4th node and check it can catch up
    make -f $MAKEFILE start-single-node-detached-3 ID_WITH_BALANCES="$(identity 1):1000000" || {
      echo '# Could not start nodes...' >&3
      exit 1
    }

    # Give the 4th node some time to boot
    wait_for_server 8003

    sleep 12  # Three consensus round.
    check_consistency --pem=1 --balance=990000 --id="$(identity 1)" 8000 8001 8002 8003
    check_consistency --pem=2 --balance=10000 --id="$(identity 2)" 8000 8001 8002 8003
}
