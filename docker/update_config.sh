#!/usr/bin/env bash

update_toml_key() {
  local file section key value temp_file
  file="$1"
  section="$2"
  key="$3"
  value="$4"
  temp_file="$(mktemp)"

  if [[ "$section" = '' ]]; then
    csplit -s -f "${temp_file}_" "${file}" "/^\[/"

    (
      sed "s/^${key}.*$/${key} = ${value}/" "${temp_file}_00"
      rm "${temp_file}_00"
      cat "${temp_file}_"*
    ) > "$file"
  elif grep -E "^\\[${section}]\$" "$file" > /dev/null; then
    cp "$file" "$temp_file"
    csplit -s -f "${temp_file}_" "${file}" "/\[${section}\]/"

    (
      cat "${temp_file}_00"
      sed "s/^${key} .*$/${key} = ${value}/" "${temp_file}_01"
    ) > "$file"
  else
    (
      echo
      echo "[${section}]"
      echo "${key} = ${value}"
    ) >> "$file"
  fi
}

usage() {
  cat <<END_OF_USAGE 1>&2

Usage: $0 -f CONFIG_FILE [-i IP_ADDRESS_RANGE] [-p PORT] <start> <end>

    -c CONFIG_ROOT       A path to the config root containing the config.toml and
                         node_key.json, with \"%\" replaced by the node id.
    -i IP_ADDRESS_RANGE  An IP Address start for nodes, which replaces
                         \"%\" with the node id. Default \"10.254.254.%\".
    -p PORT              The port instances are listening to, default 26656.

END_OF_USAGE
  exit 1
}

ip_range=10.254.254.%
port=26656
config_root=""
dweb=0
while getopts ":i:p:c:r:d:" opt; do
    case "${opt}" in
        i)  ip_range="${OPTARG}"
            ;;
        p)  port="${OPTARG}"
            [[ "$port" =~ ^[0-9]+$ ]] || usage
            ;;
        c)  config_root="${OPTARG}"
            ;;
        r)  tm_root="${OPTARG}"
            ;;
        d)  dweb="${OPTARG}"
            ;;
        *)  usage
            ;;
    esac
done
shift $((OPTIND-1))

[ "$config_root" ] || usage

NB_NODES=$(( $1 - 1 ))

all_validators="$(
  for node in $(seq 0 "$NB_NODES"); do
    jq '{ address: .address, pub_key: .pub_key }' "${config_root//%/$node}/priv_validator_key.json" | jq ".name = \"tendermint-$node\" | .power = \"1000\""
  done | jq -s -c
)"

for node in $(seq 0 "$NB_NODES"); do
  root="${config_root//%/$node}"
  config_toml_path="$root"/config.toml
  genesis_json_path="$root"/genesis.json

  if ! [ -f "$config_toml_path" ]; then
     echo Configuration file "'$config_toml_path'" could not be found. 1>&2
     exit 1
  fi
  if ! [ -f "$genesis_json_path" ]; then
     echo Configuration file "'$genesis_json_path'" could not be found. 1>&2
     exit 1
  fi

  echo Updating \""$root"\"...

  peer_ids=$(seq 0 "$NB_NODES" | grep -v "$node")
  peers=$(for peer in $peer_ids; do
    node_id=$(docker run -u $(id -u):$(id -g) --rm -v "${tm_root//%/$peer}:/tendermint" bazel/docker:tendermint_image show_node_id | tail -n +2 | tr -d '\n')
    ip_address=${ip_range//%/$peer}

    printf '%s' "$node_id@$ip_address:$port,"
  done | sed 's/,$//')

  update_toml_key "$config_toml_path" '' proxy_app "\"tcp:\\/\\/abci-${node}:26658\\/\""
  update_toml_key "$config_toml_path" '' moniker "\"many-tendermint-${node}\""
  update_toml_key "$config_toml_path" p2p persistent_peers "\"$peers\""
  update_toml_key "$config_toml_path" consensus timeout_commit "\"2s\""
  update_toml_key "$config_toml_path" consensus timeout_precommit "\"2s\""
  update_toml_key "$config_toml_path" p2p max_packet_msg_payload_size "1400"

  # Experimental section for dweb
  if [ $dweb -eq 1 ]; then
    echo "Enabling experimental dweb configuration"
    update_toml_key "$config_toml_path" rpc max_body_bytes "5000000" # Bump the max tx (rpc) body from 1Mb to 5Mb
    update_toml_key "$config_toml_path" mempool max_txs_bytes "5368709120" # Bump the max total size of all txs in the mempool from 5Mb to 25Mb
    update_toml_key "$config_toml_path" mempool max_tx_bytes "5242880" # Bump the max size of a single tx in the mempool from 1Mb to 5Mb
  fi

  # update_toml_key "$config_toml_path" p2p bootstrap-peers "\"$peers\""
done

# Same genesis data for all.
genesis_temp_file=$(mktemp)
jq ".validators = ${all_validators} | .chain_id = \"many-e2e-ledger\"" "${config_root//%/1}/genesis.json" > "$genesis_temp_file"
for node in $(seq 0 "$NB_NODES"); do
  cp "$genesis_temp_file" "${config_root//%/$node}/genesis.json"
done
