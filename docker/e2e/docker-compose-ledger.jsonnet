//

local generate_balance_flags(id_with_balances="", token="mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz") =
    if std.length(id_with_balances) == 0 then
        []
    else std.map(
        function(x) (
             local g = std.split(x, ":");
             local id = g[0];
             local amount = if std.length(g) > 1 then g[1] else "10000000000";
             local symbol = if std.length(g) > 2 then g[2] else token;
             "--balance-only-for-testing=" + std.join(":", [id, amount, symbol])
        ),
        std.split(id_with_balances, " ")
    );

local load_migrations(enable_migrations) =
    if enable_migrations then
        ["--migrations-config=/genfiles/migrations.json"]
    else
        [];

local generate_allow_addrs_flag(allow_addrs) =
    if allow_addrs then
        ["--allow-addrs=/genfiles/allow_addrs.json5"]
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
        "--many-app", "http://ledger-" + i + ":8000",
        "--many-pem", "/genfiles/abci.pem",
        "--abci", "0.0.0.0:26658",
        "--tendermint", "http://tendermint-" + i + ":26657/"
    ] + generate_allow_addrs_flag(allow_addrs),
    depends_on: [ "ledger-" + i ],
};

local ledger(i, user, id_with_balances, ledger_tag, enable_migrations) = {
    image: "lifted/many-ledger:" + ledger_tag,
    user: "" + user,
    volumes: [
        "./node" + i + "/persistent-ledger:/persistent",
        "./node" + i + ":/genfiles:ro",
    ],
    command: [
        "many-ledger",
        "--verbose", "--verbose",
        "--abci",
        "--state=/genfiles/ledger_state.json5",
        "--pem=/genfiles/ledger.pem",
        "--persistent=/persistent/ledger.db",
        "--addr=0.0.0.0:8000",
    ] + load_migrations(enable_migrations)
      + generate_balance_flags(id_with_balances)
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

function(nb_nodes=4, user=1000, id_with_balances="", tendermint_tag="0.35.4", abci_tag="latest", ledger_tag="latest", allow_addrs=false, enable_migrations=false) {
    version: '3',
    services: {
        ["abci-" + i]: abci(i, user, abci_tag, allow_addrs) for i in std.range(0, nb_nodes - 1)
    } + {
        ["ledger-" + i]: ledger(i, user, id_with_balances, ledger_tag, enable_migrations) for i in std.range(0, nb_nodes - 1)
    } + {
        ["tendermint-" + i]: tendermint(i, user, tendermint_tag) for i in std.range(0, nb_nodes - 1)
    },
}
