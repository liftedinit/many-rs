# Changelog

All notable changes to this project will be documented in this file.

## [0.1.2-rc.1] - 2023-06-02

### Bug Fixes

- Cargo boilerplate for publishing ([#348](https://github.com/liftedinit/many-rs/issues/348))
- When loading migrations, activate them properly at the height ([#360](https://github.com/liftedinit/many-rs/issues/360))
- Update dependencies ([#363](https://github.com/liftedinit/many-rs/issues/363))
- Add Application::check_tx to ensure txs are validated
- Token migration bats test ([#367](https://github.com/liftedinit/many-rs/issues/367))
- Collect bats test report on failure ([#366](https://github.com/liftedinit/many-rs/issues/366))
- Add a cache to prevent duplicated messages ([#370](https://github.com/liftedinit/many-rs/issues/370))
- Git cliff tag pattern ([#384](https://github.com/liftedinit/many-rs/issues/384))

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

