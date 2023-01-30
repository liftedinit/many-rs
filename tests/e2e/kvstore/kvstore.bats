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

    start_kvstore --pem "$(pem 0)"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: can put and get data" {
  call_kvstore --pem=1 --port=8000 put "010203" "foobar"
  call_kvstore --pem=1 --port=8000 get "010203"
  assert_output --partial "foobar"
}

@test "$SUITE: can put and query data" {
  call_kvstore --pem=1 --port=8000 put "010203" "foobar"
  call_kvstore --pem=1 --port=8000 query "010203"
  assert_output --partial "$(identity 1)"
}

@test "$SUITE: can disable data, can query but not get" {
  call_kvstore --pem=1 --port=8000 put "010203" "foobar"
  call_kvstore --pem=1 --port=8000 disable "010203"
  call_kvstore --pem=1 --port=8000 get "010203"
  assert_output --partial "The key was disabled by its owner."
  call_kvstore --pem=1 --port=8000 query "010203"
  assert_output --partial "$(identity 1)"
}

@test "$SUITE: can put data on-behalf of but not disable" {
  account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canKvStorePut"] }, 2: [2] }')
  call_kvstore --pem=2 --port=8000 --alt-owner "$account_id" put "040506" "foobar"
  call_kvstore --pem=1 --port=8000 get "040506"
  assert_output --partial "foobar"
  call_kvstore --pem=2 --port=8000 --alt-owner "$account_id" disable "040506"
  assert_output --partial "canKvStoreDisable"
}

@test "$SUITE: can disable data on-behalf of but not put" {
  account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canKvStoreDisable"] }, 2: [2] }')
  call_kvstore --pem=2 --port=8000 --alt-owner "$account_id" put "060708" "foobar"
  assert_output --partial "canKvStorePut"
  call_kvstore --pem=1 --port=8000 --alt-owner "$account_id" put "060708" "foobar"
  call_kvstore --pem=2 --port=8000 --alt-owner "$account_id" disable "060708"
  call_kvstore --pem=1 --port=8000 get "060708"
  assert_output --partial "The key was disabled by its owner."
  call_kvstore --pem=1 --port=8000 query "060708"
  assert_output --partial "$account_id"
}

@test "$SUITE: unable to disable an empty key" {
  call_kvstore --pem=1 --port=8000 disable "010203"
  assert_output --partial "Unable to disable an empty key."
}

@test "$SUITE: can disable with reason, can query but not get" {
  call_kvstore --pem=1 --port=8000 put "112233" "foobar"
  call_kvstore --pem=1 --port=8000 disable --reason "sad" "112233"
  call_kvstore --pem=1 --port=8000 get "112233"
  assert_output --partial "The key was disabled by its owner."
  call_kvstore --pem=1 --port=8000 query "112233"
  assert_output --partial "sad"
}

@test "$SUITE: can transfer" {
  call_kvstore --pem=1 --port=8000 put "112233" "foobar"
  call_kvstore --pem=1 --port=8000 query "112233"
  assert_output --partial "$(identity 1)"

  call_kvstore --pem=1 --port=8000 transfer "112233" "$(identity 2)"
  assert_output --partial "null"
  call_kvstore --pem=1 --port=8000 query "112233"
  assert_output --partial "$(identity 2)"

  call_kvstore --pem=1 --port=8000 get "112233"
  assert_output --partial "foobar"

  # PEM 1 should fail.
  call_kvstore --pem=1 --port=8000 put "112233" "hello"
  assert_output --partial "You do not have the authorization to modify this key."
  call_kvstore --pem=1 --port=8000 get "112233"
  assert_output --partial "foobar"

  # PEM 2 should work.
  call_kvstore --pem=2 --port=8000 put "112233" "bateau"
  call_kvstore --pem=1 --port=8000 get "112233"
  assert_output --partial "bateau"
}
