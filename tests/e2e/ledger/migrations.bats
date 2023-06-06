GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MIGRATION_ROOT="$GIT_ROOT/staging/ledger_migrations.json"
load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities
}

function teardown() {
    stop_background_run
}

@test "$SUITE: Load migrations" {
    jq '(.migrations[] | select(.name == "Account Count Data Attribute")).block_height |= 20 |
        (.migrations[] | select(.name == "Account Count Data Attribute")).disabled |= empty' \
        "$MIGRATION_ROOT" > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Missing migration (bad length)" {
    jq '(.migrations[] | select(.name == "Dummy Hotfix")) |= empty' \
       "$MIGRATION_ROOT" > "$BATS_TEST_ROOTDIR/migrations.json"
    start_ledger --background_output="Migration Config is missing migration"\
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Missing migration (right length, duplicate)" {
    jq '.migrations |= . + [.[-1]] | .migrations |= . - [.[0]]' \
       "$MIGRATION_ROOT" > "$BATS_TEST_ROOTDIR/migrations.json"
    start_ledger --background_output="Migration Config is missing migration" \
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Unsupported migration type" {
    jq '(.migrations[] | select(.name == "Dummy Hotfix")).name |= "Foobar"' \
       "$MIGRATION_ROOT" > "$BATS_TEST_ROOTDIR/migrations.json"
    start_ledger --background_output="Unsupported migration 'Foobar'" \
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Can disable" {
    jq '(.migrations[] | select(.name == "Block 9400")).block_height |= 40 |
        (.migrations[] | select(.name == "Block 9400")).disabled |= true' \
        "$MIGRATION_ROOT" > "$BATS_TEST_ROOTDIR/migrations.json"
    start_ledger --background_output="Running accept thread" \
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}
