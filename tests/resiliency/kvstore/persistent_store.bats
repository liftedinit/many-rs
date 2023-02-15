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
    timeout 60s bash -c probe_server 8000 8001 8002 8003
}

function teardown() {
    (
      cd "$GIT_ROOT/docker/" || exit 1
      make -f $MAKEFILE stop-nodes
    ) 2> /dev/null

    # Fix for BATS verbose run/test output gathering
    cd "$GIT_ROOT/tests/resiliency/kvstore" || exit 1
}

# Relates https://github.com/liftedinit/many-framework/issues/290
@test "$SUITE: Application hash is consistent with 1 node down" {
    cd "$GIT_ROOT/docker/" || exit 1

    # Create transactions before bringing the node down
    call_kvstore --pem=1 --port=8000 put foo bar
    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002 8003

    call_kvstore --pem=1 --port=8001 put bar foo
    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002 8003
    check_consistency --pem=1 --key=bar --value=foo 8000 8001 8002 8003

    # Bring down node 3.
    make -f $MAKEFILE stop-single-node-3

    sleep 10

    # Bring it back
    make -f $MAKEFILE start-single-node-dettached-3 || {
        echo '# Could not start nodes...' >&3
        exit 1
    }

    # At this point, node 3 should catch up and the global application hash should be valid

    # Give time to the servers to start.
    timeout 60s bash -c probe_server 8003
    sleep 10
}

# Relates https://github.com/liftedinit/many-framework/issues/289
@test "$SUITE: First block after load has a transaction" {
    cd "$GIT_ROOT/docker/" || exit 1

    call_kvstore --pem=1 --port=8000 put foo1 bar1
    call_kvstore --pem=1 --port=8000 put foo2 bar2
    call_kvstore --pem=1 --port=8000 put foo3 bar3

    # Bring down node 3.
    make -f $MAKEFILE stop-single-node-3

    call_kvstore --pem=1 --port=8000 put foo4 bar4
    call_kvstore --pem=1 --port=8000 put foo5 bar5
    call_kvstore --pem=1 --port=8000 put foo6 bar6

    # Bring it back.
    make -f $MAKEFILE start-single-node-dettached-3 || {
        echo '# Could not start nodes...' >&3
        exit 1
    }

    # Give time to the servers to start.
    timeout 60s bash -c probe_server 8003
    sleep 10

    check_consistency --pem=1 --key=foo1 --value=bar1 8000 8001 8002 8003
    check_consistency --pem=1 --key=foo2 --value=bar2 8000 8001 8002 8003
    check_consistency --pem=1 --key=foo3 --value=bar3 8000 8001 8002 8003
    check_consistency --pem=1 --key=foo4 --value=bar4 8000 8001 8002 8003
    check_consistency --pem=1 --key=foo5 --value=bar5 8000 8001 8002 8003
    check_consistency --pem=1 --key=foo6 --value=bar6 8000 8001 8002 8003
}
