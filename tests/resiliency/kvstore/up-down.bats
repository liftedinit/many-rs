GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MAKEFILE="Makefile.kvstore"

load '../../test_helper/load'
load '../../test_helper/kvstore'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    (
      cd "$GIT_ROOT/docker/e2e/" || exit
      make -f $MAKEFILE clean
      make -f $MAKEFILE $(ciopt start-nodes-dettached) ABCI_TAG=$(img_tag) KVSTORE_TAG=$(img_tag) || {
        echo Could not start nodes... >&3
        exit 1
      }
    ) > /dev/null

    # Give time to the servers to start.
    sleep 30
    timeout 30s bash <<EOT
    while ! many message --server http://localhost:8000 status; do
      sleep 1
    done >/dev/null
EOT
}

function teardown() {
    (
      cd "$GIT_ROOT/docker/e2e/" || exit 1
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
    cd "$GIT_ROOT/docker/e2e/" || exit 1

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
    make -f $MAKEFILE $(ciopt start-single-node-dettached)-3 ABCI_TAG=$(img_tag) KVSTORE_TAG=$(img_tag) || {
        echo Could not start nodes... >&3
        exit 1
    }

    # Give time to the servers to start.
    timeout 60s bash <<EOT
    while ! many message --server http://localhost:8003 status; do
      sleep 1
    done >/dev/null
EOT
    sleep 10
    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002 8003
    check_consistency --pem=1 --key=bar --value=foo 8000 8001 8002 8003
    check_consistency --pem=1 --key=foobar --value=barfoo 8000 8001 8002 8003
}
