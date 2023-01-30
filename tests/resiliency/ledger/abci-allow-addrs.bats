GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MAKEFILE="Makefile.ledger"

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    # Create PEM files beforehand, so we can generate the `allow addrs` config file
    pem 1
    pem 2

    (
      cd "$GIT_ROOT/docker/e2e/" || exit
      make -f $MAKEFILE clean
      # Generate the `allow addrs` config file using the PEM files from $PEM_ROOT
      make -f $MAKEFILE genfiles-ledger/generate-allow-addrs-config PEM_ROOT=$PEM_ROOT
      # Start the nodes, enabling MANY address filtering using the `allow addrs` config file
      make -f $MAKEFILE $(ciopt start-nodes-dettached) ABCI_TAG=$(img_tag) LEDGER_TAG=$(img_tag) ALLOW_ADDRS=true ID_WITH_BALANCES="$(identity 1):1000000" || {
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
    cd "$GIT_ROOT/tests/resiliency/ledger" || exit 1
}

@test "$SUITE: ABCI filter MANY addresses" {
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX
    call_ledger --pem=1 --port=8000 send "$(identity 3)" 1000 MFX
    sleep 4  # One consensus round.
    check_consistency --pem=1 --balance=998000 --id="$(identity 1)" 8000 8001 8002 8003
    check_consistency --pem=2 --balance=1000 --id="$(identity 2)" 8000 8001 8002 8003
    check_consistency --pem=3 --balance=1000 --id="$(identity 3)" 8000 8001 8002 8003

    call_ledger --pem=2 --port=8000 send "$(identity 1)" 1000 MFX
    sleep 4  # One consensus round.
    check_consistency --pem=1 --balance=999000 --id="$(identity 1)" 8000 8001 8002 8003
    check_consistency --pem=2 --balance=0 --id="$(identity 2)" 8000 8001 8002 8003
    check_consistency --pem=3 --balance=1000 --id="$(identity 3)" 8000 8001 8002 8003

    # Commands are not allowed for non-listed addrs
    call_ledger --pem=3 --port=8000 send "$(identity 2)" 1000 MFX
    assert_output --partial "The identity of the from field is invalid or unexpected."

    check_consistency --pem=1 --balance=999000 --id="$(identity 1)" 8000 8001 8002 8003
    check_consistency --pem=2 --balance=0 --id="$(identity 2)" 8000 8001 8002 8003

    # But anyone can make a query
    check_consistency --pem=3 --balance=1000 --id="$(identity 3)" 8000 8001 8002 8003

    # Even anonymous
    check_consistency --balance=1000 --id="$(identity 3)" 8000 8001 8002 8003

    cd "$GIT_ROOT/docker/e2e/" || exit 1
}
