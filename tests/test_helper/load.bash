MANY_BAZEL_OPTIONS="--noshow_progress --ui_event_filters=error --config=remote-cache"
MANY_BAZEL_SCRIPT_DIR="$GIT_ROOT/tmp"

# Create links to binaries built by Bazel
function create_binary_links() {
    echo '# Creating binary links' >&3
    mkdir -p "$MANY_BAZEL_SCRIPT_DIR"
    for i in $(bazel query "kind(rust_binary, //... except filter("build_script_", //...) except filter("image_binary", //...))" --output=label);
    do
        bazel run $MANY_FEATURES $MANY_BAZEL_OPTIONS --script_path="$MANY_BAZEL_SCRIPT_DIR/${i##*:}" $i;
    done
}

function remove_binary_links() {
    rm -r "$MANY_BAZEL_SCRIPT_DIR"
}

# Do not regen Docker images on CI.
# Docker images will be pulled by CI.
function ciopt() {
    [[ "$CI" == "true" ]]\
      && echo ${1}-no-img-regen \
      || echo ${1}
}

# Use the nightly Docker image when running the tests on CI.
# Use the latest Docker image when running the tests locally.
function img_tag {
    [[ "$CI" == "true" ]]\
      && echo "nightly" \
      || echo "latest"
}

# Generate allow_addrs.json5 config file
function generate_allow_addrs_config() {
    for i in "$@";
    do
      pem ${i} > /dev/null
    done
    echo "[]" > "$CONFIG_ROOT"/allow_addrs.json5
    for i in "$PEM_ROOT"/*.pem;
    do
        jq --arg id "$(call_many id "${i}")" '. += [$id]' < "$CONFIG_ROOT"/allow_addrs.json5 > "$CONFIG_ROOT"/allow_addrs_tmp.json5
        mv "$CONFIG_ROOT"/allow_addrs_tmp.json5 "$CONFIG_ROOT"/allow_addrs.json5
    done
    echo "$CONFIG_ROOT"/allow_addrs.json5
}

source "$(dirname "${BASH_SOURCE[0]}")/bats-assert/load.bash"
source "$(dirname "${BASH_SOURCE[0]}")/bats-support/load.bash"

. "$(dirname ${BASH_SOURCE[0]})/bats-utils/helpers"
set_bats_test_suite_name "${BASH_SOURCE[0]%/*}"
remove_bats_test_dirs

source "$(dirname "${BASH_SOURCE[0]}")/bats-utils/background-process"

source "$(dirname "${BASH_SOURCE[0]}")/many.bash"
source "$(dirname "${BASH_SOURCE[0]}")/account.bash"
