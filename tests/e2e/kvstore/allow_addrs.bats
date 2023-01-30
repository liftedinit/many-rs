GIT_ROOT="$BATS_TEST_DIRNAME/../../../"

load '../../test_helper/load'
load '../../test_helper/kvstore'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    if ! [ $CI ]; then
        (
          cd "$GIT_ROOT"
          cargo build
        )
    fi

    local ALLOW_ADDRS_CONFIG=$(generate_allow_addrs_config 1 2)

    start_kvstore --pem "$(pem 0)" \
          --allow-addrs "$ALLOW_ADDRS_CONFIG"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: allow addrs" {
    call_kvstore --pem=1 --port=8000 put "01" "one"
    call_kvstore --pem=1 --port=8000 get "01"
    assert_output --partial "one"

    call_kvstore --pem=2 --port=8000 put "02" "two"
    call_kvstore --pem=2 --port=8000 get "02"
    assert_output --partial "two"

    call_kvstore --pem=3 --port=8000 put "03" "three"
    assert_output --partial "The identity of the from field is invalid or unexpected."

    call_kvstore --pem=3 --port=8000 get "02"
    assert_output --partial "two"

    call_kvstore --pem=3 --port=8000 get "03"
    assert_output --partial "None"
}
