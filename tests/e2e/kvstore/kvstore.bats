GIT_ROOT="$BATS_TEST_DIRNAME/../../../"

load '../../test_helper/load'
load '../../test_helper/kvstore'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

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

@test "$SUITE: can put 254 bytes key" {
  local key
  key=$(openssl rand -hex 254)
  call_kvstore --pem=1 --port=8000 put --hex-key "$key" "foobar"
  call_kvstore --pem=1 --port=8000 get --hex-key "$key"
  assert_output --partial "foobar"
}

@test "$SUITE: can't put 255 bytes key" {
  local key
  key=$(openssl rand -hex 255)
  call_kvstore --pem=1 --port=8000 put --hex-key "$key" "foobar"
  call_kvstore --pem=1 --port=8000 get --hex-key "$key"
  refute_output --partial "foobar"
}

@test "$SUITE: can put 512KiB values" {
  dd if=/dev/urandom of=upload_test_512 bs=524288 count=1
  cat upload_test_512 | call_kvstore --pem=1 --port=8000 put --stdin "foo"
  call_kvstore --pem=1 --port=8000 get "foo"
  assert_output --partial "$(cat upload_test_512)"
}

@test "$SUITE: can't put 512KiB + 1 values" {
  dd if=/dev/urandom of=upload_test_512_1 bs=524289 count=1
  cat upload_test_512_1 | call_kvstore --pem=1 --port=8000 put --stdin "foo"
  call_kvstore --pem=1 --port=8000 get "foo"
  refute_output --partial "foo"
}

@test "$SUITE: can put 254 bytes key with 512KiB values" {
  local key
  key=$(openssl rand -hex 254)
  dd if=/dev/urandom of=upload_test_512 bs=524288 count=1
  cat upload_test_512 | call_kvstore --pem=1 --port=8000 put --stdin --hex-key "$key"
  call_kvstore --pem=1 --port=8000 get --hex-key "$key"
  assert_output --partial "$(cat upload_test_512)"
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

@test "$SUITE: can list keys" {
  call_kvstore --pem=1 --port=8000 put "112233" "foobar"
  call_kvstore --pem=1 --port=8000 put "445566" "foobar2"
  call_kvstore --pem=1 --port=8000 put "778899" "foobar3"
  call_kvstore --pem=2 --port=8000 put "aabbcc" "foobar4"
  call_kvstore --pem=2 --port=8000 put "ddeeff" "foobar5"
  call_kvstore --pem=2 --port=8000 put "gghhii" "foobar6"

  call_kvstore --pem=1 --port=8000 list --filter owner:$(identity 1)
  assert_output --partial "112233"
  assert_output --partial "445566"
  assert_output --partial "778899"
  refute_output --partial "aabbcc"
  refute_output --partial "ddeeff"
  refute_output --partial "gghhii"

  call_kvstore --pem=1 --port=8000 list --hex-key --filter owner:$(identity 1)
  assert_output --partial "313132323333"
  assert_output --partial "343435353636"
  assert_output --partial "373738383939"
  refute_output --partial "616162626363"
  refute_output --partial "646465656666"
  refute_output --partial "676768686969"

  call_kvstore --pem=2 --port=8000 list --filter owner:$(identity 2)
  assert_output --partial "aabbcc"
  assert_output --partial "ddeeff"
  assert_output --partial "gghhii"
  refute_output --partial "112233"
  refute_output --partial "445566"
  refute_output --partial "778899"

  call_kvstore --pem=2 --port=8000 list --hex-key --filter owner:$(identity 2)
  assert_output --partial "616162626363"
  assert_output --partial "646465656666"
  assert_output --partial "676768686969"
  refute_output --partial "313132323333"
  refute_output --partial "343435353636"
  refute_output --partial "373738383939"
}

@test "$SUITE: can't list disabled key" {
  call_kvstore --pem=1 --port=8000 put "112233" "foobar"
  call_kvstore --pem=1 --port=8000 put "445566" "foobar2"
  call_kvstore --pem=1 --port=8000 put "778899" "foobar3"

  call_kvstore --pem=1 --port=8000 list
  assert_output --partial "112233"
  assert_output --partial "445566"
  assert_output --partial "778899"

  call_kvstore --pem=1 --port=8000 disable "112233"
  call_kvstore --pem=1 --port=8000 list --filter disabled:false
  refute_output --partial "112233"
  assert_output --partial "445566"
  assert_output --partial "778899"

  call_kvstore --pem=1 --port=8000 disable "445566" --reason "Foo"
  call_kvstore --pem=1 --port=8000 list --filter disabled:false
  refute_output --partial "112233"
  refute_output --partial "445566"
  assert_output --partial "778899"

  call_kvstore --pem=1 --port=8000 put "112233" "foobar"
  call_kvstore --pem=1 --port=8000 list --filter disabled:false
  assert_output --partial "112233"
  refute_output --partial "445566"
  assert_output --partial "778899"
}

@test "$SUITE: can list disabled key" {
  call_kvstore --pem=1 --port=8000 put "112233" "foobar"
  call_kvstore --pem=1 --port=8000 put "445566" "foobar2"
  call_kvstore --pem=1 --port=8000 put "778899" "foobar3"

  call_kvstore --pem=1 --port=8000 disable "112233"
  call_kvstore --pem=1 --port=8000 disable "445566" --reason "Foo"
  call_kvstore --pem=1 --port=8000 list --filter disabled:true
  assert_output --partial "112233"
  assert_output --partial "445566"
  refute_output --partial "778899"
}

@test "$SUITE: can list disabled key of a given user" {
  call_kvstore --pem=1 --port=8000 put "112233" "foobar"
  call_kvstore --pem=1 --port=8000 put "445566" "foobar2"
  call_kvstore --pem=1 --port=8000 put "778899" "foobar3"
  call_kvstore --pem=2 --port=8000 put "aabbcc" "foobar4"
  call_kvstore --pem=2 --port=8000 put "ddeeff" "foobar5"
  call_kvstore --pem=2 --port=8000 put "gghhii" "foobar6"

  call_kvstore --pem=1 --port=8000 disable "112233"
  call_kvstore --pem=1 --port=8000 disable "445566" --reason "Foo"
  call_kvstore --pem=2 --port=8000 disable "aabbcc"
  call_kvstore --pem=1 --port=8000 list --filter disabled:true --filter owner:$(identity 1)
  assert_output --partial "112233"
  assert_output --partial "445566"
  refute_output --partial "778899"
  refute_output --partial "aabbcc"

  call_kvstore --pem=1 --port=8000 list --filter disabled:true --filter owner:$(identity 2)
  refute_output --partial "112233"
  refute_output --partial "445566"
  refute_output --partial "778899"
  assert_output --partial "aabbcc"

  call_kvstore --pem=1 --port=8000 list --filter disabled:true
  assert_output --partial "112233"
  assert_output --partial "445566"
  refute_output --partial "778899"
  assert_output --partial "aabbcc"
}

@test "$SUITE: list immutable keys from given user" {
  call_kvstore --pem=1 --port=8000 put "112233" "foobar"
  call_kvstore --pem=1 --port=8000 put "445566" "foobar2"
  call_kvstore --pem=1 --port=8000 put "778899" "foobar3"
  call_kvstore --pem=2 --port=8000 put "aabbcc" "foobar4"
  call_kvstore --pem=2 --port=8000 put "ddeeff" "foobar5"
  call_kvstore --pem=2 --port=8000 put "gghhii" "foobar6"

  call_kvstore --pem=1 --port=8000 transfer "112233" maiyg
  call_kvstore --pem=2 --port=8000 transfer "aabbcc" maiyg

  call_kvstore --pem=1 --port=8000 list --filter previous_owner:$(identity 1) --filter owner:maiyg
  assert_output --partial "112233"
  refute_output --partial "445566"
  refute_output --partial "778899"
  refute_output --partial "aabbcc"
  refute_output --partial "ddeeff"
  refute_output --partial "gghhii"

  call_kvstore --pem=1 --port=8000 list --filter previous_owner:$(identity 2) --filter owner:maiyg
  refute_output --partial "112233"
  refute_output --partial "445566"
  refute_output --partial "778899"
  assert_output --partial "aabbcc"
  refute_output --partial "ddeeff"
  refute_output --partial "gghhii"

  call_kvstore --pem=1 --port=8000 list --filter owner:maiyg
  assert_output --partial "112233"
  refute_output --partial "445566"
  refute_output --partial "778899"
  assert_output --partial "aabbcc"
  refute_output --partial "ddeeff"
  refute_output --partial "gghhii"
}

@test "$SUITE: list key ordering" {
  call_kvstore --pem=1 --port=8000 put "112233" "foobar"
  call_kvstore --pem=1 --port=8000 put "445566" "foobar2"
  call_kvstore --pem=1 --port=8000 put "778899" "foobar3"

  call_kvstore --pem=1 --port=8000 list --order ascending
  assert_output "112233
445566
778899"

  call_kvstore --pem=1 --port=8000 list --order descending
  assert_output "778899
445566
112233"

  call_kvstore --pem=1 --port=8000 list --hex-key --order ascending
  assert_output "313132323333
343435353636
373738383939"

  call_kvstore --pem=1 --port=8000 list --hex-key --order descending
  assert_output "373738383939
343435353636
313132323333"
}
