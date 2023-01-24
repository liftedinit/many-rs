# Test suite verifying Token feature set doesn't work unless Token Migration is active

GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    (
      cd "$GIT_ROOT"
      cargo build
    )

    start_ledger --pem "$(pem 0)"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: tokens.create is disabled" {
    call_ledger --pem=1 --port=8000 token create "Foobar" "FBR" 9
    assert_output --partial "Invalid method name"
}

@test "$SUITE: tokens.update is disabled" {
    call_ledger --pem=1 --port=8000 token update --name "Foobar2" maa
    assert_output --partial "Invalid method name"
}

@test "$SUITE: tokens.info is disabled" {
    call_ledger --pem=1 --port=8000 token info maa
    assert_output --partial "Invalid method name"
}

@test "$SUITE: tokens.addExtendedInfo is disabled" {
    call_ledger --pem=1 --port=8000 token add-ext-info maa memo "\"My memo\""
    assert_output --partial "Invalid method name"
}

@test "$SUITE: tokens.removeExtendedInfo is disabled" {
    call_ledger --pem=1 --port=8000 token remove-ext-info maa 0
    assert_output --partial "Invalid method name"
}
