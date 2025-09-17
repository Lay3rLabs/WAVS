# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog],
and this project adheres to [Semantic Versioning].

## [unreleased]

### Added

- new alerts in both wavs and aggregator with configuration that sends alerts to Slack

### Changed

- Deprecate old variant Submit::Aggregator with evm_contracts; Aggregator component is required now
- Added body size limit to http requests (default 15MB)

## [v0.5.1]

### Added
- WASM component engine in Aggregator

### Fixed
- All endpoints take ServiceManager instead of ServiceID

## [v0.5.0]

### Added

- Upgrade services (via on-chain event when ServiceManager URI is changed)
- Add new chain on the fly
- Support `ServiceStatus::Paused`
- Pass `Service` and `WorkflowID` to Component (and updated WIT)
- Support for `wasi::keyvalue` (built-in keyvalue stores, sandboxed per-service)
- CLI: Add multiple aggregator submit
- WAVS: Support for changing services
- WAVS: get service by service ID endpoint for all added services (not just those saved by http)

### Fixed

- CI Publishing
- Register operator before running full e2e test

### Changed

- Pass Service through to Aggregator (upgrades come along for the ride)
- Workflow Aggregator config is now fully in `Submit::Aggregator`
- Deprecated `service.id` (changed to a method, internal-use only)
- Aggregator opt-in now takes a `ServiceManager`
- WAVS: changed endpoint for local service getting by hash to `/service-by-hash`
- WAVS: Local save-service and get service endpoints are by hash instead of service ID

## [v0.4.0]

### Added

- Block-based interval triggers ("every N blocks")
- Cron interval triggers ("every N seconds")
- OCI/Registry support for WASI components
- Golang support for WASI components
- Decentralized Service manifests (via on-chain contract query)
- CLI deprecated "simple service" deploy in favor of full-powered "raw service" deploy
- CLI has commands to build up a full service manifest
- Jaeger tracing
- Prometheus metrics
- Swagger UI for local endpoint discoverability
- Limit running time in components (not just fuel, but time)
- General support for private keys instead of mnemonics where allowed
- service-key WAVS endpoint (returns hd-index and key)
- test components published to wa.dev
- Middleware repo for Eigenlayer-specific code
- CI-based deploy for cargo packages
- Multi-aggregator support for services (with retry and distinct responses)


### Changed

- Service type: no more ComponentID indirection, Workflow contains all
- Service type: moved settings like fuel limit on component
- Service type: much clearer config vs. env keys ergonomics
- Service type: aggregator flow improved
- WIT: method to get config value from the service
- WIT: refactor types (allow future ordering and optional return value)
- WAVS config: moved to a single config file with sections for each process
- WAVS config: evm polling interval is now a config option
- WAVS is Eigenlayer-agnostic (no more Eigenlayer-specific code, split out to middleware repo)
- Payload sent to ServiceHandler is now an Envelope type containing EventId (and unused OrderId)
- E2E tests now run concurrently
- General repo refactoring

### Fixed

- Aggregator (many bugs, now works correctly)
- WAVS concurrency (nonce management, spawn tasks, etc.)
- AVS Keys vs. Operator Keys in WAVS (each service has its own avs key)
- Eigenlayer middleware contracts
- Client code verifies that address is a contract
- Docker building

## [v0.3.0]
- bumped `WIT`, `@wavs/solidity`, `wavs-types`, `wavs-wasi-chain` and `examples` to `0.3.0`

## [v0.3.0-rc1]

### Changed

- components now return optional results. If `Ok(None)`, the workflow gracefully exits early without submission.
  You will need to migrate WASI components such that they export `fn run(a: TriggerAction) -> Result<Option<Vec<u8>>, String>` (the option is new here)

## [v0.3.0-beta]

### Added

- released WIT 0.3.0-beta: https://wa.dev/wavs:worker
- released wavs-types 0.3.0-beta: https://crates.io/crates/wavs-types/0.3.0-beta
- released wavs-wasi-chain 0.3.0-beta: https://crates.io/crates/wavs-wasi-chain/0.3.0-beta
- released @wavs/solidity 0.3.0-beta: https://www.npmjs.com/package/@wavs/solidity/v/0.3.0-beta

### Changed

- CLI gives better errors with corrupt deployment.json files

## [v0.3.0-alpha10]

### Added

- http helpers in `wavs-wasi-chain`

### Fixed

- reports fuel consumption with correct "fuel" instead of "gas"

### Changed

- `fuel_limit` moved into per-workflow configuration instead of per-service
- unused `max_gas` field removed from `ServiceConfig` (this is a per-submission configuration)
- renamed solidity contracts from `Layer*` to `Wavs*`
- bumped `wavs-types` to `0.3.0-alpha6`
- bumped `wavs-wasi-chain` to `0.3.0-alpha5`
- bumped `@wavs/solidity` to `0.3.0-alpha3`

### Changed

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
[unreleased]: https://github.com/Lay3rLabs/WAVS/compare/v0.5.1...HEAD
[v0.5.1]: https://github.com/Lay3rLabs/WAVS/compare/v0.5.0...v0.5.1
[v0.5.0]: https://github.com/Lay3rLabs/WAVS/compare/v0.4.0...v0.5.0
[v0.4.0]: https://github.com/Lay3rLabs/WAVS/compare/v0.3.0...v0.4.0
[v0.3.0]: https://github.com/Lay3rLabs/WAVS/compare/v0.2.0...v0.3.0
[v0.2.0]: https://github.com/Lay3rLabs/WAVS/compare/v0.1.0...v0.2.0
[v0.1.0]: https://github.com/Lay3rLabs/WAVS/compare/v0.0.1...v0.1.0
