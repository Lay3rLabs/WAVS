# CLAUDE.md

For context on this codebase, read the `docs/` directory and the `justfile`.

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What Is WAVS

WAVS (WebAssembly-based Actively Validated Services) is a platform for running Actively Validated Services (AVS). It executes AVS logic as sandboxed WebAssembly (WASI) components, bridges blockchain events (EVM and Cosmos) with off-chain computation, and coordinates multi-operator consensus.

## Build, Lint, and Test Commands

All build automation is in `justfile`. Run `just` to see all targets.

### Rust

```bash
just lint           # Check formatting and clippy (non-mutating)
just lint-fix       # Auto-fix formatting and clippy issues
cargo build         # Debug build
cargo build --release
```

### WASI Components (WebAssembly)

```bash
just wasi-build-native [COMPONENT]   # Build WASI components natively
just wasi-build-docker [COMPONENT]   # Build in Docker (cross-platform)
just generate-checksums              # Regenerate checksums.txt
```

### Smart Contracts

```bash
just solidity-build    # Forge build for Solidity contracts
just cosmwasm-build    # Docker-based CosmWasm build
```

### Desktop App (Tauri + React)

```bash
just app-dev           # Full Tauri dev with hot reload
just app-dev-frontend  # Vite frontend dev server only
just app-build-release # Release build
just app-build-frontend # Vite build only
```

### Tests

E2E integration tests run on-chain with a live WAVS node:

```bash
just test-wavs-e2e
# or directly:
cargo test -p layer-tests
```

To run a subset of tests, edit `packages/layer-tests/layer-tests.toml` to isolate specific cases.

### Running the Stack

```bash
just start-dev           # WAVS + Jaeger + Prometheus (full dev stack)
just start-wavs-dev      # WAVS only with dev config
just start-anvil         # Local EVM testnet on :8545
just start-jaeger        # Tracing UI at http://localhost:16686
just start-prometheus    # Metrics UI at http://localhost:9090
```

Development tools for sending triggers and deploying services:
```bash
just dev-tool deploy-service --sleep-ms 10
just dev-tool send-triggers --count 1000
```

## Architecture

### Core Node (`packages/wavs/`)

The main WAVS node is a Tokio-based async server centered around a **dispatcher** (`packages/wavs/src/dispatcher.rs`) that orchestrates four subsystems via Crossbeam channels:

1. **Trigger Manager** (`subsystems/trigger/`) — Monitors EVM and Cosmos blockchain events; routes events to registered services via cron, timer, or on-chain triggers. Uses libp2p for P2P trigger distribution.

2. **Engine** (`subsystems/engine/`) — Executes WASM components in isolated Wasmtime WASI runtimes. Each AVS service runs as a sandboxed component with restricted system access.

3. **Aggregator** (`subsystems/aggregator/`) — Collects execution results from multiple operators and handles consensus before submission.

4. **Submission** (`subsystems/submission/`) — Routes verified results to on-chain contracts (EVM or Cosmos), managing signing and transaction submission.

An HTTP API server (Axum) on top handles service registration, health checks, and administration.

### Key Packages

- `packages/types/` — Shared types, WIT interfaces, contract ABIs, and generated TypeScript bindings
- `packages/cli/` — CLI for deploying services, executing components, and EigenLayer integration
- `packages/engine/` — Wasmtime wrapper and WASI component lifecycle management
- `packages/aggregator/` — Standalone aggregation service
- `packages/layer-tests/` — E2E test suite; config in `layer-tests.toml`
- `packages/dev-tool/` — Dev utilities for local testing

### Desktop App (`app/`)

Tauri 2 desktop app with a React 19 + Vite 7 frontend. The Tauri backend in `app/src-tauri/` bridges to the WAVS node. State management uses Zustand; blockchain interaction uses Viem.

### Examples

- `examples/components/` — WASI component source code (echo, kv-store, aggregator, cosmos-query, etc.)
- `examples/contracts/` — Example Solidity and CosmWasm contracts
- `examples/build/components/` — Compiled WASM output; `checksums.txt` tracks SHA256 hashes

### External Dependencies (downloaded via `just`)

```bash
just download-wit        # WIT interface definitions (wavs-wasi)
just download-solidity   # Solidity middleware contracts
just download-cosmwasm   # CosmWasm middleware contracts
```

## Environment

Copy `.env.example` to `.env`. Key variables:

```
RUST_LOG="info,wavs=debug"
WAVS_SIGNING_MNEMONIC="..."
WAVS_AGGREGATOR_EVM_CREDENTIAL="..."
WAVS_AGGREGATOR_COSMOS_CREDENTIAL="..."
```

## Documentation

Detailed docs live in `docs/`:
- `ARCHITECTURE.md` — Subsystem design details
- `LOCAL_DEV.md` — Development workflow and telemetry
- `API.md` — HTTP API reference
- `ASYNC_NOTES.md` — Async design patterns used throughout
- `P2P.md` — libp2p/Hyperswarm networking
- `WIT_AUTHORING_NOTES.md` — Writing WIT component interfaces
