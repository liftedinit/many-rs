# Generate allow_addrs.json5 config file
function generate_allow_addrs_config() {
    for i in "$@";
    do
      pem ${i} > /dev/null
    done
    echo "[]" > "$CONFIG_ROOT"/allow_addrs.json5
    for i in "$PEM_ROOT"/*.pem;
    do
        jq --arg id "$(many id "${i}")" '. += [$id]' < "$CONFIG_ROOT"/allow_addrs.json5 > "$CONFIG_ROOT"/allow_addrs_tmp.json5
        mv "$CONFIG_ROOT"/allow_addrs_tmp.json5 "$CONFIG_ROOT"/allow_addrs.json5
    done
    echo "$CONFIG_ROOT"/allow_addrs.json5
}

. "$(dirname ${BASH_SOURCE[0]})/bats-utils/helpers"
set_bats_test_suite_name "${BASH_SOURCE[0]%/*}"
remove_bats_test_dirs

source "$(dirname "${BASH_SOURCE[0]}")/bats-utils/background-process"

source "$(dirname "${BASH_SOURCE[0]}")/many.bash"
source "$(dirname "${BASH_SOURCE[0]}")/account.bash"
