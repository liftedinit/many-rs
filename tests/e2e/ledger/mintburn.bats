# e2e tests for the mint/burn feature set
# The Token Migration needs to be active for this feature set to be enabled.

GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    (
      cd "$GIT_ROOT"
      cargo build --features migration_testing --features balance_testing
    )

    echo '
    { "migrations": [
      {
        "name": "Account Count Data Attribute",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Block 9400",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Dummy Hotfix",
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
        "block_height": 0
      }
    ] }' > "$BATS_TEST_ROOTDIR/migrations.json"


    # Dummy image
     echo -n -e '\x68\x65\x6c\x6c\x6f' > "$BATS_TEST_ROOTDIR/image.png"

    # Activating the Token Migration from block 0 will modify the ledger staging hash
    # The symbol metadata will be stored in the DB
    cp "$GIT_ROOT/staging/ledger_state.json5" "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Make `identity 1` the token identity
    sed -i 's/token_identity: ".*"/token_identity: "'"$(identity 1)"'"/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Use token identity subresource 0 as the first token symbol
    sed -i 's/token_next_subresource: 2/token_next_subresource: 0/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Skip hash check
    sed -i 's/hash/\/\/hash/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    start_ledger --state="$BATS_TEST_ROOTDIR/ledger_state.json5" \
        --pem "$(pem 0)" \
        --balance-only-for-testing="$(identity 1):1000:$MFX_ADDRESS" \
        --migrations-config "$BATS_TEST_ROOTDIR/migrations.json"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: can mint token" {
    call_ledger --pem=1 --port=8000 token mint MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\'''
    check_consistency --pem=2 --balance=123 8000
    check_consistency --pem=3 --balance=456 8000

    call_ledger --port=8000 token info ${MFX_ADDRESS}
    assert_output --regexp "total:.*(.*2000000579,.*)"
    assert_output --regexp "circulating:.*(.*2000000579,.*)"
}

@test "$SUITE: can burn token" {
    call_ledger --pem=1 --port=8000 send $(identity 2) 123 MFX
    call_ledger --pem=1 --port=8000 send $(identity 3) 456 MFX
    call_ledger --pem=1 --port=8000 token burn MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\''' --error-on-under-burn
    check_consistency --pem=2 --balance=0 8000
    check_consistency --pem=3 --balance=0 8000

    call_ledger --port=8000 token info ${MFX_ADDRESS}
    assert_output --regexp "total:.*(.*1999999421,.*)"
    assert_output --regexp "circulating:.*(.*1999999421,.*)"
}

@test "$SUITE: only token identity can mint" {
    call_ledger --pem=2 --port=8000 token mint MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\'''
    assert_output --partial "Unauthorised Token endpoints sender."

    call_ledger --pem=8 --port=8000 token mint MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\'''
    assert_output --partial "Unauthorised Token endpoints sender."

    call_ledger --port=8000 token mint MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\'''
    assert_output --partial "Unauthorised Token endpoints sender."
}

@test "$SUITE: only token identity can burn" {
    call_ledger --pem=1 --port=8000 send $(identity 2) 123 MFX
    call_ledger --pem=1 --port=8000 send $(identity 3) 456 MFX
    call_ledger --pem=2 --port=8000 token burn MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\''' --error-on-under-burn
    assert_output --partial "Unauthorised Token endpoints sender."

    call_ledger --pem=8 --port=8000 token burn MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\''' --error-on-under-burn
    assert_output --partial "Unauthorised Token endpoints sender."

    call_ledger --port=8000 token burn MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\''' --error-on-under-burn
    assert_output --partial "Unauthorised Token endpoints sender."
}

@test "$SUITE: partial burns are disabled" {
    call_ledger --pem=1 --port=8000 send $(identity 2) 123 MFX
    call_ledger --pem=1 --port=8000 send $(identity 3) 456 MFX
    call_ledger --pem=1 --port=8000 token burn MFX ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\'''
    assert_output --partial "Partial burns are disabled."
}

@test "$SUITE: can't mint over maximum" {
    create_token --pem=1 --port=8000 --maximum-supply 100
    call_ledger --port=8000 token info ${SYMBOL}
    assert_output --regexp "maximum:.*(.*100,.*)"

    call_ledger --pem=1 --port=8000 token mint ${SYMBOL} ''\''{"'$(identity 2)'": 123, "'$(identity 3)'": 456}'\'''
    assert_output --partial "Unable to mint over the maximum symbol supply"
}

@test "$SUITE: can't under burn" {
    call_ledger --pem=1 --port=8000 send $(identity 2) 123 MFX
    call_ledger --pem=1 --port=8000 send $(identity 3) 456 MFX
    call_ledger --pem=1 --port=8000 token burn MFX ''\''{"'$(identity 2)'": 124, "'$(identity 3)'": 457}'\''' --error-on-under-burn
    assert_output --partial "Unable to burn, missing funds"
}

@test "$SUITE: can't mint zero" {
    call_ledger --pem=1 --port=8000 token mint MFX ''\''{"'$(identity 2)'": 0, "'$(identity 3)'": 0}'\'''
    assert_output --partial "The mint/burn distribution contains zero"
}

@test "$SUITE: can't burn zero" {
    call_ledger --pem=1 --port=8000 token burn MFX ''\''{"'$(identity 2)'": 0, "'$(identity 3)'": 0}'\''' --error-on-under-burn
    assert_output --partial "The mint/burn distribution contains zero"
}
