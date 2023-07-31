GIT_ROOT="$BATS_TEST_DIRNAME/../../../"

load '../../test_helper/load'
load '../../test_helper/kvstore'
load '../../test_helper/http-proxy'

function setup() {
    load "test_helper/bats-assert/load"
    load "test_helper/bats-support/load"

    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    start_kvstore --pem "$(pem 0)"
    start_http_proxy --pem "$(pem 0)"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: http-proxy works" {
    call_kvstore --pem=1 --port=8000 put http/foo $(echo '<h1>Hello world</h1>' | base64 -w 0)
    run curl http://localhost:8880/foo
    assert_output --partial '<h1>Hello world</h1>'
}
