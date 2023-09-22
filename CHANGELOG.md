# Changelog

All notable changes to this project will be documented in this file.

## [0.2.0] - 2023-09-22

### Features

- Decentralized web hosting ([#403](https://github.com/liftedinit/many-rs/issues/403))

### Miscellaneous Tasks

- Rename `docker-compose` to `docker compose` ([#410](https://github.com/liftedinit/many-rs/issues/410))

## [0.1.7] - 2023-08-15

### Bug Fixes

- Rustsec-2022-0093 ([#409](https://github.com/liftedinit/many-rs/issues/409))

### CI

- Disable macos nightly for now ([#406](https://github.com/liftedinit/many-rs/issues/406))

### Features

- Http-proxy rework ([#397](https://github.com/liftedinit/many-rs/issues/397))
- [**breaking**] Kvstore list filters ([#393](https://github.com/liftedinit/many-rs/issues/393))

## [0.1.6] - 2023-07-20

### Features

- Genesis from DB ([#401](https://github.com/liftedinit/many-rs/issues/401))

### Miscellaneous Tasks

- Add genesis-from-db to pkg ([#402](https://github.com/liftedinit/many-rs/issues/402))

## [0.1.5] - 2023-07-13

### Bug Fixes

- Revert data module serialization ([#216](https://github.com/liftedinit/many-rs/issues/216)) ([#399](https://github.com/liftedinit/many-rs/issues/399))
- Compute crate version ([#400](https://github.com/liftedinit/many-rs/issues/400))

### CI

- Fix macos nightly, again ([#395](https://github.com/liftedinit/many-rs/issues/395))

### Features

- Many compute ([#391](https://github.com/liftedinit/many-rs/issues/391))

### Miscellaneous Tasks

- Crate cleanup ([#396](https://github.com/liftedinit/many-rs/issues/396))

## [0.1.4] - 2023-07-06

### Bug Fixes

- Rustsec-2023-0044 ([#390](https://github.com/liftedinit/many-rs/issues/390))

### CI

- Fix macos nightly ([#394](https://github.com/liftedinit/many-rs/issues/394))

### Features

- Token creation for all ([#392](https://github.com/liftedinit/many-rs/issues/392))

## [0.1.3] - 2023-06-14

### Bug Fixes

- Don't list disabled keys ([#388](https://github.com/liftedinit/many-rs/issues/388))
- Add kvstore.list to endpoints ([#389](https://github.com/liftedinit/many-rs/issues/389))

### Features

- Update maximum key/value length ([#381](https://github.com/liftedinit/many-rs/issues/381))
- Kvstore.list ([#383](https://github.com/liftedinit/many-rs/issues/383))

### Miscellaneous Tasks

- Update dependencies ([#379](https://github.com/liftedinit/many-rs/issues/379))

## [0.1.2-rc.2] - 2023-06-02

### Bug Fixes

- Cargo boilerplate for publishing ([#348](https://github.com/liftedinit/many-rs/issues/348))
- When loading migrations, activate them properly at the height ([#360](https://github.com/liftedinit/many-rs/issues/360))
- Update dependencies ([#363](https://github.com/liftedinit/many-rs/issues/363))
- Add Application::check_tx to ensure txs are validated
- Token migration bats test ([#367](https://github.com/liftedinit/many-rs/issues/367))
- Collect bats test report on failure ([#366](https://github.com/liftedinit/many-rs/issues/366))
- Add a cache to prevent duplicated messages ([#370](https://github.com/liftedinit/many-rs/issues/370))
- Git cliff tag pattern ([#384](https://github.com/liftedinit/many-rs/issues/384))
- Release trigger pattern ([#385](https://github.com/liftedinit/many-rs/issues/385))

### CI

- Add migration files to release archive ([#349](https://github.com/liftedinit/many-rs/issues/349))
- Print test errors ([#353](https://github.com/liftedinit/many-rs/issues/353))
- Bats test reporting ([#355](https://github.com/liftedinit/many-rs/issues/355))
- Fix macos and docker nightly ([#369](https://github.com/liftedinit/many-rs/issues/369))
- Split resiliency tests by timing ([#357](https://github.com/liftedinit/many-rs/issues/357))
- Add manual release support ([#376](https://github.com/liftedinit/many-rs/issues/376))

### Features

- Allow token owner to mint/burn ([#374](https://github.com/liftedinit/many-rs/issues/374))

### Miscellaneous Tasks

- Bump dependencies ([#351](https://github.com/liftedinit/many-rs/issues/351))
- Update Bazel, `rules_rust` and Rust ([#368](https://github.com/liftedinit/many-rs/issues/368))

## [0.1.1] - 2023-03-29

### Bug Fixes

- Fix tag id ([#344](https://github.com/liftedinit/many-rs/issues/344))
- Add decoding for blockchain.response base64 tx data ([#345](https://github.com/liftedinit/many-rs/issues/345))

### Miscellaneous Tasks

- Update cliff configuration ([#347](https://github.com/liftedinit/many-rs/issues/347))

## [0.1.0] - 2023-03-24

### Bug Fixes

- Sort token metadata ([#336](https://github.com/liftedinit/many-rs/issues/336))
- Bump `cryptoki` to `0.3.1` ([#337](https://github.com/liftedinit/many-rs/issues/337))

### Build

- Release process ([#334](https://github.com/liftedinit/many-rs/issues/334))

### CI

- MacOS resource class deprecation ([#335](https://github.com/liftedinit/many-rs/issues/335))

### Miscellaneous Tasks

- Bump `cucumber` to `0.19.1` ([#340](https://github.com/liftedinit/many-rs/issues/340))
- Bump `openssl` to `0.10.48` ([#342](https://github.com/liftedinit/many-rs/issues/342))

