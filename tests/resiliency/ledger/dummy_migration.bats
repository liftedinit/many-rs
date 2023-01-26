GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MAKEFILE="Makefile.ledger"

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    echo '
    { "migrations": [
      {
        "name": "Dummy Hotfix",
        "block_height": 20
      },
      {
        "name": "Account Count Data Attribute",
        "block_height": 0,
        "issue": "https://github.com/liftedinit/many-framework/issues/190",
        "disabled": true
      },
      {
        "name": "Block 9400",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Memo Migration",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Token Migration",
        "block_height": 0,
        "disabled": true
      }
    ] }' > "$BATS_TEST_ROOTDIR/migration.json"
    (
      cd "$GIT_ROOT/docker/e2e/" || exit
      make -f $MAKEFILE clean
      make -f $MAKEFILE $(ciopt start-nodes-dettached) \
          ABCI_TAG=$(img_tag) \
          LEDGER_TAG=$(img_tag) \
          ID_WITH_BALANCES="$(identity 1):1000000" \
          MIGRATIONS="$BATS_TEST_ROOTDIR/migration.json" || {
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

@test "$SUITE: Dummy Hotfix" {
    local account_id
    local tx_id
    local current_height

    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003

    # Create a new account where 1 is the owner and 2 can approve transactions
    account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canMultisigApprove"] }, 2: [[1, { 0: 2 }]] }')

    # Transfer MFX to the account
    call_ledger --pem=1 --port=8000 send "$account_id" 1000000 MFX
    check_consistency --pem=1 --balance=1000000 --id="$account_id" 8000 8001 8002 8003

    # Submit a new multisig as 1
    call_ledger --pem=1 --port=8000 multisig submit --execute-automatically true "$account_id" send "$(identity 3)" 1000 MFX
    assert_output --partial "Transaction Token"
    tx_id=$(echo "$output" | grep "Transaction Token" | grep -oE "[0-9a-f]+$")

    # Get the current blockchain height
    current_height=$(many_message --pem=1 blockchain.info | head -n 4 | grep "1:" | cut -f 2 -d ':' | cut -f 2 -d ' ' | grep -oE "^[0-9]+")

    # Wait until we reach block 19
    while [ "$current_height" -lt 19 ]; do
      current_height=$(many_message --pem=1 blockchain.info | head -n 4 | grep "1:" | cut -f 2 -d ':' | cut -f 2 -d ' ' | grep -oE "^[0-9]+")
      echo "$current_height"
      sleep 0.5
    done >/dev/null

    # Approve and execute the transaction
    # At this point the Dummy Hotfix should execute!
    call_ledger --pem=2 --port=8000 multisig approve "$tx_id"

    # Retrieve the send event from the multisig transaction
    # Check that the timestamp has been fixed
    run many_message --pem=1 events.list "{2: {1: [[9, [1, 3]]]}}"
    assert_output --partial "5: 1(1234567890_2)"
}
