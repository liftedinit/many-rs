GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MAKEFILE="Makefile.ledger"

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    (
      cd "$GIT_ROOT/docker/e2e/" || exit
      make -f $MAKEFILE clean
      make -f $MAKEFILE $(ciopt start-nodes-dettached) \
          ABCI_TAG=$(img_tag) \
          LEDGER_TAG=$(img_tag) \
          ID_WITH_BALANCES="$(identity 1):1000000" \
          FEATURES="--//src/many-ledger:balance_testing=1" || {
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
    cd "$GIT_ROOT/tests/resiliency/ledger" || exit 1
}

# Tests https://github.com/liftedinit/many-framework/issues/290
@test "$SUITE: Application hash is consistent with 1 node down" {
    cd "$GIT_ROOT/docker/e2e/" || exit 1

    # Create transactions before bringing the node down
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003

    call_ledger --pem=1 --port=8000 send "$(identity 2)" 3000 MFX
    check_consistency --pem=1 --balance=997000 --id="$(identity 1)" 8000 8001 8002 8003
    check_consistency --pem=2 --balance=3000 --id="$(identity 2)" 8000 8001 8002 8003

    call_ledger --pem=2 --port=8001 send "$(identity 1)" 1000 MFX
    check_consistency --pem=1 --balance=998000 --id="$(identity 1)" 8000 8001 8002 8003
    check_consistency --pem=2 --balance=2000 --id="$(identity 2)" 8000 8001 8002 8003

    # Bring down node 3.
    make -f $MAKEFILE stop-single-node-3

    sleep 10

    # Bring it back
    make -f $MAKEFILE $(ciopt start-single-node-dettached)-3 \
      ABCI_TAG=$(img_tag) \
      LEDGER_TAG=$(img_tag) \
      FEATURES="--//src/many-ledger:balance_testing=1" || {
        echo Could not start nodes... >&3
        exit 1
    }

    # At this point, node 3 should catch up and the global application hash should be valid

    # Give time to the servers to start.
    timeout 60s bash <<EOT
    while ! many message --server http://localhost:8003 status; do
      sleep 1
    done >/dev/null
EOT

    sleep 10
}

# Tests https://github.com/liftedinit/many-framework/issues/289
@test "$SUITE: First block after load has a transaction" {
    cd "$GIT_ROOT/docker/e2e" || exit 1

    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX

    # Bring down node 3.
    make -f $MAKEFILE stop-single-node-3

    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX
    call_ledger --pem=1 --port=8000 send "$(identity 2)" 1000 MFX

    # Bring it back.
    make -f $MAKEFILE $(ciopt start-single-node-dettached)-3 \
      ABCI_TAG=$(img_tag) \
      LEDGER_TAG=$(img_tag) \
      FEATURES="--//src/many-ledger:balance_testing=1" || {
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

    check_consistency --pem=1 --balance=994000 --id="$(identity 1)" 8000 8001 8002 8003
    check_consistency --pem=2 --balance=6000 --id="$(identity 2)" 8000 8001 8002 8003
}
