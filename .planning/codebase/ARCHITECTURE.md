# Architecture

**Analysis Date:** 2026-03-17

## Pattern Overview

**Overall:** Event-driven subsystem architecture with message passing.

**Key Characteristics:**
- Dispatcher orchestrates four independent subsystems via crossbeam channels
- Each subsystem runs in its own thread with blocking channel recv loops
- WASM-based sandboxed execution via Wasmtime WASI components
- Separation between operator node logic and blockchain integration
- HTTP API server for administration and monitoring

## Layers

**HTTP API Layer:**
- Purpose: Expose administrative endpoints for service management, monitoring, debugging
- Location: `packages/wavs/src/http/`
- Contains: Axum route handlers for services, chains, configuration, health checks, logs
- Depends on: Dispatcher (synchronous direct calls), config, metrics
- Used by: Desktop app frontend, CLI tools, external operators

**Dispatcher (Orchestration):**
- Purpose: Central coordinator that receives trigger events and routes through subsystems
- Location: `packages/wavs/src/dispatcher.rs`
- Contains: Service registry, channel senders/receivers, subsystem lifecycle management
- Depends on: All four subsystems, service storage, chain configs
- Used by: HTTP server, subsystems (via channels), main.rs

**TriggerManager (Event Monitoring):**
- Purpose: Listen to blockchain events, cron schedules, block intervals, and other triggers; fire events to dispatcher
- Location: `packages/wavs/src/subsystems/trigger.rs` and `trigger/` subdirectories
- Contains: EVM stream listeners, Cosmos stream listeners, cron scheduler, event multiplexer
- Depends on: Chain configs, blockchain RPC providers, services registry
- Used by: Dispatcher (receives TriggerCommand, sends TriggerAction)

**Engine (WASM Execution):**
- Purpose: Execute operator and aggregator WASM components in sandboxed Wasmtime environments
- Location: `packages/wavs/src/subsystems/engine.rs`, `packages/engine/`
- Contains: WasmEngine wrapper, component lifecycle, WIT bindings
- Depends on: Services (for component lookup), WASI component binaries
- Used by: Dispatcher (receives EngineCommand, sends EngineResponse)

**SubmissionManager (Signing & Submission Prep):**
- Purpose: Sign operator results with derived keys and prepare signed Submission objects
- Location: `packages/wavs/src/subsystems/submission.rs`
- Contains: Signer management (HD wallet derivation), submission request routing
- Depends on: Services (for service lookup), config (mnemonic)
- Used by: Dispatcher (receives SubmissionCommand, sends to Aggregator)

**Aggregator (Multi-Operator Consensus & On-Chain Submission):**
- Purpose: Collect signatures from peer operators via P2P, execute aggregator component, submit results to blockchain
- Location: `packages/wavs/src/subsystems/aggregator.rs` and `aggregator/` subdirectories
- Contains: P2P networking (libp2p GossipSub), quorum queue management, EVM/Cosmos signing clients, transaction submission
- Depends on: Services, Engine (for aggregator execution), blockchain providers, optional P2P peers
- Used by: Dispatcher (receives AggregatorCommand), SubmissionManager (broadcasts submissions)

**Storage Layer:**
- Purpose: Persist services, signatures, quorum state, and submission history
- Location: `utils/storage/` (trait-based abstraction), `packages/wavs/` uses FileStorage
- Contains: Key-value store (services, chains), database (signatures, quorum queues), IPFS gateway integration
- Depends on: Tokio async runtime, file system or database backends
- Used by: Dispatcher, Services, Aggregator, all subsystems

**Service Registry:**
- Purpose: Track registered services, workflows, and service-to-component mappings
- Location: `packages/wavs/src/service_registry.rs`
- Contains: Service storage, restore/load logic from persistent store
- Depends on: Storage layer, service definitions
- Used by: Dispatcher, subsystems for service lookup

## Data Flow

**Operator Execution Flow (Happy Path):**

1. **Trigger Phase**: TriggerManager monitors blockchain events/cron/blocks on separate thread
   - Listens via EVM RPC calls or Cosmos websockets
   - Multiplexes streams into single trigger event sequence
   - Sends `TriggerAction` via channel to Dispatcher

2. **Dispatch Phase**: Dispatcher receives TriggerAction from TriggerManager
   - Looks up service and workflow from registry
   - Routes to Engine via `EngineCommand::ExecuteOperator`
   - Blocks waiting for `EngineResponse::Operator` from Engine

3. **Execution Phase**: Engine receives ExecuteOperator command
   - Loads WASM component from storage (indexed by ComponentDigest)
   - Instantiates Wasmtime WASI component with trigger data as input
   - Executes operator component in sandbox
   - Returns `SubmissionRequest` wrapped in EngineResponse

4. **Signing Phase**: Dispatcher receives EngineResponse and routes to SubmissionManager
   - SubmissionManager looks up or derives signer for service
   - Signs the operator response with ECDSA private key
   - Produces `Submission` object with signature + event metadata
   - Returns to Dispatcher

5. **Aggregation Phase**: Dispatcher sends Submission to Aggregator
   - Aggregator broadcasts Submission to peer operators via P2P (if configured)
   - Peers send back signed submissions over P2P topic
   - Accumulates in `QuorumQueue` indexed by `(EventId, SubmitAction)`
   - When quorum threshold reached, executes aggregator component via Engine

6. **Submission Phase**: Aggregator receives AggregatorAction from Engine
   - Routes to appropriate blockchain handler (EVM or Cosmos)
   - Signs transaction with aggregator credential
   - Submits to service handler contract on-chain
   - Records result in submission history

**Submit::None Short-Circuit:**
- If workflow's `submit` field is `None`, execution stops after step 3
- Operator component runs but no signing/submission occurs
- Useful for side-effect-only operations (e.g., posting to external APIs)

**State Management:**
- Services state: Persistent in storage (KeyValue store)
- Submission state: Quorum queues held in memory, periodically flushed to database
- Signer state: Derived on-demand from mnemonic + HD index, cached in SubmissionManager
- Component state: Stateless (read from storage on each execution)

## Key Abstractions

**TriggerAction:**
- Purpose: Represents a concrete trigger event (block interval, contract event, cron tick)
- Examples: `packages/wavs/src/subsystems/trigger.rs`
- Pattern: Enum wrapping event-specific data and metadata

**EngineCommand / EngineResponse:**
- Purpose: Communication protocol between Dispatcher and Engine subsystem
- Examples: `packages/wavs/src/subsystems/engine.rs`
- Pattern: Command/Response pair across channel boundaries

**Submission:**
- Purpose: Signed operator response ready for blockchain submission
- Examples: `packages/types/` (shared types)
- Pattern: Contains operator response + signature + event proof

**Service & Workflow:**
- Purpose: Define AVS operator logic, triggers, and submission config
- Examples: Loaded from storage via ServiceRegistry
- Pattern: Tree of Service -> Workflows -> Components, each with triggers and target contract

**CAStorage trait:**
- Purpose: Abstract storage backend (File, Database, etc.)
- Examples: `packages/wavs/` uses `FileStorage`
- Pattern: Generic trait allowing Dispatcher to work with any storage implementation

## Entry Points

**main.rs:**
- Location: `packages/wavs/src/main.rs`
- Triggers: Process start (`cargo run`)
- Responsibilities: Parse args, initialize config, setup telemetry (Jaeger/Prometheus), spawn HTTP server thread, spawn Dispatcher thread, wait for threads to finish

**HTTP Server (Axum):**
- Location: `packages/wavs/src/http/server.rs`
- Triggers: HTTP requests to `{host}:{port}`
- Responsibilities: Route requests to handlers (service management, health checks, logs), validate auth, serialize responses

**Service Addition Endpoint:**
- Location: `packages/wavs/src/http/handlers/service/add.rs`
- Triggers: POST `/service`
- Responsibilities: Validate service definition, add to registry, start listening for triggers

**Dispatcher.start():**
- Location: `packages/wavs/src/dispatcher.rs` (impl block at line 231)
- Triggers: Called from main thread after config setup
- Responsibilities: Spawn all four subsystem threads, listen for kill signal, coordinate graceful shutdown

## Error Handling

**Strategy:** Non-fatal errors are logged and execution continues; fatal errors panic or return error codes.

**Patterns:**
- Subsystem channels: Errors logged when channel send fails (peer dropped); subsystem continues
- WASM execution: Errors from component returned as part of response; logged at info level
- Blockchain RPC: Connection errors trigger retry logic with exponential backoff (handled by Alloy provider)
- Signing: Missing keys or invalid configs return SubmissionError to dispatcher, submitted as failed submission
- Storage: DBError propagated up; non-critical reads return Ok(None) if key not found

## Cross-Cutting Concerns

**Logging:** Structured logging via `tracing` crate; sent to stdout in dev, optionally to Jaeger for distributed tracing. Each subsystem includes `fields(subsys = "SubsystemName")` for filtering.

**Validation:** Input validation occurs at HTTP handler boundary. Service definitions validated against schema. Component digests verified against checksums.txt.

**Authentication:** HTTP endpoints support optional Bearer token auth via wildcard matching config (`auth_token_pattern`). Signing credentials stored in environment (mnemonic for operator, separate credentials for aggregator).

**Metrics:** Prometheus metrics collected via OpenTelemetry. DispatcherMetrics, TriggerMetrics, EngineMetrics, SubmissionMetrics, AggregatorMetrics each track subsystem-specific counters/histograms. Scraped by Prometheus on `/metrics` endpoint.

**Telemetry:** Distributed tracing via OpenTelemetry + Jaeger integration optional. Each operation spans subsystems to track full request lifecycle.

---

*Architecture analysis: 2026-03-17*
