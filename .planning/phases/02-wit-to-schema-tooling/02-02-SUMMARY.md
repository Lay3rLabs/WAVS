---
phase: 02-wit-to-schema-tooling
plan: 02
status: complete
started: 2026-03-25
completed: 2026-03-25
---

## Summary

Wired the wit-schema library into the WAVS CLI as the `wit-schema` subcommand. Developers can now run `wavs wit-schema --component <path.wasm>` to generate JSON Schema from any compiled WASM component's WIT interface.

## What Was Built

- **CLI command handler** (`packages/cli/src/command/wit_schema.rs`): Synchronous `run()` function that loads a WASM component via Wasmtime, invokes `wit_schema::generate_schema()`, and outputs JSON to stdout
- **Command enum variant** (`packages/cli/src/args.rs`): `WitSchema` variant with `--component` (required) and `--wit-path` (optional) flags
- **Early-return pattern** (`packages/cli/src/main.rs`): WitSchema handled before `CliContext::try_new()` so it works without a running WAVS node, credentials, or network configuration
- **Module re-export** (`packages/cli/src/command/mod.rs`): `pub mod wit_schema`

## Key Files

### Created
- `packages/cli/src/command/wit_schema.rs` — CLI command handler

### Modified
- `packages/cli/src/args.rs` — Added WitSchema variant to Command enum
- `packages/cli/src/command/mod.rs` — Added module re-export
- `packages/cli/src/main.rs` — Added early-return handling before CliContext
- `packages/cli/Cargo.toml` — Added wit-schema workspace dependency

## Verification Results

- `cargo check -p wavs-cli` — compiles cleanly
- echo_data.wasm — valid JSON with `exports: ["run"]`, correct type mappings
- simple_aggregator.wasm — 3 exports: `process-input`, `handle-timer-callback`, `handle-submit-callback`
- Error handling — nonexistent file exits code 1, error on stderr
- Pipe-friendly — stdout is valid JSON parseable by external tools
- No WAVS node dependency — works without .env or configured endpoints
- Human verification — approved

## Deviations

None.

## Self-Check: PASSED
