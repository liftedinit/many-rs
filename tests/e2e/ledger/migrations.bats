GIT_ROOT="$BATS_TEST_DIRNAME/../../../"

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    (
      cd "$GIT_ROOT"
      cargo build --features migration_testing
    )
}

function teardown() {
    stop_background_run
}

@test "$SUITE: Load migrations" {
    echo '
    { "migrations": [
      {
        "name": "Account Count Data Attribute",
        "block_height": 20,
        "issue": "https://github.com/liftedinit/many-framework/issues/190"
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
        "block_height": 0,
        "disabled": true
      }
    ] }' > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Missing migration (bad length)" {
    echo '
    { "migrations": [
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
      }
    ] }' > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --background_output="Migration Config is missing migration"\
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Missing migration (right length, duplicate)" {
    echo '
    { "migrations": [
      {
        "name": "Dummy Hotfix",
        "block_height": 20
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
      }
    ] }' > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --background_output="Migration Config is missing migration" \
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Unsupported migration type" {
    echo '
    { "migrations": [
      {
        "name": "Foobar",
        "block_height": 20
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
      }
    ] }' > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --background_output="Unsupported migration 'Foobar'" \
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Can disable" {
    echo '
    { "migrations": [
      {
        "name": "Account Count Data Attribute",
        "block_height": 20,
        "issue": "https://github.com/liftedinit/many-framework/issues/190"
      },
      {
        "name": "Dummy Hotfix",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Block 9400",
        "block_height": 40,
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
    ] }' > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --background_output="block_height: 40, disabled: true" \
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}
