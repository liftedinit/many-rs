PEM_ROOT="$(mktemp -d)"
CONFIG_ROOT="$(mktemp -d)"

source "$(dirname "${BASH_SOURCE[0]}")/token.bash"

function start_ledger() {
    local persistent
    local state
    local clean
    local background_output
    persistent="$(mktemp -d)"
    state="$GIT_ROOT/staging/ledger_state.json5"
    clean="--clean"
    background_output="Running accept thread"

    while (( $# > 0 )); do
        case "$1" in
            --persistent=*) persistent="${1#--persistent=}"; shift ;;
            --state=*) state="${1#--state=}"; shift ;;
            --no-clean) clean=""; shift ;;
            --background_output=*) background_output="${1#--background_output=}"; shift ;;
            --) shift; break ;;
            *) break ;;
        esac
    done

    run_in_background "$GIT_ROOT/target/debug/many-ledger" \
        -v \
        $clean \
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

    local ledgercmd
    [[ "$CI" == "true" ]]\
      && ledgercmd="ledger" \
      || ledgercmd="$GIT_ROOT/target/debug/ledger"

    echo "${ledgercmd} $pem_arg http://localhost:${port}/ $*" >&2
    # `run` doesn't handle empty parameters well, i.e., $pem_arg is empty
    # We need to use `bash -c` to fix the issue
    run bash -c "${ledgercmd} $pem_arg http://localhost:${port}/ $*"
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
