GIT_ROOT="$BATS_TEST_DIRNAME/../../../"

load '../../test_helper/load'
load '../../test_helper/ledger'

# Pass in the recall phrase, the identity, the cred id and the key cose.
function assert_idstore() {
    local recall="$1"
    local identity="$2"
    local cred_id="$3"
    local key2cose="$4"

    run many_message --pem=0 idstore.getFromRecallPhrase "$recall"
    assert_output --partial "0: h'$(echo "$cred_id" | tr A-Z a-z)'"
    assert_output --partial "1: h'${key2cose}'"

    run many_message --pem=0 idstore.getFromAddress '{0: "'"$identity"'"}'
    assert_output --partial "0: h'$(echo "$cred_id" | tr A-Z a-z)'"
    assert_output --partial "1: h'${key2cose}'"
}

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    (
      cd "$GIT_ROOT"
      cargo build --features webauthn_testing
    )
}

function teardown() {
    stop_background_run
}

@test "$SUITE: IdStore store and get" {
    start_ledger --pem "$(pem 0)" \
                 --disable-webauthn-only-for-testing # Disable WebAuthn check for this test

    identity_hex=$(identity_hex 1)
    cred_id=$(cred_id)
    key2cose=$(key2cose 1)

    run many_message --pem=0 idstore.store "{0: 10000_1(h'${identity_hex}'), 1: h'${cred_id}', 2: h'${key2cose}'}"
    assert_output '{0: ["abandon", "again"]}'

    assert_idstore "$output" "$(identity 1)" "$cred_id" "$key2cose"
}

@test "$SUITE: IdStore store deny non-webauthn" {
    start_ledger --pem "$(pem 0)"

    run many_message --pem=0 idstore.store "{0: 10000_1(h'$(identity_hex 1)'), 1: h'$(cred_id)', 2: h'$(key2cose 1)'}"
    assert_output --partial "Non-WebAuthn request denied for endpoint"
}

@test "$SUITE: IdStore export works" {
    which jq || skip "'jq' needs to be installed for this test."
    local ledger_db
    local state
    ledger_db="$(mktemp -d)"
    state="$GIT_ROOT/tests/e2e/ledger/webauthn_state.json"

    start_ledger \
        "--persistent=$ledger_db" \
        "--state=$state" \
        --pem "$(pem 0)" \
        --disable-webauthn-only-for-testing # Disable WebAuthn check for this test

    local identity_hex_1
    local cred_id_1
    local key2cose_1
    identity_hex_1=$(identity_hex 1)
    cred_id_1=$(cred_id)
    key2cose_1=$(key2cose 1)

    run many_message --pem=0 idstore.store "{0: 10000_1(h'${identity_hex_1}'), 1: h'${cred_id_1}', 2: h'${key2cose_1}'}"
    assert_output '{0: ["abandon", "again"]}'
    local recall_1
    recall_1="$output"

    # Stop and regenesis.
    stop_background_run

    # Export to a temp file.
    local export_file
    export_file="$(mktemp)"
    "$GIT_ROOT/target/debug/idstore-export" "$ledger_db" > "$export_file"
    local import_file
    import_file="$(mktemp)"
    jq -s '.[0] * .[1]' "$state" "$export_file" > "$import_file"

    cat $import_file >&2

    start_ledger \
        --persistent="$ledger_db" \
        --state="$import_file" \
        --pem "$(pem 0)" \
        --disable-webauthn-only-for-testing # Disable WebAuthn check for this test

    local identity_hex_2
    local cred_id_2
    local key2cose_2
    identity_hex_2=$(identity_hex 2)
    cred_id_2=$(cred_id)
    key2cose_2=$(key2cose 2)

    # Continue the test.
    run many_message --pem=0 idstore.store "{0: 10000_1(h'${identity_hex_2}'), 1: h'${cred_id_2}', 2: h'${key2cose_2}'}"
    assert_output '{0: ["abandon", "asset"]}'
    local recall_2
    recall_2="$output"

    assert_idstore "$recall_1" "$(identity 1)" "$cred_id_1" "$key2cose_1"
    assert_idstore "$recall_2" "$(identity 2)" "$cred_id_2" "$key2cose_2"
}
