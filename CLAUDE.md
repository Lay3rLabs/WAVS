# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Project Is

WAVS (WebAssembly for Actively Validated Services) is a platform for running decentralized off-chain services with on-chain verification, built for EigenLayer AVS operators. Services are compiled to WebAssembly components and executed in a WASI sandbox (Wasmtime), with results submitted to EVM or Cosmos smart contracts.

## Commands

```bash
# Build
cargo build --workspace --locked

# Test all
cargo test --workspace --locked --all-features -- --nocapture --test-threads=4

# Run a single test
cargo test --package wavs --locked --all-features -- test_name --nocapture

# Lint
cargo clippy --all-targets --all-features -- -D warnings

# Format check
cargo fmt --all -- --check

# Benchmarks
cargo bench --package wavs --bench engine_execute
```

Tests require environment variables for credentials (see `.env.example`). The `layer-tests.toml` file controls which middleware variant (EigenLayer vs POA) is used in E2E tests.

## Architecture

### Dispatcher Model

The `wavs` binary runs an HTTP server plus a `Dispatcher` that orchestrates four subsystems via crossbeam channels:

```
TriggerManager  →  Dispatcher  →  EngineManager
                        ↓
              SubmissionManager  →  Aggregator  →  Chain
```

1. **TriggerManager** monitors blockchain events, cron jobs, and block intervals; fires `TriggerAction` to the Dispatcher.
2. **Dispatcher** (`packages/wavs/src/dispatcher/`) calls the EngineManager to execute the matching WASM component.
3. **EngineManager** (`packages/engine/`) runs the WASM component inside a Wasmtime WASI instance; returns `EngineResponse`.
4. **SubmissionManager** packages the result and sends it to the Aggregator.
5. **Aggregator** signs and submits to the chain via a `ServiceHandler` contract.

The HTTP server runs in its own thread; each subsystem is async (Tokio). The Dispatcher itself runs in a dedicated thread communicating with subsystems over bounded crossbeam channels.

### Workspace Packages

| Package | Purpose |
|---|---|
| `packages/wavs` | Main binary: HTTP API, Dispatcher, TriggerManager, SubmissionManager |
| `packages/engine` | WASM execution engine (Wasmtime), component caching, WASI bindings |
| `packages/types` | Shared domain types published to crates.io; feature-gated (`full`, `solidity-rpc`, `cosmwasm`, `signer`, `clock`) |
| `packages/cli` | Operator CLI: deploy services, upload/exec components, manage aggregators |
| `packages/utils` | Telemetry (OpenTelemetry/Jaeger), EVM/Cosmos clients, config, file storage |
| `packages/layer-tests` | E2E integration tests only (dev-dependencies); tests both EigenLayer and POA flows |
| `packages/dev-tool` | Development utilities |

### Multi-Chain Support

- **EVM**: `alloy-*` (v1.0.42) for provider, signer, and contract interaction
- **Cosmos**: `cosmwasm-*` + `layer-climb` for CosmWasm contracts
- Chain selection is runtime-configured; the Dispatcher is chain-agnostic

### WASM Component Model

- Services are WIT-defined WASM components (Component Model spec)
- `wit-bindgen` generates host/guest bindings
- `wasm-pkg-client` handles component package resolution
- Components are stored content-addressed (`CAStorage`) and LRU-cached in memory
- Example components live in `examples/components/`

### Configuration

- TOML-based via `Figment`; primary config is `wavs.toml`
- Environment variable overrides use `WAVS_` prefix
- `layer-tests.toml` selects middleware type for tests
- Docker Compose stack: Anvil (port 8545), WAVS node (port 8000), Jaeger (16686), Prometheus (9090)

### Key Conventions

- **No global state**: each `Dispatcher` instance is self-contained
- **Credentials**: managed with `zeroize`; never stored in plain strings longer than needed
- **Errors**: `thiserror` for typed errors, `anyhow` for application-level; no panics in subsystems
- **Tracing**: structured via the `tracing` crate; `#[instrument]` is used widely but `Service` is skipped from instrumentation to reduce log noise (see recent commit history)
- **Async**: single Tokio runtime per process; blocking work uses `spawn_blocking`
- **P2P**: libp2p (0.56) with gossipsub, Kademlia DHT, mDNS — optional for node operation
