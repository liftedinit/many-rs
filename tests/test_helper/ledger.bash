PEM_ROOT="$(mktemp -d)"
CONFIG_ROOT="$(mktemp -d)"

source "$(dirname "${BASH_SOURCE[0]}")/token.bash"

function start_ledger() {
    local root
    local persistent
    local state
    local clean
    local cache_db
    local background_output
    root="$(mktemp -d)"
    persistent="$root/ledger.db"
    state="$GIT_ROOT/staging/ledger_state.json5"
    clean="--clean"
    cache_db=""
    background_output="Running accept thread"

    while (( $# > 0 )); do
        case "$1" in
            --persistent=*) persistent="${1#--persistent=}"; shift ;;
            --state=*) state="${1#--state=}"; shift ;;
            --no-clean) clean=""; shift ;;
            --background_output=*) background_output="${1#--background_output=}"; shift ;;
            --cache) shift; cache_db="$root/request_cache.db"; continue ;;
            --) shift; break ;;
            *) break ;;
        esac
    done

    run_in_background many-ledger \
        -v \
        $clean \
        $cache_db \
        --persistent "$persistent" \
        --state "$state" \
        "$@"
    wait_for_background_output "$background_output"
}

# Do not rename this function `ledger`.
# It clashes with the call to the `ledger` binary on CI
function call_ledger() {
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

    echo "ledger $pem_arg http://localhost:${port}/ $*" >&2
    # `run` doesn't handle empty parameters well, i.e., $pem_arg is empty
    # We need to use `bash -c` to fix the issue
    run bash -c "ledger $pem_arg http://localhost:${port}/ $*"
}

function check_consistency() {
    local pem_arg
    local expected_balance
    local id

    while (( $# > 0 )); do
      case "$1" in
        --pem=*) pem_arg=${1}; shift ;;
        --balance=*) expected_balance=${1#--balance=}; shift;;
        --id=*) id=${1#--id=}; shift ;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    for port in "$@"; do
        # Named parameters that can be empty need to be located after those who can't
        call_ledger "--port=$port" "$pem_arg" balance "$id"
        assert_output --partial "$expected_balance MFX "
    done
}
