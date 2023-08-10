GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MAKEFILE="Makefile.dweb"

load '../../test_helper/load'
load '../../test_helper/web'

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
    rm dummy.zip

    (
      cd "$GIT_ROOT/docker/" || exit 1
      make -f $MAKEFILE stop-nodes
    ) 2> /dev/null

    # Fix for BATS verbose run/test output gathering
    cd "$GIT_ROOT/tests/resiliency/web" || exit 1
}

@test "$SUITE: max tx bytes limits" {
    head -c 5242350 </dev/urandom | zip -0 > dummy.zip

    run wc -c dummy.zip
    assert_output --partial "5242546"

    # With the header/envelope overhead, the transaction should be exactly 5242880 bytes
    call_web --pem=1 --port=8000 deploy foobar dummy.zip
    assert_output --partial '{0: "https://foobar.'$(identity 1)'.web.liftedinit.tech"}' # TODO: Final format TBD

    # Create a new dummy file with one more byte
    head -c 5242351 </dev/urandom | zip -0 > dummy.zip

    run wc -c dummy.zip
    assert_output --partial "5242547"

    # With the header/envelope overhead, the transaction should be 5242881 bytes
    # This transaction is over limit and should fail
    call_web --pem=1 --port=8000 deploy foobar dummy.zip
    assert_output --partial "Content Too Large"
}
