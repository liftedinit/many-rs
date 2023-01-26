bats_require_minimum_version 1.5.0
GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MAKEFILE="Makefile.kvstore"

load '../../test_helper/load'
load '../../test_helper/kvstore'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    # Create PEM files beforehand, so we can generate the `allow addrs` config file
    pem 1
    pem 2

    (
      cd "$GIT_ROOT/docker/e2e/" || exit
      make -f $MAKEFILE clean
      # Generate the `allow addrs` config file using the PEM files from $PEM_ROOT
      make -f $MAKEFILE genfiles-kvstore/generate-allow-addrs-config PEM_ROOT=$PEM_ROOT
      # Start the nodes, enabling MANY address filtering using the `allow addrs` config file
      make -f $MAKEFILE $(ciopt start-nodes-dettached) ABCI_TAG=$(img_tag) KVSTORE_TAG=$(img_tag) ALLOW_ADDRS=true || {
            echo Could not start nodes... >&3
            exit 1
          }
    ) > /dev/null

    # Give time to the servers to start.
    sleep 30
    timeout 60s bash <<EOT
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

@test "$SUITE: ABCI filter MANY addresses" {
    call_kvstore --pem=1 --port=8000 put foo bar
    sleep 4  # One consensus round.
    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002 8003

    call_kvstore --pem=2 --port=8000 put bar foo
    sleep 4  # One consensus round.
    check_consistency --pem=2 --key=bar --value=foo 8000 8001 8002 8003

    # Commands are not allowed for non-listed addrs
    call_kvstore --pem=3 --port=8000 put one two
    assert_output --partial "The identity of the from field is invalid or unexpected."

    check_consistency --pem=1 --key=foo --value=bar 8000 8001 8002 8003
    check_consistency --pem=2 --key=bar --value=foo 8000 8001 8002 8003

    call_kvstore --pem=1 --port=8000 get one
    assert_output --partial "None"

    # But anyone can make a query
    call_kvstore --pem=3 --port=8000 get bar
    assert_output --partial "foo"

    # Even anonymous
    call_kvstore --port=8000 get bar
    assert_output --partial "foo"

    cd "$GIT_ROOT/docker/e2e/" || exit 1
}
