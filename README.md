# many-rs
[![ci](https://img.shields.io/circleci/build/gh/liftedinit/many-rs)](https://app.circleci.com/pipelines/gh/liftedinit/many-rs)
[![coverage](https://img.shields.io/codecov/c/gh/liftedinit/many-rs)](https://app.codecov.io/gh/liftedinit/many-rs)
[![license](https://img.shields.io/github/license/liftedinit/many-rs)](https://github.com/liftedinit/many-rs/blob/main/LICENSE)

A collection of applications and libraries for the [MANY protocol](https://github.com/many-protocol).

Features
- A ledger client/server
- A key-value store client/server
- An application blockchain interface (ABCI)
- A http proxy
- A 4-nodes end-to-end Docker demo
- MANY module interfaces
- MANY common types
- MANY message and transport layers
- MANY client and server
- Hardware Security Module
- CLI developer's tools

# Requirements
- [Bazelisk](https://github.com/bazelbuild/bazelisk) or [Bazel](https://bazel.build/versions/6.0.0/install) >= 6.0.0
- (macOS) [Brew](https://brew.sh/)

# Build

1. Install build dependencies
    ```shell
    # Ubuntu/Debian
    $ sudo apt update && sudo apt install build-essential clang libssl-dev libsofthsm2 libudev-dev 
        libusb-1.0-0-dev bsdextrautils
    
    # macOS
    $ brew update
    $ brew install git bazelisk
    ```
1. Build
    ```shell
    $ git clone https://github.com/liftedinit/many-rs.git
    $ cd many-rs
    $ bazel build //...
    ```
1. Tests
    ```shell
    # Unit/integration tests
    $ bazel test --config=all-features //...
   
    # E2E integration tests
    $ bazel test --config=all-features //tests/e2e/kvstore:bats-e2e-kvstore
    $ bazel test --config=all-features //tests/e2e/ledger:bats-e2e-ledger
    $ bazel test --balance_testing --migration_testing --config=remote-cache //tests/e2e/ledger:bats-e2e-ledger-tokens
   
    # Resiliency integration tests (Linux only - requires Docker)
    $ bazel test //tests/resiliency/kvstore:bats-resiliency-kvstore 
    $ bazel test --config=all-features //tests/resiliency/ledger:bats-resiliency-ledger
    ```

# Usage example
Below are some examples of how to use the different CLI.

## Ledger cluster 
```shell
# Create a 4-nodes Ledger cluster. Requires local Docker. Linux only
$ bazel run //:start-ledger-cluster

# Create a 4-nodes Ledger cluster in the background
$ bazel run //:start-ledger-cluster-detached

# Stop the ledger cluster
$ bazel run //:stop-ledger-cluster 
```

## Balance
```shell
# Query the local ledger cluster
$ bazel run //src/ledger -- --pem $(pwd)/keys/id1.pem balance
  1000000000 MFX (mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz)
```

## Send tokens
```shell
# Send tokens from id1.pem to id2.pem
$ bazel run //src/ledger -- --pem $(pwd)/keys/id1.pem send mahukzwuwgt3porn6q4vq4xu3mwy5gyskhouryzbscq7wb2iow 10000 MFX
2023-03-13T19:07:20.120255Z  INFO ledger: Async token: a560d5409a18ae493ce457bb4008da0afc3d383c2a505979a963c26398f51fc9
  Waiting for async response
null

# Check the balance of the new ID
$ bazel run //src/ledger -- --pem $(pwd)/keys/id2.pem balance
       10000 MFX (mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz)
```

## Print the MANY ID of a key file
```shell
$ bazel run //src/many -- id $(pwd)/keys/id1.pem
maffbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wijp
```

## Retrieve the status of a running MANY server
```shell
$ bazel run //src/many -- message --server https://alberto.app/api 'status' '{}'
{_
    0: 1,
    1: "AbciModule(many-ledger)",
    2: h'a5010103270481022006215820e5cd546d5292af5d9f0ffd54b57ff555c51b91a249b9cf544010a3c01cfa75a2',
    3: 10000_1(h'01378dd9916915fb276116ff4bc13c04a4e413f663e04b710199c46021'),
    4: [0, 1, 2, 4, 6, 8, 9, 1002_1],
    5: "0.1.0",
    7: 300_1,
}
```

# Developers
## Contributing
Read our [Contributing Guidelines](https://github.com/liftedinit/.github/blob/main/docs/CONTRIBUTING.md)
## Crates

Here's a list of crates published by this repository and their purposes.
You can visit their crates entries (linked below) for more information.

### Published to crates.io

* `many`([crates](https://crates.io/crate/many), [docs](https://docs.rs/many))
    – Contains the CLI tool to contact and diagnose MANY servers.
* `many-client`([crates](https://crates.io/crate/many-client), [docs](https://docs.rs/many-client))
  – Types and methods to talk to the MANY network.
* `many-client-macros`([crates](https://crates.io/crate/many-client-macros), [docs](https://docs.rs/many-client-macros))
    – `many-client` procedural macro
* `many-cli-helpers`([crate](https://crates.io/crate/many-cli-helpers), [docs](https://docs.rs/many-cli-helpers))) 
    – Common CLI flags
* `many-error`([crates](https://crates.io/crate/many-error), [docs](https://docs.rs/many-error))
    – Error and Reason types, as defined by the specification.
* `many-identity`([crates](https://crates.io/crate/many-identity), [docs](https://docs.rs/many-identity))
    – Types for managing an identity, its address and traits related to signing/verification of messages.
* `many-identity-dsa`([crates](https://crates.io/crate/many-identity-dsa), [docs](https://docs.rs/many-identity-dsa))
    – Digital Signature identity, verifiers and utility functions. 
      This crate has features for all supported algorithms (e.g. `ed25519`).
* `many-identity-hsm`([crates](https://crates.io/crate/many-identity-hsm), [docs](https://docs.rs/many-identity-hsm))
    – Hardware Security Module based identity, verifiers and utility functions.
* `many-identity-webauthn`([crates](https://crates.io/crate/many-identity-webauthn), [docs](https://docs.rs/many-identity-webauthn))
    – Verifiers for WebAuthn signed envelopes.
      This uses our custom WebAuthn format, which is not fully compliant with the [WebAuthn standard](https://webauthn.io).
      See the [Lifted WebAuthn Auth Paper](https://coda.io/@hans-larsen/lifted-webauthn-auth).
* `many-macros`([crates](https://crates.io/crate/many-macros), [docs](https://docs.rs/many-macros))
    – Contains macros to help with server and module declaration and implementations.
* `many-migration`([crates](https://crates.io/crate/many-migration), [docs](https://docs.rs/many-migration))
  – Storage/Transaction migration framework.
* `many-mock`([crates](https://crates.io/crate/many-mock), [docs](https://docs.rs/many-mock))
    – Utility types for creating mocked MANY servers.
* `many-modules`([crates](https://crates.io/crate/many-modules), [docs](https://docs.rs/many-modules))
    – All modules declared in the specification.
* `many-protocol`([crates](https://crates.io/crate/many-protocol), [docs](https://docs.rs/many-protocol))
    – Types exclusively associated with the protocol.
      This does not include types that are related to attributes or modules.
* `many-server`([crates](https://crates.io/crate/many-server), [docs](https://docs.rs/many-server))
    – Types and methods to create a MANY network server and neighborhood.
* `many-types`([crates](https://crates.io/crate/many-types), [docs](https://docs.rs/many-types))
  – General types related to CBOR encoding, or to the specification.

## Using Bazel
### Remote cache
```shell
# Use BuildBuddy remote cache
$ bazel build --config=remote-cache //...
```

### Code formatting
```shell
# Check code formatting
$ bazel build --config=rustfmt-check //...

# Apply format changes using
$ bazel run @rules_rust//:rustfmt
```

### Lint
```shell
# Clippy
$ bazel build --config=clippy //...
```

## Generating new keys
### ECDSA
```shell
$ ssh-keygen -a 100 -q -P "" -m pkcs8 -t ecdsa -f key_name.pem
```

### Ed25519
```shell
# Requires openssl@3 on macOS
$ openssl genpkey -algorithm Ed25519 -out key_name.pem
```

## References

- Concise Binary Object Representation (CBOR): [RFC 8949](https://www.rfc-editor.org/rfc/rfc8949.html)
- CBOR Object Signing and Encryption (COSE): [RFC 8152](https://datatracker.ietf.org/doc/html/rfc8152)
- Platform-independent API to cryptographic tokens: [PKCS #11](https://docs.oasis-open.org/pkcs11/pkcs11-base/v2.40/os/pkcs11-base-v2.40-os.html)
- Blockchain application platform: [Tendermint](https://docs.tendermint.com/master/)
- Persistent key-value store: [RocksDB](https://rocksdb.org/)
- Concise Binary Object Representation (CBOR): [RFC 8949](https://www.rfc-editor.org/rfc/rfc8949.html)
- CBOR Object Signing and Encryption (COSE): [RFC 8152](https://datatracker.ietf.org/doc/html/rfc8152)
- Platform-independent API to cryptographic tokens: [PKCS #11](https://docs.oasis-open.org/pkcs11/pkcs11-base/v2.40/os/pkcs11-base-v2.40-os.html)


## Tools

- CBOR playground: [CBOR.me](https://cbor.me)
- CBOR diagnostic utilities: [cbor-diag](https://github.com/cabo/cbor-diag)
- Software Hardware Security Module (HSM): [SoftHSM2](https://github.com/opendnssec/SoftHSMv2)
- Bash automated testing system: [bats-core](https://github.com/bats-core/bats-core)
- Container engine: [Docker](https://www.docker.com/)
- The MANY libraries: [many-rs](https://github.com/liftedinit/many-rs)
