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

1. **Trigger Manager** (`subsystems/trigger/`) — Monitors EVM and Cosmos blockchain events; routes events to registered services via cron, timer, or on-chain triggers. Uses commonware-p2p for P2P message broadcast between operators.

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
- `P2P.md` — commonware P2P networking
- `WIT_AUTHORING_NOTES.md` — Writing WIT component interfaces

<!-- GSD:project-start source:PROJECT.md -->
## Project

**WAVS**

WAVS (WebAssembly-based Actively Validated Services) is a platform for running decentralized off-chain computation anchored to blockchains. Operators run sandboxed WASM components, reach multi-operator consensus via P2P (commonware), and submit verified results on-chain. Services declare their own trigger, signature scheme (secp256k1 or BLS12-381), and submission target.

**Core Value:** Multi-operator signature aggregation over P2P must work reliably — operators broadcast signed submissions, reach quorum, and submit on-chain.

### Constraints

- **Coexistence**: secp256k1 and bls12381 work as per-service options — no breaking changes to existing services
- **Runtime**: blst signing is sync/CPU-bound — runs on blocking thread pool (spawn_blocking)
- **Hash-to-curve**: hash-to-curve matches `HashToCurve.sol` (RFC 9380, DST `BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_`)
- **Pubkey sort**: aggregator sorts signerPubkeys by keccak256(pubkey) ascending — contract enforces this
- **Reference block**: referenceBlock < current block at submission, >= block when operators registered keys
<!-- GSD:project-end -->

<!-- GSD:stack-start source:codebase/STACK.md -->
## Technology Stack

## Languages
- Rust 1.91.0 - Core WAVS node, subsystems (trigger, engine, aggregator, submission), CLI, backend services
- TypeScript 5.8.3 - Desktop app frontend (Tauri React app)
- JavaScript/JSX - Frontend React components
- Solidity - Example EVM contracts in `examples/contracts/solidity/`
- Rust (WASI components) - Example AVS components in `examples/components/`
- Rust (CosmWasm) - Example Cosmos contracts in `examples/contracts/cosmwasm/`
## Runtime
- Tokio 1.47.1 - Async runtime for the WAVS node server
- Node.js (for frontend build tooling) - Managed via pnpm
- Cargo - Rust workspace with resolver version 2
- pnpm 10.18.3 - Frontend package manager for Tauri app
- Lockfiles: `Cargo.lock` (Rust), `pnpm-lock.yaml` (frontend)
## Frameworks
- Axum 0.8.6 - HTTP API server with macros
- Tauri 2.10.2 - Desktop application framework (backend bridge)
- Wasmtime 42.0.1 - WebAssembly runtime with component-model, cache, std features
- OpenTelemetry 0.31.0 - Distributed tracing with Jaeger propagation and OTLP export
- libp2p 0.56 - P2P networking (tcp, dns, noise, yamux, gossipsub, kad, mdns, autonat)
- React 19.1.0 - UI framework
- Vite 7.0.4 - Build tool and dev server
- Tauri 2.10 (CLI) - Build and packaging for desktop
- React Router DOM 7.1.0 - Client-side routing
- Zustand 5.0.0 - State management
- TailwindCSS 3.4.0 - CSS framework
- CodeMirror 6 - Code editor component
- Criterion 0.7.0 - Benchmarking framework (Rust)
- Foundry (Forge) - Solidity contract building
- Docker (implied for CosmWasm builds via justfile)
- OpenAPI/Swagger - API documentation (utoipa 5.4.0, utoipa-swagger-ui 9.0.2)
## Key Dependencies
- alloy 1.0.42 (suite) - EVM interaction: alloy-contract, alloy-provider (with ws/pubsub), alloy-signer-local (mnemonic), alloy-sol-types, alloy-rpc-types-eth
- cosmwasm-std 3.0.2 - Cosmos smart contract library
- layer-climb 0.9.0 - Layer Labs proprietary EVM/Cosmos credential handling
- hyperswarm (from git: datrs/hyperswarm-rs) - Hypercore/Hyperswarm protocol support
- hypercore 0.14.0 - Distributed append-only logs with tokio and sparse features
- hypercore-protocol 0.6.1 - Protocol implementation
- tokio-stream 0.1 - Stream adapters for tokio
- futures 0.3.31 - Async utilities
- crossbeam 0.8.4 - Concurrent data structures (channels, epoch-based GC)
- tokio-tungstenite 0.28.0 - WebSocket implementation
- reqwest 0.12.23 - HTTP client with JSON support
- tower-http 0.6.6 - HTTP middleware (CORS, tracing)
- axum-tracing-opentelemetry 0.32.0 - OpenTelemetry integration for Axum
- serde 1.0.228 - Serialization framework with derive
- serde_json 1.0.145 - JSON handling
- bincode 2.0.1 - Binary encoding (for trigger actions)
- toml 0.9.7 - TOML parsing for config
- bip39 2.2.0 - BIP39 mnemonic support with rand
- alloy-signer-local - Local signing with mnemonic support
- secp256k1 (via libp2p) - P2P identity derivation from signing mnemonic
- chrono 0.4.42 - Date/time handling
- cron 0.15.0 - Cron expression parsing
- dashmap 6.1.0 - Concurrent hash map
- lru 0.16.1 - LRU cache for compiled WASM modules
- slotmap 1.0.7 - Sparse collections
- bimap 0.6.3 - Bidirectional map
- clap 4.5.48 - CLI argument parsing with derive and env support
- dotenvy 0.15.7 - .env file loading
- tracing 0.1.41 - Structured logging with filtering
- thiserror 2.0.17 - Error type derivation
- uuid 1.18.1 - UUID generation (v7, serde support)
- zeroize 1.8.2 - Secure memory clearing
- Viem 2.23.5 - Ethereum client (EVM interaction in browser)
- @scure/bip39 1.4.0 - BIP39 mnemonic generation
- clsx 2.1.0 - Conditional className utility
- react-virtual 3.13.18 - Virtual scrolling
## Configuration
- Configuration via `.env` file with `dotenvy` loader
- TOML config file at `./wavs.toml` (or `~/.wavs/wavs.toml`, platform-specific XDG paths)
- Search order: CLI arg `--home` → `./wavs.toml` → `~/.wavs/wavs.toml` → platform XDG paths → `/etc/wavs/wavs.toml`
- Key environment variables:
- `Cargo.toml` - Rust workspace at root with members pointing to packages
- `foundry.toml` - Forge config for Solidity contracts (via_ir=true, EigenLayer lib)
- `wavs.toml` - Comprehensive WAVS server and CLI configuration
- `wkg.toml` - WIT package management configuration
- `app/package.json` - Frontend package configuration
- `app/vite.config.ts` - Vite build config (implicit, standard Tauri React template)
- `tsconfig.json` - TypeScript configuration (implied)
- Jaeger OTLP endpoint: `http://localhost:4317` (optional, configured in wavs.toml)
- Prometheus metrics push endpoint: `http://localhost:9090` (optional, 30-second default interval)
- In-memory log buffer for `/dev/logs` endpoint (enabled when dev_endpoints_enabled=true)
## Platform Requirements
- Rust 1.91.0+ (workspace edition 2021)
- Node.js + pnpm 10.18.3 (for frontend)
- Docker (for cross-platform WASI and CosmWasm builds)
- Foundry (forge) for Solidity compilation
- Justfile support (build orchestration)
- Deployment target: Linux/macOS/Windows (via Tauri)
- EVM chain endpoints (WebSocket + HTTP for fallback): Ethereum, Sepolia, Holesky, or local Anvil (chain ID 31337)
- Cosmos chain endpoints (RPC + gRPC): Neutron, Layer, or local testnet
- IPFS gateway: `http://127.0.0.1:8080/ipfs/` (local) or `https://ipfs.io/ipfs/` (remote)
- HTTP server binding: `127.0.0.1:8041` (default port 8041 from wavs.toml)
- WASM LRU cache: 20 compiled modules (default)
- Wasmtime fuel (compute metering): Unlimited (configurable)
- Max execution time: Unlimited (configurable via max_execution_seconds)
- Max HTTP body: 15 MB (configurable)
- Max WASM response: 50 MB (configurable)
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->
## Conventions

## Naming Patterns
- Rust: `snake_case` (e.g., `dispatcher.rs`, `service_registry.rs`, `mock_config.rs`)
- TypeScript/TSX: `camelCase` or `PascalCase` for components (e.g., `App.tsx`, `appStore.ts`, `AddressDisplay.tsx`)
- Test modules: inline in Rust files under `#[cfg(test)] mod tests { ... }`
- Test files: separate files in `tests/` directory with `_tests.rs` suffix (e.g., `dispatcher_tests.rs`, `aggregator_tests.rs`)
- Rust: `snake_case` for all functions (pub and private)
- TypeScript: `camelCase` for functions and methods
- Rust: `snake_case` for all variables (let bindings, struct fields, locals)
- TypeScript: `camelCase` for variables and state
- Rust: `PascalCase` for types, traits, structs, enums
- TypeScript: `PascalCase` for interfaces, types, components
- Rust: `SCREAMING_SNAKE_CASE` for compile-time constants and module-level statics
- TypeScript: `SCREAMING_SNAKE_CASE` or `camelCase` depending on scope
## Code Style
- Rust: Enforced by `cargo fmt` (configured via workspace)
- TypeScript: No explicit linter configured, but follows standard conventions
- Rust: `cargo clippy --all-targets --all-features` with `-D warnings` (deny all warnings)
- TypeScript: Strict compiler settings (tsconfig.json)
## Import Organization
- TypeScript: Not configured - uses relative paths throughout
- Rust: Uses workspace dependencies via `Cargo.toml` references (e.g., `use wavs::...`, `use utils::...`)
## Error Handling
## Logging
- Initialization: `init_tracing_tests()` in tests
- Levels: `trace!()`, `debug!()`, `info!()`, `warn!()`, `error!()`
- Structured logging: Named fields via macro arguments
- Instrumentation: `#[instrument]` macro on functions to auto-trace entry/exit
- Configuration: Via `RUST_LOG` environment variable (e.g., `RUST_LOG=debug,wavs=debug`)
- Methods: `console.log()`, `console.warn()`, `console.error()`
- Used for initialization and error reporting
- Example: `console.warn('Failed to start WAVS:', err);`
- No structured logging framework detected
## Comments
- Multi-line documentation comments for public APIs and complex functions
- Explain the "why", not the "what" (code shows the "what")
- Document runtime invariants and constraints
- Three-slash doc comments (`///`) for public items
- Doc comment blocks before items they document
- Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/types/src/bytes.rs`:
- Field documentation (inline docs):
- Enum variant docs:
- JSDoc-style blocks (/** ... */) for exported functions
- Single-line comments for inline explanation
- Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/app/src/utils/error.ts`:
- Explain complex logic or non-obvious decisions
- Example from `/Users/jacobhartnell/Dev/WAVS/WAVS/packages/wavs/src/dispatcher.rs`:
## Function Design
- Rust: Functions are generally 10-50 lines, larger functions broken into smaller helpers
- TypeScript: React components 30-100 lines, hooks/utils 5-30 lines
- Rust: Use struct types for functions with many parameters
- TypeScript: Object destructuring for multiple parameters in components
- Rust: Explicit `Result<T, E>` types for fallible operations
- TypeScript: Explicit return types on public functions, implicit on internal helpers
## Module Design
- Rust: Use `pub mod` for public modules, re-export important items at crate root
- TypeScript: Named exports for all functions/types, use `export { }` at end of files
- Rust: Minimal re-exports; subsystems define their own `mod.rs` or `lib.rs`
- TypeScript: Component directories have `index.ts` that re-exports (rarely used)
- Rust: Explicit `pub` on items meant for external use, private by default
- TypeScript: All exports are public by convention; no `private` keyword on module level
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->
## Architecture

## Pattern Overview
- Dispatcher orchestrates four independent subsystems via crossbeam channels
- Each subsystem runs in its own thread with blocking channel recv loops
- WASM-based sandboxed execution via Wasmtime WASI components
- Separation between operator node logic and blockchain integration
- HTTP API server for administration and monitoring
## Layers
- Purpose: Expose administrative endpoints for service management, monitoring, debugging
- Location: `packages/wavs/src/http/`
- Contains: Axum route handlers for services, chains, configuration, health checks, logs
- Depends on: Dispatcher (synchronous direct calls), config, metrics
- Used by: Desktop app frontend, CLI tools, external operators
- Purpose: Central coordinator that receives trigger events and routes through subsystems
- Location: `packages/wavs/src/dispatcher.rs`
- Contains: Service registry, channel senders/receivers, subsystem lifecycle management
- Depends on: All four subsystems, service storage, chain configs
- Used by: HTTP server, subsystems (via channels), main.rs
- Purpose: Listen to blockchain events, cron schedules, block intervals, and other triggers; fire events to dispatcher
- Location: `packages/wavs/src/subsystems/trigger.rs` and `trigger/` subdirectories
- Contains: EVM stream listeners, Cosmos stream listeners, cron scheduler, event multiplexer
- Depends on: Chain configs, blockchain RPC providers, services registry
- Used by: Dispatcher (receives TriggerCommand, sends TriggerAction)
- Purpose: Execute operator and aggregator WASM components in sandboxed Wasmtime environments
- Location: `packages/wavs/src/subsystems/engine.rs`, `packages/engine/`
- Contains: WasmEngine wrapper, component lifecycle, WIT bindings
- Depends on: Services (for component lookup), WASI component binaries
- Used by: Dispatcher (receives EngineCommand, sends EngineResponse)
- Purpose: Sign operator results with derived keys and prepare signed Submission objects
- Location: `packages/wavs/src/subsystems/submission.rs`
- Contains: Signer management (HD wallet derivation), submission request routing
- Depends on: Services (for service lookup), config (mnemonic)
- Used by: Dispatcher (receives SubmissionCommand, sends to Aggregator)
- Purpose: Collect signatures from peer operators via P2P, execute aggregator component, submit results to blockchain
- Location: `packages/wavs/src/subsystems/aggregator.rs` and `aggregator/` subdirectories
- Contains: P2P networking (libp2p GossipSub), quorum queue management, EVM/Cosmos signing clients, transaction submission
- Depends on: Services, Engine (for aggregator execution), blockchain providers, optional P2P peers
- Used by: Dispatcher (receives AggregatorCommand), SubmissionManager (broadcasts submissions)
- Purpose: Persist services, signatures, quorum state, and submission history
- Location: `utils/storage/` (trait-based abstraction), `packages/wavs/` uses FileStorage
- Contains: Key-value store (services, chains), database (signatures, quorum queues), IPFS gateway integration
- Depends on: Tokio async runtime, file system or database backends
- Used by: Dispatcher, Services, Aggregator, all subsystems
- Purpose: Track registered services, workflows, and service-to-component mappings
- Location: `packages/wavs/src/service_registry.rs`
- Contains: Service storage, restore/load logic from persistent store
- Depends on: Storage layer, service definitions
- Used by: Dispatcher, subsystems for service lookup
## Data Flow
- If workflow's `submit` field is `None`, execution stops after step 3
- Operator component runs but no signing/submission occurs
- Useful for side-effect-only operations (e.g., posting to external APIs)
- Services state: Persistent in storage (KeyValue store)
- Submission state: Quorum queues held in memory, periodically flushed to database
- Signer state: Derived on-demand from mnemonic + HD index, cached in SubmissionManager
- Component state: Stateless (read from storage on each execution)
## Key Abstractions
- Purpose: Represents a concrete trigger event (block interval, contract event, cron tick)
- Examples: `packages/wavs/src/subsystems/trigger.rs`
- Pattern: Enum wrapping event-specific data and metadata
- Purpose: Communication protocol between Dispatcher and Engine subsystem
- Examples: `packages/wavs/src/subsystems/engine.rs`
- Pattern: Command/Response pair across channel boundaries
- Purpose: Signed operator response ready for blockchain submission
- Examples: `packages/types/` (shared types)
- Pattern: Contains operator response + signature + event proof
- Purpose: Define AVS operator logic, triggers, and submission config
- Examples: Loaded from storage via ServiceRegistry
- Pattern: Tree of Service -> Workflows -> Components, each with triggers and target contract
- Purpose: Abstract storage backend (File, Database, etc.)
- Examples: `packages/wavs/` uses `FileStorage`
- Pattern: Generic trait allowing Dispatcher to work with any storage implementation
## Entry Points
- Location: `packages/wavs/src/main.rs`
- Triggers: Process start (`cargo run`)
- Responsibilities: Parse args, initialize config, setup telemetry (Jaeger/Prometheus), spawn HTTP server thread, spawn Dispatcher thread, wait for threads to finish
- Location: `packages/wavs/src/http/server.rs`
- Triggers: HTTP requests to `{host}:{port}`
- Responsibilities: Route requests to handlers (service management, health checks, logs), validate auth, serialize responses
- Location: `packages/wavs/src/http/handlers/service/add.rs`
- Triggers: POST `/service`
- Responsibilities: Validate service definition, add to registry, start listening for triggers
- Location: `packages/wavs/src/dispatcher.rs` (impl block at line 231)
- Triggers: Called from main thread after config setup
- Responsibilities: Spawn all four subsystem threads, listen for kill signal, coordinate graceful shutdown
## Error Handling
- Subsystem channels: Errors logged when channel send fails (peer dropped); subsystem continues
- WASM execution: Errors from component returned as part of response; logged at info level
- Blockchain RPC: Connection errors trigger retry logic with exponential backoff (handled by Alloy provider)
- Signing: Missing keys or invalid configs return SubmissionError to dispatcher, submitted as failed submission
- Storage: DBError propagated up; non-critical reads return Ok(None) if key not found
## Cross-Cutting Concerns
<!-- GSD:architecture-end -->

<!-- GSD:workflow-start source:GSD defaults -->
## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:
- `/gsd:quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd:debug` for investigation and bug fixing
- `/gsd:execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->
## Developer Profile

> Profile not yet configured. Run `/gsd:profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
