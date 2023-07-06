# Resiliency test verifying the Token Migration
# New token metadata are stored in the DB after the migration
# Token endpoints should be activated after the migration

GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MIGRATION_ROOT="$GIT_ROOT/staging/ledger_migrations.json"
MAKEFILE="Makefile.ledger"
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    # jq doesn't support bigint
    jq '(.migrations[] | select(.name == "Token Migration")).block_height |= 30 |
        (.migrations[] | select(.name == "Token Migration")).disabled |= empty |
        (.migrations[] | select(.name == "Token Migration")) |= . + {
            "token_identity": "'$(identity 1)'",
            "token_next_subresource": 0,
            "symbol": "'${MFX_ADDRESS}'",
            "symbol_name": "Manifest Network Token",
            "symbol_decimals": 9,
            "symbol_total": 100000000000000,
            "symbol_circulating": 100000000000000,
            "symbol_maximum": null,
            "symbol_owner": "'${MFX_ADDRESS}'"
        }' \
        "$MIGRATION_ROOT" > "$BATS_TEST_ROOTDIR/migrations.json"

    (
      cd "$GIT_ROOT/docker/" || exit
      make -f $MAKEFILE clean
      make -f $MAKEFILE start-nodes-detached \
          ID_WITH_BALANCES="$(identity 1):1000000" \
          MIGRATIONS="$BATS_TEST_ROOTDIR/migrations.json" || {
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
    cd "$GIT_ROOT/tests/resiliency/ledger" || exit 1
}

@test "$SUITE: Token Migration" {
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003

    call_ledger --pem=1 --port=8000 token info ${MFX_ADDRESS}
    assert_output --partial "Invalid method name"

    wait_for_block 30

    for port in 8000 8001 8002 8003; do
        call_ledger --pem=1 --port=${port} token info ${MFX_ADDRESS}
        assert_output --partial "name: \"Manifest Network Token\""
        assert_output --partial "ticker: \"MFX\""
        assert_output --partial "decimals: 9"
        assert_output --regexp "owner:.*${MFX_ADDRESS}.*)"
        assert_output --regexp "total:.*(.*100000000000000,.*)"
        assert_output --regexp "circulating:.*(.*100000000000000,.*)"
        assert_output --regexp "maximum:.*None,.*"
    done
}

@test "$SUITE: Token endpoints are active after migration" {
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003

    call_ledger --pem=1 --port=8000 token info ${MFX_ADDRESS}
    assert_output --partial "Invalid method name"

    call_ledger --pem=1 --port=8000 token create "Foobar" "FBR" 9
    assert_output --partial "Invalid method name"

    call_ledger --pem=1 --port=8000 token update --name "Foobar2" maa
    assert_output --partial "Invalid method name"

    call_ledger --pem=1 --port=8000 token add-ext-info maa memo "\"My memo\""
    assert_output --partial "Invalid method name"

    call_ledger --pem=1 --port=8000 token remove-ext-info maa 0
    assert_output --partial "Invalid method name"

    call_ledger --pem=1 --port=8000 token mint MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\'''
    assert_output --partial "Invalid method name"

    call_ledger --pem=1 --port=8000 token burn MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\''' --error-on-under-burn
    assert_output --partial "Invalid method name"

    wait_for_block 30

    # Make sure MFX token info are present
    call_ledger --pem=1 --port=8000 token info ${MFX_ADDRESS}
    assert_output --partial "name: \"Manifest Network Token\""
    assert_output --partial "ticker: \"MFX\""
    assert_output --partial "decimals: 9"
    assert_output --regexp "total:.*(.*100000000000000,.*)"
    assert_output --regexp "circulating:.*(.*100000000000000,.*)"
    assert_output --regexp "maximum:.*None,.*"

    # Test token update
    # Change the token owner to `identity 2`
    create_token --pem=1 --port=8000
    call_ledger --pem=1 --port=8000 token update --name "\"New name\"" \
        --ticker "LLT" \
        --decimals "42" \
        --memo "\"Update memo\"" \
        --owner "$(identity 2)" \
        "${SYMBOL}"

    # And add a memo to the token
    call_ledger --pem=2 --port=8000 token add-ext-info "${SYMBOL}" memo "\"My memo\""

    for port in 8000 8001 8002 8003; do
        call_ledger --port=${port} token info "${SYMBOL}"
        assert_output --partial "name: \"New name\""
        assert_output --partial "ticker: \"LLT\""
        assert_output --partial "decimals: 42"
        assert_output --regexp "owner:.*$(identity 2).*)"
        assert_output --partial "My memo"
    done

    # Remove the memo from the token
    call_ledger --pem=2 --port=8000 token remove-ext-info "${SYMBOL}" 0
    for port in 8000 8001 8002 8003; do
        call_ledger --port=${port} token info "${SYMBOL}"
        refute_output --partial "Some memo"
    done

    # Mint tokens as the token identity
    call_ledger --pem=1 --port=8000 token mint ${SYMBOL} ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\'''
    for port in 8000 8001 8002 8003; do
        call_ledger --port=${port} token info "${SYMBOL}"
        assert_output --regexp "total:.*(.*579,.*)"
        assert_output --regexp "circulating:.*(.*579,.*)"
    done

    # Burn tokens as the token identity
    call_ledger --pem=1 --port=8000 token burn ${SYMBOL} ''\''{"'$(identity 2)'": 122, "'$(identity 3)'": 455}'\''' --error-on-under-burn
    for port in 8000 8001 8002 8003; do
        call_ledger --port=${port} token info "${SYMBOL}"
        assert_output --regexp "total:.*(.*2,.*)"
        assert_output --regexp "circulating:.*(.*2,.*)"
    done

    # Mint tokens as the token owner
    call_ledger --pem=2 --port=8000 token mint ${SYMBOL} ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\'''
    for port in 8000 8001 8002 8003; do
        call_ledger --port=${port} token info "${SYMBOL}"
        assert_output --regexp "total:.*(.*581,.*)"
        assert_output --regexp "circulating:.*(.*581,.*)"
    done

    # Burn tokens as the token owner
    call_ledger --pem=2 --port=8000 token burn ${SYMBOL} ''\''{"'$(identity 2)'": 122, "'$(identity 3)'": 455}'\''' --error-on-under-burn
    for port in 8000 8001 8002 8003; do
        call_ledger --port=${port} token info "${SYMBOL}"
        assert_output --regexp "total:.*(.*4,.*)"
        assert_output --regexp "circulating:.*(.*4,.*)"
    done
}

@test "$SUITE: Token creation migration is properly initialized when resetting the node" {
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003
    call_ledger --pem=1 --port=8000 token info ${MFX_ADDRESS}
    assert_output --partial "Invalid method name"

    wait_for_block 30
    for port in 8000 8001 8002 8003; do
        call_ledger --pem=1 --port=${port} token info ${MFX_ADDRESS}
        assert_output --partial "name: \"Manifest Network Token\""
    done

    cd "$GIT_ROOT/docker/" || exit 1
    make -f $MAKEFILE stop-single-node-0
    wait_for_block 40
    make -f $MAKEFILE start-single-node-detached-0
    wait_for_server 8000

    call_ledger --pem=1 --port=8000 token info ${MFX_ADDRESS}
    assert_output --partial "name: \"Manifest Network Token\""
}
