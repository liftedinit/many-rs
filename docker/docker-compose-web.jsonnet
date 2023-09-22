//

local generate_allow_addrs_flag(allow_addrs) =
    if allow_addrs then
        ["--allow-addrs", "/genfiles/allow_addrs.json5"]
    else
        [];

local abci(i, user, allow_addrs) = {
    image: "bazel/src/many-abci:many-abci-image",
    ports: [ (8000 + i) + ":8000" ],
    volumes: [
        // TODO: have a volume specifically created for the cache db.
        // Right now we reuse the same volume as the web db.
        "./node" + i + "/persistent-web:/persistent",
        "./node" + i + ":/genfiles:ro",
    ],
    user: "" + user,
    command: [
        "--verbose", "--verbose",
        "--many", "0.0.0.0:8000",
        "--many-app", "http://web-" + i + ":8000",
        "--many-pem", "/genfiles/abci.pem",
        "--cache-db", "/persistent/abci_request_cache.db",
        "--abci", "0.0.0.0:26658",
        "--tendermint", "http://tendermint-" + i + ":26657/"
    ] + generate_allow_addrs_flag(allow_addrs),
    depends_on: [ "web-" + i ],
};

local proxy(i, user) = {
    image: "bazel/src/http-proxy:http-proxy-image",
    ports: [ (8880 + i) + ":8880" ],
    user: "" + user,
    command: [
        "--verbose", "--verbose",
        "--addr", "0.0.0.0:8880",
        "http://web-" + i + ":8000",
    ],
    depends_on: [ "web-" + i ],
};

local web(i, user) = {
    image: "bazel/src/many-web:many-web-image",
    user: "" + user,
    volumes: [
        "./node" + i + "/persistent-web:/persistent",
        "./node" + i + ":/genfiles:ro",
    ],
    command: [
        "--verbose", "--verbose",
        "--abci",
        "--state=/genfiles/web_state.json5",
        "--pem=/genfiles/web.pem",
        "--persistent=/persistent/web.db",
        "--addr=0.0.0.0:8000",
        "--domain=ghostcloud.org"
    ],
};

local tendermint(i, user) = {
    image: "bazel/docker:tendermint_image",
    command: [
        "start",
        "--rpc.laddr", "tcp://0.0.0.0:26657",
        "--proxy_app", "tcp://abci-" + i + ":26658",
    ],
    user: "" + user,
    volumes: [
        "./node" + i + "/tendermint/:/tendermint"
    ],
    ports: [ "" + (26600 + i) + ":26600" ],
};

function(nb_nodes=4, user=1000, allow_addrs=false) {
    version: '3',
    services: {
        ["abci-" + i]: abci(i, user, allow_addrs) for i in std.range(0, nb_nodes - 1)
    } + {
        ["web-" + i]: web(i, user) for i in std.range(0, nb_nodes - 1)
    } + {
        ["tendermint-" + i]: tendermint(i, user) for i in std.range(0, nb_nodes - 1)
    } + {
        ["proxy-" + i]: proxy(i, user) for i in std.range(0, nb_nodes - 1)
    },
}
