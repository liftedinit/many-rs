GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MAKEFILE="Makefile.ledger"

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    echo '
    { "migrations": [
      {
        "name": "Account Count Data Attribute",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Dummy Hotfix",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Block 9400",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Memo Migration",
        "block_height": 30
      },
      {
        "name": "Token Migration",
        "block_height": 0,
        "disabled": true
      }
    ] }' > "$BATS_TEST_ROOTDIR/migrations.json"

    (
      cd "$GIT_ROOT/docker/e2e/" || exit
      make -f $MAKEFILE clean
      make -f $MAKEFILE $(ciopt start-nodes-dettached) \
          ABCI_TAG=$(img_tag) \
          LEDGER_TAG=$(img_tag) \
          ID_WITH_BALANCES="$(identity 1):1000000" \
          MIGRATIONS="$BATS_TEST_ROOTDIR/migrations.json" || {
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

@test "$SUITE: Memo Migration" {
    local account_id
    local tx_id_1
    local tx_id_2

    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003

    account_id="$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canMultisigApprove"] }, 2: [[1, { 0: 2 }]] }')"
    call_ledger --pem=1 --port=8000 send "$account_id" 1000000 MFX

    call_ledger --pem=1 --port=8000 multisig \
        submit --legacy-memo="Legacy_Memo" --memo="New_Memo" "$account_id" \
        send "$(identity 2)" 1000 MFX
    tx_id_1=$(echo $output | grep "Transaction Token:" | grep -o "[0-9a-f]*$")

    run many_message --pem=1 events.list "{}"
    assert_output --regexp "3:.*Legacy_Memo"
    refute_output --partial "New_Memo"

    call_ledger --port=8000 multisig info $tx_id_1
    assert_output --partial "memo_: Some"
    assert_output --partial "data_: None"
    assert_output --partial "memo: None"

    wait_for_block 30

    run many_message --pem=1 events.list "{}"
    refute_output --partial "3: \"Legacy_Memo\""
    assert_output --partial "10: [\"Legacy_Memo\"]"
    refute_output --partial "New_Memo"

    call_ledger --pem=1 --port=8000 multisig \
        submit --legacy-memo="Legacy_Memo2" --memo="New_Memo2" "$account_id" \
        send "$(identity 2)" 1000 MFX
    tx_id_2=$(echo $output | grep "Transaction Token:" | grep -o "[0-9a-f]*$")

    run many_message --pem=1 events.list "{}"
    assert_output --regexp "10:.*New_Memo2"
    refute_output --partial "Legacy_Memo2"

    call_ledger --port=8000 multisig info $tx_id_1
    assert_output --partial "memo_: None"
    assert_output --partial "data_: None"
    assert_output --partial "memo: Some"

    call_ledger --port=8000 multisig info $tx_id_2
    assert_output --partial "memo_: None"
    assert_output --partial "data_: None"
    assert_output --partial "memo: Some"
}
