PEM_ROOT="$(mktemp -d)"
CONFIG_ROOT="$(mktemp -d)"

function start_web() {
    local root
    local persistent
    local state
    local clean
    local cache_db
    root="$(mktemp -d)"
    persistent="$root/web.db"
    cache_db=""
    state="$GIT_ROOT/staging/web_state.json5"
    clean="--clean"

    while (( $# > 0 )); do
        case "$1" in
            --persistent=*) persistent="${1#--persistent=}"; shift ;;
            --state=*) state="${1#--state=}"; shift ;;
            --no-clean) clean=""; shift ;;
            --cache) cache_db="--cache-db=$root/request_cache.db"; shift ;;
            --) shift; break ;;
            *) break ;;
        esac
    done

    run_in_background many-web \
        -v \
        $clean \
        $cache_db \
        --persistent "$persistent" \
        --state "$state" \
        "$@"
    wait_for_background_output "Running accept thread"
}

function call_web() {
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

    echo "web $pem_arg http://localhost:${port}/ $*" >&2
    # `run` doesn't handle empty parameters well, i.e., $pem_arg is empty
    # We need to use `bash -c` to this the issue
    run bash -c "web $pem_arg http://localhost:${port}/ $*"
}

function check_consistency() {
    local pem_arg
    local key
    local expected_value

    while (( $# > 0 )); do
      case "$1" in
        --pem=*) pem_arg=${1}; shift ;;
        --value=*) expected_value=${1#--value=}; shift;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    for port in "$@"; do
        # Named parameters that can be empty need to be located after those who can't
        call_web --port="$port" "$pem_arg" list
        assert_output --partial "$expected_value"
    done
}
