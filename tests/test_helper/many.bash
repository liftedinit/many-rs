function pem() {
    [ -f "$PEM_ROOT/id-$1.pem" ] || ssh-keygen -a 100 -q -P "" -m pkcs8 -t ecdsa -f "$PEM_ROOT/id-$1.pem" >/dev/null
    echo "$PEM_ROOT/id-$1.pem"
}

# Print the X-coord of an Ed25519 public key
function ed25519_x_coord() {
    openssl pkey -in "$(pem "$1")" -text_pub -noout | grep "    " | awk '{printf("%s ",$0)} END { printf "\n" }' | tr -d ' ' | tr -d ':'
}

# Requires `cbor-diag` from https://github.com/Nemo157/cbor-diag-rs
# $ cargo install cbor-diag-cli
function key2cose() {
  echo "{1: 1, 2: h'"$(identity_hex "$1")"', 3: -8, 4: [2], -1: 6, -2: h'"$(ed25519_x_coord "$1")"'}" | cbor-diag --to bytes | xxd -p -c 10000
}

# Return 16 bytes of random data
function cred_id() {
  hexdump -vn16 -e'4/4 "%08X" 1 "\n"' /dev/urandom
}

function many_message() {
    local pem_arg

    while (( $# > 0 )); do
       case "$1" in
         --pem=*) pem_arg="--pem=$(pem ${1#--pem=})"; shift ;;
         --) shift; break ;;
         *) break ;;
       esac
     done

    command many message "$pem_arg" --server http://localhost:8000 "$@"
}

function identity() {
    command many id "$(pem "$1")"
}

function subresource() {
    command many id "$(pem "$1")" "$2"
}

function identity_hex() {
    command many id $(many id "$(pem "$1")")
}

function account() {
    command many id mahukzwuwgt3porn6q4vq4xu3mwy5gyskhouryzbscq7wb2iow "$1"
}

function wait_for_block() {
    local block
    local current
    block=$1
    # Using [0-9] instead of \d for grep 3.8
    # https://salsa.debian.org/debian/grep/-/blob/debian/master/NEWS
    current=$(many message --server http://localhost:8000/ blockchain.info | grep -oE '1: [0-9]+' | colrm 1 3)
    while [ "$current" -lt "$block" ]; do
      sleep 1
      current=$(many message --server http://localhost:8000/ blockchain.info | grep -oE '1: [0-9]+' | colrm 1 3)
    done >/dev/null
}
