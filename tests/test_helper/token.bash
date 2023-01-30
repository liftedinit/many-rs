# Creates a token
# The global variable `SYMBOL` will be set to the new token symbol
function create_token() {
    local ext_info_type
    local ext_args
    local pem_arg
    local port
    local error

    while (( $# > 0 )); do
       case "$1" in
         --pem=*) pem_arg=${1}; shift ;;                                    # Identity to create the token with
         --port=*) port=${1}; shift ;;                                      # Port of the ledger server
         --ext_info_type=*) ext_info_type=${1#--ext_info_type=}; shift ;;   # Extended info to add at token creation
         --error=*) error=${1#--error=}; shift ;;                           # If this is set, token creation is expected to fail
         --) shift; break ;;
         *) break ;;
       esac
     done

    if [ "${ext_info_type}" = "image" ]; then
        ext_args="logo image \"$BATS_TEST_ROOTDIR/image.png\""
    elif [ "${ext_info_type}" = "unicode" ]; then
        ext_args='logo unicode "'∑'"'
    elif [ "$ext_info_type" = "memo" ]; then
        ext_args='memo "My memo"'
    fi

    call_ledger ${pem_arg} ${port} token create "Foobar" "FBR" 9 "$ext_args" "$@"

    if [[ $error = "anon" ]]; then
        assert_output --partial "Invalid Identity; the sender cannot be anonymous."
    elif [[ $error = "invalid_sender" ]]; then
        assert_output --partial "Unauthorised Token endpoints sender."
    else
        SYMBOL=$(echo $output | grep -oE '"m[a-z0-9]+"' | head -n 1)
        assert [ ${#SYMBOL} -eq 57 ]     # Check the account ID has the right length (55 chars + "")

        assert_output --partial "name: \"Foobar\""
        assert_output --partial "ticker: \"FBR\""
        assert_output --partial "decimals: 9"
        assert_output --regexp "owner:.*$(identity ${pem_arg#--pem=}).*)"

        call_ledger --port=8000 token info "${SYMBOL}"
        if [ "${ext_info_type}" = "image" ]; then
            assert_output --partial "image/png"
            assert_output --regexp "binary: \[.*104,.*101,.*108,.*108,.*111,.*\]"
        elif [ "${ext_info_type}" = "unicode" ]; then
            assert_output --partial "'∑'"
        elif [ "$ext_info_type" = "memo" ]; then
            assert_output --partial "\"My memo\""
        fi
    fi
}

# Create a new token and assign a new account as the token owner
# `identity(2)` will be assigned the permission given by `--perm`
function token_account() {
    local ext_info_type
    local perm

    while (( $# > 0 )); do
       case "$1" in
         --perm=*) perm=${1#--perm=}; shift ;;                              # Identity to create the token with
         --ext_info_type=*) ext_info_type=${1#--ext_info_type=}; shift ;;   # Extended info to add at token creation
         --) shift; break ;;
         *) break ;;
       esac
     done

    create_token --pem=1 --port=8000 --ext_info_type=${ext_info_type}

    account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["'${perm}'"] }, 2: [3] }')

    # Account is the new token owner
    call_ledger --pem=1 --port=8000 token update --owner "${account_id}" "${SYMBOL}"
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --regexp "owner:.*${account_id}.*)"
}

