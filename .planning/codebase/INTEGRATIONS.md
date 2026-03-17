# External Integrations

**Analysis Date:** 2026-03-17

## APIs & External Services

**Blockchain RPC Nodes:**
- Ethereum Mainnet / Testnets (Sepolia, Holesky)
  - Protocol: HTTP/WebSocket
  - Client: `alloy-provider` (0.42) with WS pubsub support
  - Config location: `wavs.toml` under `[default.chains.evm.{chain_id}]`
  - Credentials: Optional HTTP endpoint, required WS endpoints array
  - Example: `ws_endpoints = ["wss://ethereum-holesky-rpc.publicnode.com"]`

- Cosmos Chains (Neutron, Layer, local testnet)
  - Protocols: RPC (HTTP) + gRPC
  - Client: Layer Labs proprietary `layer-climb` (0.9.0) with address helpers
  - Config location: `wavs.toml` under `[default.chains.cosmos.{chain_id}]`
  - Credentials: `bech32_prefix`, `rpc_endpoint`, `grpc_endpoint`, `gas_price`, `gas_denom`
  - Example: Neutron pion-1: `rpc_endpoint = "https://rpc-falcron.pion-1.ntrn.tech"`

- Local Test Networks
  - Anvil (EVM) - default on chain ID 31337, `http://localhost:8545`
  - Local Cosmos - testnet RPC on `http://localhost:26657`

**ATProto Jetstream API:**
- Service: Bluesky/ATProto event stream
- Endpoint: `wss://jetstream1.us-east.bsky.network/subscribe` (default, configurable)
- Client: WebSocket subscription via tokio-tungstenite
- Purpose: Trigger AVS workflows on ATProto commits (posts, updates, deletes)
- Config key: `WAVS_jetstream_endpoint` (in wavs.toml `[wavs]` section)
- Supported triggers: `AtProtoEvent` with collection NSIDs (e.g., "app.bsky.feed.post") and action filtering (create/update/delete)
- Location: `packages/wavs/src/subsystems/trigger/` handles connection and event routing

**Hypercore/Hyperswarm P2P Network:**
- Service: Distributed append-only log and DHT discovery
- Client: `hyperswarm` (git fork from datrs/hyperswarm-rs) + `hypercore` (0.14.0)
- Purpose: Multi-operator trigger distribution and consensus coordination
- Config location: `wavs.toml` under `[wavs.p2p]`
- Modes:
  - Local mDNS: Auto-discovery on LAN, `listen_port = 9000`
  - Remote Kademlia DHT: Internet-wide discovery with bootstrap nodes
  - Disabled: Default for single-operator setups
- Features: libp2p with tcp, dns, noise encryption, yamux multiplexing, gossipsub, kad, mdns, autonat
- Location: `packages/wavs/src/subsystems/` orchestrates P2P submissions

## Data Storage

**Databases:**
- Not detected - No persistent database layer (SQLite, PostgreSQL, etc.)
- In-memory only: LRU cache for compiled WASM modules (`lru` crate, size configurable)
- State: Ephemeral in `DashMap` concurrent hash maps

**File Storage:**
- Local filesystem only (no cloud storage integration)
- Data directory: Configurable via `WAVS_DATA` or `wavs.toml` `[wavs].data` field
- Default: `/var/wavs`
- Stores: Service definitions, trigger history, compiled WASM cache
- No S3, Azure, or GCS integration

**IPFS Gateway (Content-Addressed Retrieval):**
- Service: InterPlanetary File System
- Endpoints:
  - Local: `http://127.0.0.1:8080/ipfs/` (configured in wavs.toml)
  - Remote: `https://ipfs.io/ipfs/` (fallback, configured in CLAUDE.md)
- Purpose: Fetch service definitions and WASM components by content hash
- Client: `reqwest` HTTP client with URL conversion
- Config key: `WAVS_ipfs_gateway` (in wavs.toml)
- Implementation: `packages/utils/src/service.rs` handles ipfs:// URI scheme conversion
- Fallback chain: Tries local daemon first, then configured gateway
- Location: Used by engine during component source resolution

**Caching:**
- LRU cache for compiled WASM modules
  - Size: 20 compiled modules (default, `wasm_lru_size` in wavs.toml)
  - Purpose: Avoid recompilation of frequently-used components
  - Storage: In-memory only, not persisted
  - Implementation: `packages/wavs/src/subsystems/engine/`

## Authentication & Identity

**Auth Provider:**
- Custom decentralized model (no external OAuth provider)
- BIP39 mnemonic-based derivation for all chains

**EVM Chain Signing:**
- Credential type: Private key or BIP39 mnemonic
- Env var: `WAVS_AGGREGATOR_EVM_CREDENTIAL` (for aggregator) or `WAVS_MCP_CHAIN_CREDENTIAL` (for MCP tools)
- Library: `alloy-signer-local` with mnemonic support
- Key derivation: HD wallet path from BIP39 mnemonic per service
- Multi-signature: Each service gets unique signing key derived from `WAVS_SIGNING_MNEMONIC`

**Cosmos Chain Signing:**
- Credential type: BIP39 mnemonic
- Env var: `WAVS_AGGREGATOR_COSMOS_CREDENTIAL` (aggregator) or `WAVS_CLI_COSMOS_MNEMONIC` (CLI)
- Library: `layer-climb-address` for bech32 address generation
- Key derivation: HD wallet from mnemonic (m/44'/118'/0'/0/0 standard)

**HTTP API Bearer Token:**
- Env var: `WAVS_BEARER_TOKEN` (set via env or wavs.toml)
- Purpose: Protects all mutating endpoints (POST/DELETE)
- Implementation: Axum middleware checking `Authorization: Bearer <token>` header
- Location: `packages/wavs/src/http/server.rs`
- Optional: Not required for GET endpoints

**P2P Identity:**
- Derivation: secp256k1 key derived from `WAVS_SIGNING_MNEMONIC`
- Purpose: libp2p peer ID and message signing
- Client: libp2p's secp256k1 feature

## Monitoring & Observability

**Error Tracking:**
- Not detected - No external error tracking service (Sentry, DataDog, etc.)
- Local handling: Structured errors via `thiserror`, propagated as HTTP 400/500 responses

**Logs:**
- Console output: via `tracing` crate with `tracing-subscriber`
- Tracing levels configurable: `RUST_LOG` env var or wavs.toml `log_level`
- Format: Compact, with line numbers
- In-memory buffer: Optional for `/dev/logs` endpoint when `dev_endpoints_enabled=true`
- OpenTelemetry integration: Optional via `tracing-opentelemetry` to Jaeger
- Jaeger endpoint: `http://localhost:4317` (OTLP gRPC)

**Metrics:**
- Prometheus remote-write: Optional via `opentelemetry-otlp`
- Endpoint: Configurable in wavs.toml `[default].prometheus`
- Push interval: 30 seconds (default, configurable)
- Implementation: `packages/utils/src/telemetry.rs` exports metrics via OTLP
- Metrics tracked: Trigger counts, engine execution time, aggregator consensus, submission status

**Distributed Tracing:**
- Jaeger/OpenTelemetry: Optional integration
- Endpoint: `http://localhost:4317` (OTLP gRPC)
- Propagation: Jaeger propagation via `opentelemetry-jaeger-propagator`
- Axum middleware: Automatic trace context attachment via `axum-tracing-opentelemetry`

## CI/CD & Deployment

**Hosting:**
- Deployment target: Linux, macOS, Windows (via Tauri desktop app)
- Single-node deployment: Docker-compatible (implicit from justfile)
- Multi-operator deployment: P2P mode with DHT bootstrap nodes

**CI Pipeline:**
- Not detected in codebase - Assumed external (GitHub Actions or similar)
- Build outputs:
  - Compiled Rust binary (debug/release)
  - Tauri desktop app bundle (.dmg, .exe, .deb)
  - WASM components in `examples/build/components/`
  - Solidity contracts (via Forge)

**Build Commands (from justfile):**
```bash
just lint              # Check formatting and clippy
just wasi-build-native # Native WASI component build
just solidity-build    # Forge contract build
just app-build-release # Tauri app release build
just start-dev         # Full dev stack with Jaeger + Prometheus
```

## Environment Configuration

**Required Environment Variables:**
- `WAVS_SIGNING_MNEMONIC` - BIP39 mnemonic for operator key HD derivation (default: test mnemonic)
- `WAVS_AGGREGATOR_COSMOS_CREDENTIAL` - Cosmos signing mnemonic (default: test mnemonic)
- `WAVS_AGGREGATOR_EVM_CREDENTIAL` - EVM signing key/mnemonic (default: test key)

**Optional Environment Variables:**
- `RUST_LOG` - Tracing filter (default: "info")
- `WAVS_DATA` - Data directory (default: /var/wavs)
- `WAVS_BEARER_TOKEN` - HTTP API bearer token
- `WAVS_MCP_CHAIN_CREDENTIAL` - MCP tool on-chain operations credential
- `WAVS_JETSTREAM_ENDPOINT` - ATProto WebSocket endpoint
- `WAVS_ENV_*` - External env variables accessible to WASM components (prefix required)

**Secrets Location:**
- Primary: `.env` file (local dev only, not committed)
- Recommended: `~/.wavs/wavs.toml` (user home, not committed to repo)
- Never commit: Real mnemonics, private keys, API keys
- Safe defaults: Test mnemonics in `wavs.toml` for local development

## Webhooks & Callbacks

**Incoming Webhooks:**
- Not detected - WAVS acts as subscriber, not webhook endpoint
- Event ingestion: Via blockchain RPC nodes (WebSocket subscriptions), not HTTP webhooks
- ATProto integration: Subscribes to Jetstream WebSocket, not webhook-based

**Outgoing Webhooks:**
- Not detected - Callbacks handled via on-chain transaction submission
- Submission: Aggregator sends results to service manager contracts on EVM/Cosmos
- Location: `packages/wavs/src/subsystems/submission/`
- No external HTTP callbacks or message queues

**Trigger Event Distribution (P2P):**
- Multi-operator coordination: libp2p gossipsub protocol
- Submission broadcast: Operators publish execution results to peers via DHT
- No external message queue (SQS, RabbitMQ, etc.)

## Service Registry & Component Distribution

**Component Source Resolution:**
- Method 1: Download from IPFS content-addressed URIs
  - URI scheme: `ipfs://QmHash...`
  - Gateway resolution: Configurable IPFS gateway
  - Integrity: SHA256 digest verification

- Method 2: Registry package reference
  - Registry: wa.dev (default) or custom domain (e.g., ghcr.io)
  - Package format: `<namespace>:<packagename>`
  - Version: Semver (default: latest)
  - Implementation: `wasm-pkg-client` (0.12.0)

- Method 3: Pre-deployed by digest
  - Digest: ComponentDigest (SHA256 hash)
  - Caching: LRU prevents re-download

**Service Definition Delivery:**
- Source: Service URI (http, https, or ipfs scheme)
- Fetching: IPFS gateway for content-addressed resolution
- Format: JSON service definition with workflows, components, triggers, submit targets
- Location: Loaded by dispatcher during service registration

---

*Integration audit: 2026-03-17*
