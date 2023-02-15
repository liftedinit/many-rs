GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MAKEFILE="Makefile.kvstore"

load '../../test_helper/load'
load '../../test_helper/kvstore'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    (
      cd "$GIT_ROOT/docker/" || exit
      make -f $MAKEFILE clean
      make -f $MAKEFILE start-nodes-dettached || {
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
    cd "$GIT_ROOT/tests/resiliency/kvstore" || exit 1
}

@test "$SUITE: Network is consistent" {
    call_kvstore --pem=1 --port=8000 put foo bar
    sleep 4  # One consensus round.
    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002 8003

    call_kvstore --pem=1 --port=8001 put bar foo
    sleep 4  # One consensus round.
    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002 8003
    check_consistency --pem=1 --key=bar --value=foo 8000 8001 8002 8003

    call_kvstore --pem=1 --port=8002 put foobar barfoo
    sleep 4  # One consensus round.
    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002 8003
    check_consistency --pem=1 --key=bar --value=foo 8000 8001 8002 8003
    check_consistency --pem=1 --key=foobar --value=barfoo 8000 8001 8002 8003

    call_kvstore --pem=1 --port=8003 put barfoo foobar
    sleep 4  # One consensus round.
    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002 8003
    check_consistency --pem=1 --key=bar --value=foo 8000 8001 8002 8003
    check_consistency --pem=1 --key=foobar --value=barfoo 8000 8001 8002 8003
    check_consistency --pem=1 --key=barfoo --value=foobar 8000 8001 8002 8003
}

@test "$SUITE: Network is consistent with 1 node down" {
    cd "$GIT_ROOT/docker/" || exit 1

    # Bring down node 3.
    make -f $MAKEFILE stop-single-node-3

    call_kvstore --pem=1 --port=8000 put foo bar
    sleep 4  # One consensus round.
    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002

    call_kvstore --pem=1 --port=8001 put bar foo
    sleep 4  # One consensus round.
    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002
    check_consistency --pem=1 --key=bar --value=foo 8000 8001 8002

    call_kvstore --pem=1 --port=8002 put foobar barfoo
    sleep 4  # One consensus round.
    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002
    check_consistency --pem=1 --key=bar --value=foo 8000 8001 8002
    check_consistency --pem=1 --key=foobar --value=barfoo 8000 8001 8002

    # Bring it back.
    make -f $MAKEFILE start-single-node-dettached-3 || {
        echo '# Could not start nodes...' >&3
        exit 1
    }

    # Give time to the servers to start.
    wait_for_server 8003
    sleep 10
    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002 8003
    check_consistency --pem=1 --key=bar --value=foo 8000 8001 8002 8003
    check_consistency --pem=1 --key=foobar --value=barfoo 8000 8001 8002 8003
}
