//

local generate_allow_addrs_flag(allow_addrs) =
    if allow_addrs then
        ["--allow-addrs", "/genfiles/allow_addrs.json5"]
    else
        [];

local abci(i, user, abci_tag, allow_addrs) = {
    image: "lifted/many-abci:" + abci_tag,
    ports: [ (8000 + i) + ":8000" ],
    volumes: [ "./node" + i + ":/genfiles:ro" ],
    user: "" + user,
    command: [
        "many-abci",
        "--verbose", "--verbose",
        "--many", "0.0.0.0:8000",
        "--many-app", "http://kvstore-" + i + ":8000",
        "--many-pem", "/genfiles/abci.pem",
        "--abci", "0.0.0.0:26658",
        "--tendermint", "http://tendermint-" + i + ":26657/"
    ] + generate_allow_addrs_flag(allow_addrs),
    depends_on: [ "kvstore-" + i ],
};

local kvstore(i, user, kvstore_tag) = {
    image: "lifted/many-kvstore:" + kvstore_tag,
    user: "" + user,
    volumes: [
        "./node" + i + "/persistent-kvstore:/persistent",
        "./node" + i + ":/genfiles:ro",
    ],
    command: [
        "many-kvstore",
        "--verbose", "--verbose",
        "--abci",
        "--state=/genfiles/kvstore_state.json5",
        "--pem=/genfiles/kvstore.pem",
        "--persistent=/persistent/kvstore.db",
        "--addr=0.0.0.0:8000",
    ],
};

local tendermint(i, user, tendermint_tag) = {
    image: "tendermint/tendermint:v" + tendermint_tag,
    command: [
        "--log-level", "info",
        "start",
        "--rpc.laddr", "tcp://0.0.0.0:26657",
        "--proxy-app", "tcp://abci-" + i + ":26658",
    ],
    user: "" + user,
    volumes: [
        "./node" + i + "/tendermint/:/tendermint"
    ],
    ports: [ "" + (26600 + i) + ":26600" ],
};

function(nb_nodes=4, user=1000, tendermint_tag="0.35.4", abci_tag="latest", kvstore_tag="latest", allow_addrs=false) {
    version: '3',
    services: {
        ["abci-" + i]: abci(i, user, abci_tag, allow_addrs) for i in std.range(0, nb_nodes - 1)
    } + {
        ["kvstore-" + i]: kvstore(i, user, kvstore_tag) for i in std.range(0, nb_nodes - 1)
    } + {
        ["tendermint-" + i]: tendermint(i, user, tendermint_tag) for i in std.range(0, nb_nodes - 1)
    },
}
