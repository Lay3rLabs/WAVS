# Technology Stack

**Analysis Date:** 2026-03-17

## Languages

**Primary:**
- Rust 1.91.0 - Core WAVS node, subsystems (trigger, engine, aggregator, submission), CLI, backend services
- TypeScript 5.8.3 - Desktop app frontend (Tauri React app)
- JavaScript/JSX - Frontend React components

**Secondary:**
- Solidity - Example EVM contracts in `examples/contracts/solidity/`
- Rust (WASI components) - Example AVS components in `examples/components/`
- Rust (CosmWasm) - Example Cosmos contracts in `examples/contracts/cosmwasm/`

## Runtime

**Environment:**
- Tokio 1.47.1 - Async runtime for the WAVS node server
- Node.js (for frontend build tooling) - Managed via pnpm

**Package Manager:**
- Cargo - Rust workspace with resolver version 2
- pnpm 10.18.3 - Frontend package manager for Tauri app
- Lockfiles: `Cargo.lock` (Rust), `pnpm-lock.yaml` (frontend)

## Frameworks

**Core (Rust):**
- Axum 0.8.6 - HTTP API server with macros
- Tauri 2.10.2 - Desktop application framework (backend bridge)
- Wasmtime 42.0.1 - WebAssembly runtime with component-model, cache, std features
- OpenTelemetry 0.31.0 - Distributed tracing with Jaeger propagation and OTLP export
- libp2p 0.56 - P2P networking (tcp, dns, noise, yamux, gossipsub, kad, mdns, autonat)

**Frontend (JavaScript/TypeScript):**
- React 19.1.0 - UI framework
- Vite 7.0.4 - Build tool and dev server
- Tauri 2.10 (CLI) - Build and packaging for desktop
- React Router DOM 7.1.0 - Client-side routing
- Zustand 5.0.0 - State management
- TailwindCSS 3.4.0 - CSS framework
- CodeMirror 6 - Code editor component

**Testing:**
- Criterion 0.7.0 - Benchmarking framework (Rust)

**Build/Dev:**
- Foundry (Forge) - Solidity contract building
- Docker (implied for CosmWasm builds via justfile)
- OpenAPI/Swagger - API documentation (utoipa 5.4.0, utoipa-swagger-ui 9.0.2)

## Key Dependencies

**Blockchain Integration (Critical):**
- alloy 1.0.42 (suite) - EVM interaction: alloy-contract, alloy-provider (with ws/pubsub), alloy-signer-local (mnemonic), alloy-sol-types, alloy-rpc-types-eth
- cosmwasm-std 3.0.2 - Cosmos smart contract library
- layer-climb 0.9.0 - Layer Labs proprietary EVM/Cosmos credential handling

**Networking & P2P:**
- hyperswarm (from git: datrs/hyperswarm-rs) - Hypercore/Hyperswarm protocol support
- hypercore 0.14.0 - Distributed append-only logs with tokio and sparse features
- hypercore-protocol 0.6.1 - Protocol implementation

**Async & Concurrency:**
- tokio-stream 0.1 - Stream adapters for tokio
- futures 0.3.31 - Async utilities
- crossbeam 0.8.4 - Concurrent data structures (channels, epoch-based GC)
- tokio-tungstenite 0.28.0 - WebSocket implementation

**HTTP & Networking:**
- reqwest 0.12.23 - HTTP client with JSON support
- tower-http 0.6.6 - HTTP middleware (CORS, tracing)
- axum-tracing-opentelemetry 0.32.0 - OpenTelemetry integration for Axum

**Serialization:**
- serde 1.0.228 - Serialization framework with derive
- serde_json 1.0.145 - JSON handling
- bincode 2.0.1 - Binary encoding (for trigger actions)
- toml 0.9.7 - TOML parsing for config

**Cryptography & Signing:**
- bip39 2.2.0 - BIP39 mnemonic support with rand
- alloy-signer-local - Local signing with mnemonic support
- secp256k1 (via libp2p) - P2P identity derivation from signing mnemonic

**Time & Scheduling:**
- chrono 0.4.42 - Date/time handling
- cron 0.15.0 - Cron expression parsing

**Data Structures:**
- dashmap 6.1.0 - Concurrent hash map
- lru 0.16.1 - LRU cache for compiled WASM modules
- slotmap 1.0.7 - Sparse collections
- bimap 0.6.3 - Bidirectional map

**Utilities:**
- clap 4.5.48 - CLI argument parsing with derive and env support
- dotenvy 0.15.7 - .env file loading
- tracing 0.1.41 - Structured logging with filtering
- thiserror 2.0.17 - Error type derivation
- uuid 1.18.1 - UUID generation (v7, serde support)
- zeroize 1.8.2 - Secure memory clearing

**Frontend Utilities:**
- Viem 2.23.5 - Ethereum client (EVM interaction in browser)
- @scure/bip39 1.4.0 - BIP39 mnemonic generation
- clsx 2.1.0 - Conditional className utility
- react-virtual 3.13.18 - Virtual scrolling

## Configuration

**Environment:**
- Configuration via `.env` file with `dotenvy` loader
- TOML config file at `./wavs.toml` (or `~/.wavs/wavs.toml`, platform-specific XDG paths)
- Search order: CLI arg `--home` → `./wavs.toml` → `~/.wavs/wavs.toml` → platform XDG paths → `/etc/wavs/wavs.toml`
- Key environment variables:
  - `RUST_LOG` - Tracing filter directives (e.g., "info,wavs=debug")
  - `WAVS_SIGNING_MNEMONIC` - BIP39 mnemonic for WAVS operator key derivation
  - `WAVS_AGGREGATOR_COSMOS_CREDENTIAL` - Mnemonic for Cosmos chain signing
  - `WAVS_AGGREGATOR_EVM_CREDENTIAL` - Private key or mnemonic for EVM signing
  - `WAVS_BEARER_TOKEN` - Bearer token for protecting mutating HTTP endpoints
  - `WAVS_MCP_CHAIN_CREDENTIAL` - Credential for on-chain MCP tool operations

**Build Configuration:**
- `Cargo.toml` - Rust workspace at root with members pointing to packages
- `foundry.toml` - Forge config for Solidity contracts (via_ir=true, EigenLayer lib)
- `wavs.toml` - Comprehensive WAVS server and CLI configuration
- `wkg.toml` - WIT package management configuration
- `app/package.json` - Frontend package configuration
- `app/vite.config.ts` - Vite build config (implicit, standard Tauri React template)
- `tsconfig.json` - TypeScript configuration (implied)

**Tracing & Observability:**
- Jaeger OTLP endpoint: `http://localhost:4317` (optional, configured in wavs.toml)
- Prometheus metrics push endpoint: `http://localhost:9090` (optional, 30-second default interval)
- In-memory log buffer for `/dev/logs` endpoint (enabled when dev_endpoints_enabled=true)

## Platform Requirements

**Development:**
- Rust 1.91.0+ (workspace edition 2021)
- Node.js + pnpm 10.18.3 (for frontend)
- Docker (for cross-platform WASI and CosmWasm builds)
- Foundry (forge) for Solidity compilation
- Justfile support (build orchestration)

**Production:**
- Deployment target: Linux/macOS/Windows (via Tauri)
- EVM chain endpoints (WebSocket + HTTP for fallback): Ethereum, Sepolia, Holesky, or local Anvil (chain ID 31337)
- Cosmos chain endpoints (RPC + gRPC): Neutron, Layer, or local testnet
- IPFS gateway: `http://127.0.0.1:8080/ipfs/` (local) or `https://ipfs.io/ipfs/` (remote)
- HTTP server binding: `127.0.0.1:8041` (default port 8041 from wavs.toml)

**Compute Limits (Configurable):**
- WASM LRU cache: 20 compiled modules (default)
- Wasmtime fuel (compute metering): Unlimited (configurable)
- Max execution time: Unlimited (configurable via max_execution_seconds)
- Max HTTP body: 15 MB (configurable)
- Max WASM response: 50 MB (configurable)

---

*Stack analysis: 2026-03-17*
