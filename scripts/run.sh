#!/usr/bin/env bash
#
# Requirements
# - The `tendermint` binary should be in your $PATH
# - `tmux` should be installed and in your $PATH
# - A `debug` build of the `many-ledger` repository
#
# Usage
# $ cd /path/to/many-ledger
# $ ./script/run.sh
set -pube # .. snicker ..
TM_MIN_VERSION=0.35.0 # The minimum tendermint version supported

toml_set() {
    local tmp=$(mktemp)
    ./target/bin/toml set "$1" "$2" "$3" >"$tmp"
    cp "$tmp" "$1"
    rm $tmp
}

# Ver1 <= Ver2
verlte() {
    printf '%s\n%s' "$1" "$2" | sort -C -V
}

# Ver1 < Ver2
verlt() {
    ! verlte "$2" "$1"
}

check_dep() {
    which "$1" >/dev/null || {
        echo You need the binary \""$1"\" installed and accessible to use this script.
        echo
        false
    }
}

check_deps() {
    local return_value
    return_value=0
    check_dep tmux || return_value=$((return_value + 1))
    check_dep ssh-keygen || return_value=$((return_value + 1))

    # Check that tendermint is installed AND that it has the minimum version.
    if ! command -v tendermint >/dev/null; then
        echo "The command 'tendermint' could not be found."
        echo "Please install 'tendermint' from https://github.com/tendermint/tendermint/releases"
        echo "The 'tendermint' binary should be in your '$PATH'"
        return_value=$((return_value + 1))
    else
        local tm_current_version
        tm_current_version=$(tendermint version | cut -d '-' -f 1)
        verlt "$tm_current_version" $TM_MIN_VERSION && {
            echo Tendermint version should be at least $TM_MIN_VERSION.
            echo Current version is "$tm_current_version".
            return_value=$((return_value + 1))
        }
    fi

    return $return_value
}

main() {
    local tm_current_version
    tm_current_version="$(tendermint version | cut -d '-' -f 1)"
    echo "Current Tendermint version: $tm_current_version"

    cd "$(dirname "$0")/.."

    local root_dir
    if [ -n "$1" ]; then
        root_dir="$1"
    else
        root_dir=$(mktemp -d)
    fi
    echo Using directory "$root_dir" for tendermint root.

    local tmux_name
    tmux_name="${2:-many}"

    cargo build
    [ -x ./target/bin/toml ] || cargo install --root ./target -- toml-cli
    tmux kill-session -t "$tmux_name" || true

    local pem_root
    pem_root="$root_dir/pem"
    [ -x "$pem_root" ] || {
        # Create 5 keys in the root.
        mkdir -p "$pem_root"
        for fn in "$pem_root"/id{1,2,3,4,5}.pem; do
            ssh-keygen -a 100 -q -P "" -m pkcs8 -t ecdsa -f "$fn"
        done
    }

    [ -x $root_dir/ledger ] || {
        TMHOME="$root_dir/ledger" tendermint init validator
        TMHOME="$root_dir/kvstore" tendermint init validator

        toml_set "$root_dir/ledger/config/config.toml" consensus.create-empty-blocks "false"
        toml_set "$root_dir/ledger/config/config.toml" consensus.create-empty-blocks-interval "20s"
        toml_set "$root_dir/ledger/config/config.toml" consensus.timeout-commit "10s"
        toml_set "$root_dir/ledger/config/config.toml" consensus.timeout-precommit "10s"

        toml_set "$root_dir/ledger/config/config.toml" p2p.laddr "tcp://127.0.0.1:26656"
        toml_set "$root_dir/ledger/config/config.toml" rpc.laddr "tcp://127.0.0.1:26657"
        toml_set "$root_dir/ledger/config/config.toml" proxy-app "tcp://127.0.0.1:26658"
        toml_set "$root_dir/kvstore/config/config.toml" p2p.laddr "tcp://127.0.0.1:16656"
        toml_set "$root_dir/kvstore/config/config.toml" rpc.laddr "tcp://127.0.0.1:16657"
        toml_set "$root_dir/kvstore/config/config.toml" proxy-app "tcp://127.0.0.1:16658"
    }

    tmux new-session -s "$tmux_name" -n tendermint-ledger -d "TMHOME=\"$root_dir/ledger\" tendermint start 2>&1 | tee \"$root_dir/tendermint-ledger.log\""
    tmux new-window -t "$tmux_name" -n tendermint-kvstore "TMHOME=\"$root_dir/kvstore\" tendermint start 2>&1 | tee \"$root_dir/tendermint-kvstore.log\""
    # This makes sure the sessions remain opened when the command exits.
    tmux setw remain-on-exit on

    tmux new-window -t "$tmux_name" -n ledger -e SHELL=bash "./target/debug/many-ledger -v -v --abci --addr 127.0.0.1:8001 --pem \"$pem_root/id1.pem\" --state ./staging/ledger_state.json5 --persistent \"$root_dir/ledger.db\" 2>&1 | tee \"$root_dir/many-ledger.log\""
    tmux new-window -t "$tmux_name" -n ledger-abci "./target/debug/many-abci -v -v --many 127.0.0.1:8000 --many-app http://localhost:8001 --many-pem \"$pem_root/id2.pem\" --abci 127.0.0.1:26658 --tendermint http://127.0.0.1:26657/ 2>&1 | tee \"$root_dir/many-abci-ledger.log\""

    tmux new-window -t "$tmux_name" -n kvstore "./target/debug/many-kvstore -v -v --abci --addr 127.0.0.1:8010 --pem \"$pem_root/id3.pem\" --state ./staging/kvstore_state.json5 2>&1 --persistent \"$root_dir/kvstore.db\" | tee \"$root_dir/many-kvstore.log\""
    tmux new-window -t "$tmux_name" -n kvstore-abci "./target/debug/many-abci -v -v --many 127.0.0.1:8011 --many-app http://127.0.0.1:8010 --many-pem \"$pem_root/id4.pem\" --abci 127.0.0.1:16658 --tendermint http://127.0.0.1:16657/ 2>&1 | tee \"$root_dir/many-abci-kvstore.log\""

    tmux new-window -t "$tmux_name" -n http "./target/debug/http_proxy -v http://localhost:8011 --pem \"$pem_root/id5.pem\" --addr 0.0.0.0:8888 2>&1 | tee \"$root_dir/http.log\""

    tmux new-window -t "$tmux_name"

    tmux -2 attach-session -t "$tmux_name"
}

check_deps
main "${1:-}" "${2:-}"
