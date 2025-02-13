# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog],
and this project adheres to [Semantic Versioning].

## [unreleased]

## [v0.3.0-alpha9]

### Fixed
- Debug impl for `ByteArray` type

### Added
- wavs-types bump, includes new `ByteArray` type 

## [v0.3.0-alpha8]

### Added
- wavs-types bump, includes new `ByteArray` type 

### Changed
- Eth event hash in trigger is now serialized as hex-encoded string (via `ByteArray` type)

## [v0.3.0-alpha7]

### Added
- Supports multiple services for the same trigger

### Removed
- Gets rid of HTTP test service

### Changed
- Better surfacing of component execution errors


## [v0.3.0-alpha6]

### Fixed

- `wavs-wasi-chain` uses re-exported `alloy_primitives`

### Added

- CLI `deploy-service` now supports plaintext solidity event types (no need to precompute the signature hash)

### Changed

- CLI `deploy-service` makes `--trigger` optional, inferrable from the supplied address

## [v0.3.0-alpha5]

### Added

- Published `wavs-wasi-chain` crate to https://crates.io/crates/wavs-wasi-chain
- Published `wavs-types` crate to https://crates.io/crates/wavs-types
- Published `wavs:worker` WIT to https://wa.dev/wavs:worker
- Published `@wavs/solidity` to https://www.npmjs.com/package/@wavs/solidity

### Fixed

- CLI handles relative paths properly
- Engine execution is now consistent between WAVS and CLI
- Engine now logs `stdout` / `stderr` on the host

### Changed

- Pointed various crates to registries (local crates mentioned above and `layer-climb-*`)
- Bumped all dependencies
- Refactor Solidity contracts into desirable manager/handler relationship ("the inversion")
- Consistent snake_case in API
- Stronger separation between local utils and public types (now in its own crate)
- Pinning foundry binaries in Docker
- Update wstd to 0.5.0 (gets rid of Reactor etc.)
- Solidity interface uses only primitive types (allows contracts to easily satisfy without imports) 
- Some refactoring of directories etc. (`sdk` is now only the `wit`, all Rust packages are in `packages`, all public contracts in `contracts`)

### Removed

- `layer-wasi` no longer has WIT bindings locally
- `layer-wasi` no longer has cosmwasm code (now in climb itself)
- CLI removes proprietary example service support (e.g. no longer has an `add-task` command)
- Removed unused cargo dependencies across the workspace


## [v0.3.0-alpha4]

### Added

- CLI supports "none" for submit kind
- WAVS namespaces storage only by ServiceID, not ServiceID + WorkflowID
- WAVS and CLI support raw service deployment
- Overall support for multi-workflow services (including e2e tests)

## [v0.3.0-alpha3]

### Added

- CLI writes deployed Eigenlayer Service Manager addresses into `deployments.json`
- Hex string parsing now supports optional `0x` prefix

### Changed

- CLI writes full `Service` type into `deployments.json` (previously it was a reduced, cli-specific type)
- CLI displays output on a per-command basis 
- Moved more "public" types out of `wavs` package and into `utils`
- More breaking changes to clean up API and deprecate cruft from `0.2.0`
- CLI no longer imports `wavs` or `aggregator` packages

<!-- Links -->
[keep a changelog]: https://keepachangelog.com/en/1.0.0/
[semantic versioning]: https://semver.org/spec/v2.0.0.html

<!-- Versions -->
[unreleased]: https://github.com/Lay3rLabs/WAVS/compare/v0.3.0-alpha9...HEAD
[v0.3.0-alpha9]: https://github.com/Lay3rLabs/WAVS/compare/v0.3.0-alpha8...v0.3.0-alpha9
[v0.3.0-alpha8]: https://github.com/Lay3rLabs/WAVS/compare/v0.3.0-alpha7...v0.3.0-alpha8
[v0.3.0-alpha7]: https://github.com/Lay3rLabs/WAVS/compare/v0.3.0-alpha6...v0.3.0-alpha7
[v0.3.0-alpha6]: https://github.com/Lay3rLabs/WAVS/compare/v0.3.0-alpha5...v0.3.0-alpha6
[v0.3.0-alpha5]: https://github.com/Lay3rLabs/WAVS/compare/v0.3.0-alpha4...v0.3.0-alpha5
[v0.3.0-alpha4]: https://github.com/Lay3rLabs/WAVS/compare/v0.3.0-alpha3...v0.3.0-alpha4
[v0.3.0-alpha3]: https://github.com/Lay3rLabs/WAVS/compare/v0.3.0-alpha2...v0.3.0-alpha3
