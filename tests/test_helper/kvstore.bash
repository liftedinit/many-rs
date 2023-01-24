PEM_ROOT="$(mktemp -d)"
CONFIG_ROOT="$(mktemp -d)"

function start_kvstore() {
    local persistent
    local state
    local clean
    persistent="$(mktemp -d)"
    state="$GIT_ROOT/staging/kvstore_state.json5"
    clean="--clean"

    while (( $# > 0 )); do
        case "$1" in
            --persistent=*) persistent="${1#--persistent=}"; shift ;;
            --state=*) state="${1#--state=}"; shift ;;
            --no-clean) clean=""; shift ;;
            --) shift; break ;;
            *) break ;;
        esac
    done

    run_in_background "$GIT_ROOT/target/debug/many-kvstore" \
        -v \
        $clean \
        --persistent "$persistent" \
        --state "$state" \
        "$@"
    wait_for_background_output "Running accept thread"
}

# Do not rename this function `kvstore`.
# It clashes with the call to the `kvstore` binary on CI
function call_kvstore() {
    local pem_arg
    local port

    while (( $# > 0 )); do
      case "$1" in
        --pem=*) pem_arg="--pem=$(pem "${1#--pem=}")"; shift ;;
        --port=*) port=${1#--port=}; shift ;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    local kvstorecmd
    [[ "$CI" == "true" ]]\
      && kvstorecmd="kvstore" \
      || kvstorecmd="$GIT_ROOT/target/debug/kvstore"

    echo "${kvstorecmd} $pem_arg http://localhost:${port}/ $*" >&2
    # `run` doesn't handle empty parameters well, i.e., $pem_arg is empty
    # We need to use `bash -c` to this the issue
    run bash -c "${kvstorecmd} $pem_arg http://localhost:${port}/ $*"
}

function check_consistency() {
    local pem_arg
    local key
    local expected_value

    while (( $# > 0 )); do
      case "$1" in
        --pem=*) pem_arg=${1}; shift ;;
        --key=*) key=${1#--key=}; shift ;;
        --value=*) expected_value=${1#--value=}; shift;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    for port in "$@"; do
        # Named parameters that can be empty need to be located after those who can't
        call_kvstore --port="$port" "$pem_arg" get "$key"
        assert_output --partial "$expected_value"
    done
}
