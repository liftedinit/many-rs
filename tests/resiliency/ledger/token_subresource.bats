# Resiliency test verifying the Token subresource exhaustion

GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MAKEFILE="Makefile.ledger"
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"
}

function teardown() {
    (
      cd "$GIT_ROOT/docker/e2e/" || exit 1
      make -f $MAKEFILE stop-nodes
    ) 2> /dev/null

    # Fix for BATS verbose run/test output gathering
    cd "$GIT_ROOT/tests/resiliency/ledger" || exit 1
}

@test "$SUITE: Token subresource exhaustion" {
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
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Token Migration",
        "block_height": 20,
        "token_identity": "'$(identity 1)'",
        "token_next_subresource": 2147483648,
        "symbol": "'$(subresource 1 2147483647)'",
        "symbol_name": "Manifest Network Token",
        "symbol_decimals": 9,
        "symbol_total": 100000000000000000,
        "symbol_circulating": 100000000000000000,
        "symbol_maximum": null,
        "symbol_owner": "'$(subresource 1 2147483647)'"
      }
    ] }' > "$BATS_TEST_ROOTDIR/migrations.json"

    cp "$GIT_ROOT/staging/ledger_state.json5" "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # The MFX address in the staging file is now `identity 1`
    sed -i.bak 's/'${MFX_ADDRESS}'/'$(subresource 1 2147483647)'/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Make `identity 1` the token identity
    sed -i.bak 's/token_identity: ".*"/token_identity: "'"$(identity 1)"'"/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Skip hash check
    sed -i.bak 's/hash/\/\/hash/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    (
      cd "$GIT_ROOT/docker/e2e/" || exit
      make -f $MAKEFILE clean
      make -f $MAKEFILE $(ciopt start-nodes-dettached) \
          ABCI_TAG=$(img_tag) \
          LEDGER_TAG=$(img_tag) \
          ID_WITH_BALANCES="$(identity 1):1000000:$(subresource 1 2147483647)" \
          STATE="$BATS_TEST_ROOTDIR/ledger_state.json5" \
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

    wait_for_block 20

    call_ledger --pem=1 --port=8000 token create ABC "abc" 9
    assert_output --partial "Subresources are exhausted"
}

@test "$SUITE: Token subresource exhaustion, existing subresource" {
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
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Token Migration",
        "block_height": 20,
        "token_identity": "'$(identity 1)'",
        "token_next_subresource": 2147483646,
        "symbol": "'$(subresource 1 2147483647)'",
        "symbol_name": "Manifest Network Token",
        "symbol_decimals": 9,
        "symbol_total": 100000000000000000,
        "symbol_circulating": 100000000000000000,
        "symbol_maximum": null,
        "symbol_owner": "'$(subresource 1 2147483647)'"
      }
    ] }' > "$BATS_TEST_ROOTDIR/migrations.json"

    cp "$GIT_ROOT/staging/ledger_state.json5" "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # The MFX address in the staging file is now `identity 1`
    sed -i.bak 's/'${MFX_ADDRESS}'/'$(subresource 1 2147483647)'/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Make `identity 1` the token identity
    sed -i.bak 's/token_identity: ".*"/token_identity: "'"$(identity 1)"'"/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Skip hash check
    sed -i.bak 's/hash/\/\/hash/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    (
      cd "$GIT_ROOT/docker/e2e/" || exit
      make -f $MAKEFILE clean
      make -f $MAKEFILE $(ciopt start-nodes-dettached) \
          ABCI_TAG=$(img_tag) \
          LEDGER_TAG=$(img_tag) \
          ID_WITH_BALANCES="$(identity 1):1000000:$(subresource 1 2147483647)" \
          STATE="$BATS_TEST_ROOTDIR/ledger_state.json5" \
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

    wait_for_block 20

    create_token --pem=1 --port=8000
    assert_output --partial "$(subresource 1 2147483646)"

    # MFX is subresource 2147483647

    call_ledger --pem=1 --port=8000 token create ABC "abc" 9
    assert_output --partial "Subresources are exhausted"
}
