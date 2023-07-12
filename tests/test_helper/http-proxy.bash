PEM_ROOT="$(mktemp -d)"
CONFIG_ROOT="$(mktemp -d)"

function start_http_proxy() {
    local root
    local addr
    local server
    root="$(mktemp -d)"
    addr="127.0.0.1:8880"
    server="http://127.0.0.1:8000"

    while (( $# > 0 )); do
        case "$1" in
            --addr=*) addr="${1#--addr=}"; shift ;;
            --server=*) server="${1#--server=}"; shift ;;
            --) shift; break ;;
            *) break ;;
        esac
    done

    run_in_background http-proxy \
        -v \
        --addr "$addr" \
        "$server" \
        "$@"
    wait_for_background_output "Running accept thread"
}

