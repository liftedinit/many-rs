FROM rust:latest as chef
# We only pay the installation cost once,
# it will be cached from the second build onwards
RUN cargo install cargo-chef
WORKDIR /src

FROM chef AS planner
COPY /rust-toolchain.toml /Cargo.* ./
RUN cargo version
COPY /src/http_proxy/Cargo.toml ./src/http_proxy/
COPY /src/idstore-export/Cargo.toml ./src/idstore-export/
COPY /src/kvstore/Cargo.toml ./src/kvstore/
COPY /src/ledger/Cargo.toml ./src/ledger/
COPY /src/many-abci/Cargo.toml ./src/many-abci/
COPY /src/many-kvstore/Cargo.toml ./src/many-kvstore/
COPY /src/many-ledger/Cargo.toml ./src/many-ledger/
RUN find /src/
RUN --mount=type=ssh cargo chef prepare  --recipe-path recipe.json

FROM chef as builder
WORKDIR /src

RUN apt-get update && \
    apt-get -y upgrade && \
    apt-get -y install make openssh-client git jq

# Install build dependencies
RUN apt-get -y install musl-dev libssl-dev clang lld librocksdb-dev

RUN mkdir ~/.cargo && \
    echo "[net]" >> ~/.cargo/config.toml && \
    echo "git-fetch-with-cli = true" >> ~/.cargo/config.toml && \
    echo "retry = 2" >> ~/.cargo/config.toml


COPY --from=planner /src/recipe.json recipe.json
# Just make sure we're already caching the rust toolchain.
COPY /rust-toolchain.toml .

# Build dependencies - this is the caching Docker layer!
RUN --mount=type=ssh cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .
RUN --mount=type=ssh cargo build --release --all-features
