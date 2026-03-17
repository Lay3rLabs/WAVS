# Codebase Structure

**Analysis Date:** 2026-03-17

## Directory Layout

```
WAVS/
├── packages/                    # Core Rust packages (node, engine, CLI, types)
│   ├── wavs/                    # Main WAVS node server
│   ├── engine/                  # WASM execution engine (Wasmtime wrapper)
│   ├── types/                   # Shared types, WIT interfaces, contract ABIs
│   ├── cli/                     # Command-line tools
│   ├── dev-tool/                # Development utilities (triggers, deployment)
│   ├── layer-tests/             # E2E integration test suite
│   ├── utils/                   # Shared utilities (storage, telemetry, config)
│   ├── aggregator/              # Standalone aggregation service
│   ├── wasi-utils/              # WASI component utilities
│   ├── wavs-mcp/                # Model Context Protocol server
│   ├── gui/                     # Desktop app shared types
│   └── version-pins/            # Version pinning utilities
├── app/                         # Desktop app (Tauri + React + Vite)
│   ├── src/                     # React frontend
│   ├── src-tauri/               # Tauri backend (Rust)
│   └── public/                  # Static assets
├── examples/                    # Example WASI components and contracts
│   ├── components/              # WASI component source code
│   ├── contracts/               # Solidity and CosmWasm smart contracts
│   └── build/                   # Compiled WASM binaries and checksums
├── wit-definitions/             # WIT interface definitions (downloaded)
├── docs/                        # Architecture and integration documentation
├── config/                      # Example configuration files
├── contracts/                   # On-chain contract build outputs
├── wasi/                        # WASI component utilities and helpers
├── wit/                         # Core WIT interfaces
├── Cargo.toml                   # Workspace root manifest
├── Cargo.lock                   # Dependency lock file
├── justfile                     # Build automation targets
└── wavs.toml                    # Example WAVS configuration
```

## Directory Purposes

**packages/wavs/:**
- Purpose: Main WAVS operator node (Tokio-based async server)
- Contains: Dispatcher, HTTP API, subsystems (trigger, engine, submission, aggregator), service registry
- Key files: `src/dispatcher.rs` (orchestration), `src/main.rs` (entry point), `src/http/server.rs` (HTTP router)
- Dependencies: All subsystems, types, utils, telemetry

**packages/engine/:**
- Purpose: WASM component execution abstraction layer
- Contains: WIT bindings code generation, Wasmtime integration, component lifecycle
- Key files: `src/worlds/` (per-component WIT targets), `src/bindings/` (generated code)

**packages/types/:**
- Purpose: Shared type definitions and contract ABIs
- Contains: Rust types for Service/Workflow/Trigger, generated TypeScript bindings, Solidity/CosmWasm contract ABIs
- Key files: Type enums, contract binding structs

**packages/utils/:**
- Purpose: Common utilities across packages
- Contains: Storage abstractions, EVM/Cosmos clients, telemetry setup, configuration parsing
- Key files: `storage/` (trait-based backend), `evm_client.rs`, `context.rs` (Tokio runtime)

**packages/cli/:**
- Purpose: Command-line tools for deploying services and running components
- Contains: Subcommands for service deployment, component execution, EigenLayer integration

**packages/dev-tool/:**
- Purpose: Development utilities for local testing
- Contains: Trigger sender, service deployment helpers, mock blockchain interactions

**packages/layer-tests/:**
- Purpose: End-to-end integration tests running on live chain
- Contains: Test scenarios configured in `layer-tests.toml`, full stack test harness

**app/:**
- Purpose: Desktop application for operators to manage WAVS node
- Contains: React UI (pages, components, hooks, stores), Tauri backend (Rust commands)
- Structure: `src/` (React), `src-tauri/` (Rust)
- Key pages: `/services` (service management), `/settings` (configuration), `/logs` (streaming logs), `/activity` (submissions)

**app/src-tauri/src/:**
- Purpose: Tauri backend for desktop app
- Contains: IPC commands to WAVS HTTP API, state management, logger
- Key files: `commands.rs` (command handlers), `state.rs` (app state), `main.rs` (window setup)

**examples/components/:**
- Purpose: Reference WASI component implementations
- Contains: Echo, KV store, aggregator, cosmos-query, price-feed examples
- Key files: Each subdirectory is a buildable WASI component

**examples/contracts/:**
- Purpose: Example smart contracts for operator opt-in and result submission
- Contains: Solidity contracts (EVM), CosmWasm contracts (Cosmos)

**examples/build/:**
- Purpose: Compiled WASM binaries
- Contains: `.wasm` files, `checksums.txt` (SHA256 hashes for integrity verification)

**docs/:**
- Purpose: Architecture and integration guides
- Key files: `ARCHITECTURE.md` (high-level design), `ASYNC_NOTES.md` (async patterns), `P2P.md` (networking), `WIT_AUTHORING_NOTES.md` (component authoring)

**wit-definitions/:**
- Purpose: WIT interface definitions for component contracts
- Contains: Downloaded WIT packages (types, operator, aggregator interfaces)
- Subdirectories: `types/`, `operator/`, `aggregator/` (each with `wit/deps/` dependencies)

**config/:**
- Purpose: Example configuration templates
- Contains: Example WAVS config TOML files for different scenarios

## Key File Locations

**Entry Points:**
- `packages/wavs/src/main.rs`: CLI argument parsing, config loading, Jaeger/Prometheus setup, server startup
- `app/src/main.tsx`: React app bootstrap, router initialization
- `app/src-tauri/src/main.rs`: Tauri window creation

**HTTP Router & Handlers:**
- `packages/wavs/src/http/server.rs`: Route registration, middleware setup, graceful shutdown
- `packages/wavs/src/http/handlers/service/add.rs`: Service registration endpoint
- `packages/wavs/src/http/handlers/service/get.rs`: Service retrieval endpoints
- `packages/wavs/src/http/handlers/config.rs`: Configuration management
- `packages/wavs/src/http/handlers/logs.rs`: Log streaming

**Core Subsystems:**
- `packages/wavs/src/dispatcher.rs`: Central orchestrator (trigger -> engine -> submission -> aggregator)
- `packages/wavs/src/subsystems/trigger.rs`: Event monitoring and trigger routing
- `packages/wavs/src/subsystems/engine.rs`: WASM execution manager
- `packages/wavs/src/subsystems/submission.rs`: Signing and submission preparation
- `packages/wavs/src/subsystems/aggregator.rs`: Peer coordination and on-chain submission

**Trigger Subtypes:**
- `packages/wavs/src/subsystems/trigger/streams/evm_stream.rs`: EVM block/log monitoring
- `packages/wavs/src/subsystems/trigger/streams/cosmos_stream.rs`: Cosmos event monitoring
- `packages/wavs/src/subsystems/trigger/schedulers/`: Cron and interval schedulers

**Storage & Persistence:**
- `utils/storage/fs.rs`: File-based storage (dev/test)
- `utils/storage/db.rs`: Database abstraction for signatures/queues
- `packages/wavs/src/service_registry.rs`: Service loading and registry

**Configuration:**
- `packages/wavs/src/config.rs`: Config struct and parsing
- `packages/wavs/src/args.rs`: CLI argument definitions

**Testing:**
- `packages/layer-tests/`: Integration test harness and scenarios
- `packages/layer-tests/layer-tests.toml`: Test configuration

## Naming Conventions

**Files:**
- Rust modules: snake_case (e.g., `dispatcher.rs`, `trigger_manager.rs`)
- Subsystem modules: descriptive (e.g., `trigger/streams/evm_stream.rs`)
- Handlers: action-based (e.g., `add.rs`, `get.rs`, `save.rs`)
- Tests: `_test.rs` suffix or `tests/` directory

**Directories:**
- Packages: lowercase with dash (e.g., `wavs`, `dev-tool`)
- Modules: lowercase snake_case (e.g., `subsystems`, `handlers`)
- Component examples: kebab-case (e.g., `kv-store`, `echo-block-interval`)

**Rust Types:**
- Structs/Enums: PascalCase (e.g., `Dispatcher`, `TriggerAction`)
- Commands/Responses: Action + suffix (e.g., `EngineCommand`, `EngineResponse`)
- Errors: `*Error` suffix (e.g., `TriggerError`, `SubmissionError`)
- Traits: Descriptive (e.g., `CAStorage`, `TauriEventEmitterExt`)

**React Components:**
- Files: PascalCase (e.g., `ServiceList.tsx`, `SettingsPage.tsx`)
- Directories: kebab-case for multi-file components (e.g., `components/service-builder/`)
- Custom hooks: `use*` prefix (e.g., `useAppStore`, `useWalletStore`)
- Pages: In `pages/` directory, mirroring route structure

## Where to Add New Code

**New Feature (Node):**
- Primary code: `packages/wavs/src/` (new module or subsystem)
- HTTP handler: `packages/wavs/src/http/handlers/{feature}/`
- Types: Add to `packages/types/`
- Tests: Co-located as `_test.rs` or in `layer-tests/`

**New Component/Module (Rust):**
- Implementation: `packages/{new_package}/src/lib.rs` or `src/mod.rs`
- Follow workspace pattern: Add to `Cargo.toml` in root
- Export via public module: `pub mod {module_name}`

**New WASI Component:**
- Source code: `examples/components/{component_name}/` (create directory)
- Must include `Cargo.toml` and WIT target definition
- Build output: `examples/build/{component_name}.wasm`
- Register digest in `checksums.txt` after building

**New React Page:**
- Page component: `app/src/pages/{page_name}.tsx` (or directory for complex pages)
- Add route in `app/src/App.tsx`
- Use existing stores (appStore, walletStore) for state
- Tauri IPC calls in `app/src/tauri/commands.ts`

**Utilities & Shared Code:**
- Common utilities: `packages/utils/` (string/math helpers), or subsystem-specific modules
- Shared React hooks: `app/src/hooks/`
- Shared React components: `app/src/components/atoms/` (basic), `app/src/components/{domain}/` (domain-specific)

## Special Directories

**target/:**
- Purpose: Build artifacts and compiled binaries (generated by Cargo)
- Generated: Yes
- Committed: No

**out/:**
- Purpose: Compiled Solidity contracts (build output from Foundry)
- Generated: Yes
- Committed: Selectively (ABIs committed, bytecode not always)

**cache/:**
- Purpose: Local development cache (IPFS downloads, component builds)
- Generated: Yes
- Committed: No

**data/:**
- Purpose: Runtime data (service state, submissions, keystore)
- Generated: During operation
- Committed: No

**wit-definitions/:**
- Purpose: Downloaded WIT packages for component development
- Generated: No (downloaded via `just download-wit`)
- Committed: No

**.planning/codebase/:**
- Purpose: Generated codebase analysis documents (ARCHITECTURE.md, STRUCTURE.md, etc.)
- Generated: Yes (by mapping tools)
- Committed: Yes

---

*Structure analysis: 2026-03-17*
