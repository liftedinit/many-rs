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
      make -f $MAKEFILE start-nodes-detached || {
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

@test "$SUITE: can put 254 bytes key with 512KiB values" {
    local key
    key=$(openssl rand -hex 254)
    dd if=/dev/urandom of=upload_test_512 bs=512000 count=1
    cat upload_test_512 | call_kvstore --pem=1 --port=8000 put --stdin --hex-key "$key"
    check_consistency_value_from_file --pem=1 --key="$key" --file="upload_test_512" 8000 8001 8002 8003
}
