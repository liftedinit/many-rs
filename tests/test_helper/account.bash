function account_create() {
    local pem_arg
    while (( $# > 0 )); do
      case "$1" in
        --pem=*) pem_arg="${1}"; shift ;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    account_id="$(many_message "$pem_arg" account.create "$@" | grep -o "h'[0-9a-z]*'" | grep -oE "[0-9a-z][0-9a-z]+")"
    account_many_id=$(many id "$account_id")
    assert [ "${account_many_id::1}" = "m" ]  # Check the account ID starts with an "m"
    assert [ ${#account_many_id} -eq 55 ]     # Check the account ID has the right length
    echo "${account_many_id}"
}
