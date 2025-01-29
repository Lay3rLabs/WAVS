# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog],
and this project adheres to [Semantic Versioning].

## [unreleased]

### Added

- CLI supports "none" for submit kind

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
[unreleased]: https://github.com/Lay3rLabs/WAVS/compare/v0.3.0-alpha3...HEAD
[v0.3.0-alpha3]: https://github.com/Lay3rLabs/WAVS/compare/v0.3.0-alpha2...v0.3.0-alpha3