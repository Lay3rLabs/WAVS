# Architecture Research

**Domain:** WAVS platform extension — WIT-to-schema, MCP execution interface, OCI distribution
**Researched:** 2026-03-24
**Confidence:** HIGH (based on direct codebase inspection + Wassette source reading)

## Standard Architecture

### System Overview: Existing + New Components

```
┌───────────────────────────────────────────────────────────────────┐
│                         AI Agent (MCP Client)                      │
└──────────────────────┬────────────────────────────────────────────┘
                       │ MCP protocol (stdio)
┌──────────────────────▼────────────────────────────────────────────┐
│                      packages/wavs-mcp                             │
│  ┌─────────────────────────┐  ┌──────────────────────────────┐    │
│  │  Management Tools        │  │  Execution Tools (NEW)        │    │
│  │  (existing)              │  │  wavs_run_component           │    │
│  │  wavs_deploy_service      │  │  tier: result_only           │    │
│  │  wavs_upload_component    │  │  tier: signed_result         │    │
│  │  wavs_simulate_trigger    │  │  tier: on_chain              │    │
│  │  wavs_get_wit_interface   │  │                              │    │
│  └─────────────────────────┘  └──────────────┬───────────────┘    │
│  ┌───────────────────────────────────────────┤                    │
│  │  WIT Schema Tools (NEW)                    │                    │
│  │  wavs_get_component_schema                 │                    │
│  │  list_tools (dynamic, per deployed service)│                    │
│  └───────────────────────┬───────────────────┘                    │
│        WavsClient (HTTP)  │  Direct engine call (NEW path)         │
└──────────┬───────────────┴────────────────────────────────────────┘
           │ HTTP                           │ Axum handler (NEW)
           │                               ▼
┌──────────▼───────────────────────────────────────────────────────┐
│                    packages/wavs (node HTTP API)                   │
│  Existing:  GET/POST /services  GET /health  POST /dev/components  │
│  New:       POST /dev/execute/{service_id}/{workflow_id}           │
│             GET  /dev/components/{digest}/schema                   │
└──────────┬───────────────────────────────────────────────────────┘
           │ DispatcherCommand channel (crossbeam)
┌──────────▼───────────────────────────────────────────────────────┐
│                    Dispatcher (packages/wavs/src/dispatcher.rs)    │
│  Existing: add_service, store_component_bytes, TriggerManager     │
│  New: execute_direct (bypasses TriggerManager, calls engine sync) │
└──────────┬───────────────────────────────────────────────────────┘
           │ EngineCommand::ExecuteOperator
┌──────────▼───────────────────────────────────────────────────────┐
│                    Engine (packages/engine/)                       │
│  Existing: execute_operator_component, store_component_from_source │
│  New: introspect_wit (Component::component_type() + wasmparser)   │
│       OCI backend registration in WkgClient                        │
└──────────┬──────────────────────┬─────────────────────────────────┘
           │ wasmtime              │ wasm-pkg-client
┌──────────▼──────────┐  ┌───────▼────────────────────────────────┐
│  Component CA Store  │  │  packages/utils/src/wkg.rs (modified)   │
│  (digest-addressed)  │  │  Adds OCI backend: ghcr.io, docker.io   │
└─────────────────────┘  └────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Status |
|-----------|----------------|--------|
| `packages/wavs-mcp/src/server.rs` | MCP tool registry, call dispatch | Existing — extend |
| `packages/wavs-mcp/src/execution.rs` | Trust tier execution logic (NEW) | New module |
| `packages/wavs/src/http/handlers/` | HTTP request routing | Existing — add handlers |
| `packages/wavs/src/dispatcher.rs` | Orchestrates engine, channels | Existing — add `execute_direct` |
| `packages/engine/src/common/base_engine.rs` | Component load + execute | Existing — add `introspect_wit` |
| `packages/utils/src/wkg.rs` | wasm-pkg-client wrapper | Existing — add OCI config |
| `packages/wavs-mcp/src/wit_schema.rs` | WIT-to-JSON-Schema conversion (NEW) | New module |

## Feature Integration Details

### 1. WIT-to-Schema

**Question: Where does WIT introspection happen?**

WIT introspection happens in `packages/engine` (the only layer that has wasmtime in scope), exposed upward via an HTTP endpoint and consumed by `wavs-mcp`.

**Mechanism:** wasmtime 42.x provides `Component::component_type()` which returns a `types::Component` for iterating exports pre-instantiation. Combined with `wasmparser` (already a transitive dependency via wasmtime), this is the same approach as Wassette's `component2json` crate. The key function chains:

```
component.component_type().exports(engine)  →  ComponentItem::ComponentFunc
    →  ComponentFuncType { params, results }
        →  recursive type_to_json_schema() mapping
            →  serde_json::Value (JSON Schema)
```

**New code location:**
- `packages/engine/src/common/wit_schema.rs` — pure function `component_bytes_to_schema(bytes: &[u8]) -> Result<Vec<ToolSchema>>` using wasmtime + wasmparser. No service or workflow context needed — operates on raw bytes.
- `packages/wavs/src/http/handlers/service/schema.rs` — HTTP handler `GET /dev/components/{digest}/schema` that retrieves bytes from CA store, calls `component_bytes_to_schema`, returns JSON.
- `packages/wavs-mcp/src/server.rs` — `wavs_get_component_schema` tool added to the static tool list. Also `list_tools` is augmented to dynamically enumerate deployed services and emit one MCP tool per `(service_id, workflow_id)` based on the workflow's component schema.

**Why engine layer, not CLI?** The CLI runs outside the WAVS node and does not have access to the node's content-addressed store. The node already has the bytes; introspection at deploy time (or on-demand) is the correct model. The CLI could call the HTTP endpoint, but the introspection logic itself belongs in the engine package.

**Type mapping (WIT → JSON Schema):**

| WIT type | JSON Schema |
|----------|-------------|
| `bool` | `{"type": "boolean"}` |
| `u8`/`u16`/`u32`/`u64`/`s8`/`s16`/`s32`/`s64` | `{"type": "integer"}` |
| `f32`/`f64` | `{"type": "number"}` |
| `string`/`char` | `{"type": "string"}` |
| `list<T>` | `{"type": "array", "items": <T schema>}` |
| `record { field: T }` | `{"type": "object", "properties": {...}, "required": [...]}` |
| `option<T>` | `{"anyOf": [<T schema>, {"type": "null"}]}` |
| `result<T, E>` | `{"oneOf": [{"type":"object","properties":{"ok":<T>}}, {"type":"object","properties":{"err":<E>}}]}` |
| `variant` | `{"oneOf": [...tag+val objects]}` |
| `enum` | `{"type": "string", "enum": [...values]}` |
| `tuple<A, B>` | `{"type":"array","prefixItems":[<A>,<B>],"minItems":2,"maxItems":2}` |

**Note:** WAVS operator components all export a single `run(trigger-action) -> result<list<wasm-response>, string>` function. The `trigger-action` input is always the same WIT type (defined in `operator.wit`). The schema value for an MCP tool describing a WAVS workflow is therefore the `trigger-data` variant (Cron / Raw / EVM event / etc.) that the specific workflow uses — derivable from the `Trigger` enum stored in the service definition, not from WIT introspection of the component itself.

This is a key architectural insight: **the MCP tool input schema for a WAVS workflow comes from the service definition (which trigger type it uses), not from WIT introspection of the component output.** WIT-to-schema is more valuable for future custom worlds or for developer tooling (understanding what a component exports) than for the MCP execution flow's `inputSchema`.

### 2. MCP Execution Interface

**Question: How does the MCP execution interface connect to the dispatcher/engine?**

The MCP execution interface adds a new code path: **direct execution bypassing the TriggerManager and Aggregator**. The three trust tiers determine what happens *after* the engine returns.

**New HTTP endpoint:**
```
POST /dev/execute/{service_id}/{workflow_id}
Body: { "data": <TriggerData as JSON>, "trust_tier": "result_only" | "signed_result" | "on_chain" }
Response: { "result": <WasmResponse bytes as hex>, "signature"?: "...", "tx_hash"?: "..." }
```

**Trust tier data flow:**

```
MCP call: wavs_run_service(service_id, workflow_id, input, trust_tier)
    │
    ▼
wavs-mcp constructs TriggerAction { config: {service_id, workflow_id, Trigger::Manual}, data: TriggerData::Raw(input_bytes) }
    │
    ▼ POST /dev/execute/{service_id}/{workflow_id}
    │
    ▼
Dispatcher::execute_direct(trigger_action, trust_tier)
    │
    ├── Engine::execute_operator_component(service, trigger_action)
    │       → Vec<WasmResponse>
    │
    ├── trust_tier == ResultOnly:  return WasmResponse.payload as-is
    │
    ├── trust_tier == SignedResult: return payload + operator signature
    │   (sign with WAVS_SIGNING_MNEMONIC like existing submission path)
    │
    └── trust_tier == OnChain: route through existing Aggregator + Submission path
        (same as normal trigger execution, but initiated by MCP call not TriggerManager)
```

**Integration point: Dispatcher.** The new `execute_direct` method on `Dispatcher` takes a `TriggerAction`, a trust tier enum, and returns a response struct. It calls the engine synchronously (via `tokio::spawn` + `await`). For `OnChain`, it sends an `EngineCommand::ExecuteOperator` through the existing `dispatcher_to_engine_tx` channel and then queues a submission, exactly as existing trigger execution does. For `ResultOnly` and `SignedResult`, it bypasses channels and calls the engine directly since no aggregation or blockchain coordination is needed.

**Why extend wavs-mcp, not a separate MCP server?** The `WavsMcpServer` already holds `WavsClient` (HTTP client to the node). Adding execution tools to the same server keeps the agent's MCP config as a single entry. The management tools and execution tools share authentication (bearer token). A separate server would require agents to configure two MCP entries, doubling friction.

**Dynamic tool listing:** When an MCP client calls `list_tools`, `wavs-mcp` calls `GET /services` to enumerate deployed services and emits one MCP tool per workflow. Tool name format: `run_{service_name}_{workflow_id}` (snake_case, trimmed). The `inputSchema` for each tool is the `TriggerData` variant schema corresponding to that workflow's trigger type — statically generated from a match on `Trigger` enum, not dynamically from WIT introspection (see insight in section 1). The `description` field comes from the service name + workflow ID.

**Trust tier as MCP tool parameter**, not separate tools. Each `run_*` tool accepts `trust_tier: "result_only" | "signed_result" | "on_chain"` as a parameter. This matches the "dial, not binary" positioning. Agents can choose per-call.

### 3. OCI Component Distribution

**Question: Where does OCI pull/cache logic live?**

OCI pull already partially exists. The `ComponentSource::Registry` variant in `packages/types/src/service.rs` accepts a `domain: Option<String>` (e.g., `"ghcr.io"`), and `packages/utils/src/wkg.rs`'s `WkgClient` calls `wasm-pkg-client` which already has OCI backend support (`oci-client` and `oci-wasm` crate dependencies in wasm-pkg-client 0.12).

**What is missing:** The `WkgClient::new()` in `packages/utils/src/wkg.rs` only configures `warg` backends for `wa.dev` and `localhost:8090`. OCI backends require a different config entry format. The fix is to add OCI backend config entries to the TOML string in `WkgClient::new()` and/or accept a registry type discriminator.

**New code location:**
- `packages/utils/src/wkg.rs` — modify `WkgClient::new()` to detect OCI domains (anything with a `/` or matching known OCI registry hostnames like `ghcr.io`, `docker.io`, `registry-1.docker.io`) and emit `type = "oci"` config sections instead of `type = "warg"`.
- `packages/types/src/service.rs` — the `Registry` struct already supports `domain: Option<String>`. No type changes needed.

**OCI URI format in service.json** (what users write):
```json
{
  "source": {
    "registry": {
      "package": "microsoft/time-server-js",
      "digest": "sha256:abc123...",
      "domain": "ghcr.io"
    }
  }
}
```

The `oci://` prefix mentioned in the PROJECT.md requirements maps to this `ComponentSource::Registry { registry: Registry { domain: Some("ghcr.io"), ... } }` structure. The URI scheme is not a literal field value — it's a user-facing shorthand for CLI/MCP tool parsing that gets translated into the `Registry` struct.

**Cache location:** `wasm-pkg-client`'s `FileCache::global_cache_path()` handles caching automatically (platform-appropriate directory, content-addressed by digest). No new cache layer is needed. Components are cached on first pull and served from disk on subsequent deploys.

**Digest verification:** Already implemented in `WkgClient::fetch()` via `assert_eq!(fetched_digest, registry.digest)`. OCI manifests also carry their own digests that wasm-pkg-client validates internally.

## Recommended Project Structure (new files only)

```
packages/
├── engine/
│   └── src/
│       └── common/
│           └── wit_schema.rs          # WIT introspection → JSON Schema
├── utils/
│   └── src/
│       └── wkg.rs                     # MODIFIED: add OCI backend config
├── wavs/
│   └── src/
│       ├── dispatcher.rs              # MODIFIED: add execute_direct method
│       └── http/
│           └── handlers/
│               └── service/
│                   ├── execute.rs     # NEW: POST /dev/execute handler
│                   └── schema.rs      # NEW: GET /dev/components/{digest}/schema handler
└── wavs-mcp/
    └── src/
        ├── execution.rs               # NEW: trust tier logic, execute tool impl
        └── server.rs                  # MODIFIED: add dynamic tool listing, schema tool
```

### Structure Rationale

- `engine/src/common/wit_schema.rs`: Introspection is a pure function of raw WASM bytes + a wasmtime Engine. Lives in `common/` alongside `base_engine.rs`. No service or workflow context.
- `wavs/src/http/handlers/service/execute.rs`: Follows the existing handler file-per-endpoint pattern. The execute endpoint is a dev endpoint (like `/dev/components`) and only enabled when `dev_endpoints_enabled = true`.
- `wavs-mcp/src/execution.rs`: Trust tier logic isolated so `server.rs` stays readable. Pattern matches existing `chain_ops.rs` and `scaffold.rs` module split.

## Data Flow

### WIT-to-Schema Flow

```
MCP client calls wavs_get_component_schema(digest)
    │
    ▼ WavsClient: GET /dev/components/{digest}/schema
    │
    ▼ HTTP handler (schema.rs)
    │   dispatcher.get_component_bytes(digest) → Vec<u8> from CA store
    │   engine::wit_schema::component_bytes_to_schema(bytes) → Vec<ToolSchema>
    │
    ▼ JSON response: [{ name, description, inputSchema, outputSchema }]
    │
    ▼ MCP tool result returned to agent
```

### Dynamic MCP Tool Listing Flow

```
MCP client calls list_tools
    │
    ▼ WavsMcpServer::list_tools()
    │   Static tools: [existing management tools] + [wavs_get_component_schema]
    │   WavsClient: GET /services → Vec<Service>
    │   For each service.workflows.iter():
    │     trigger_type = workflow.trigger variant name
    │     input_schema = trigger_type_to_json_schema(trigger_type)  // static match
    │     emit Tool { name: "run_{service}_{workflow}", inputSchema, ... }
    │
    ▼ Full tool list returned
```

### MCP Execution Flow (trust tier: result_only)

```
MCP client calls run_{service}_{workflow}(input_bytes, trust_tier="result_only")
    │
    ▼ execution.rs: build TriggerAction { Trigger::Manual, TriggerData::Raw(input_bytes) }
    │
    ▼ WavsClient: POST /dev/execute/{service_id}/{workflow_id}
    │   Body: { data: TriggerData::Raw, trust_tier: "result_only" }
    │
    ▼ Dispatcher::execute_direct(action, TrustTier::ResultOnly)
    │   service = services.get(service_id)
    │   engine.execute_operator_component(service, action) → Vec<WasmResponse>
    │
    ▼ Response: { result: hex(payload), execution_time_ms }
    │
    ▼ MCP tool result: decoded payload
```

### MCP Execution Flow (trust tier: signed_result)

```
... same as above up to engine.execute_operator_component ...
    │
    ▼ Dispatcher signs payload using WAVS_SIGNING_MNEMONIC
    │   (existing signing infrastructure in packages/types/src/signing.rs)
    │
    ▼ Response: { result: hex(payload), signature: "0x...", signer: "0x..." }
```

### MCP Execution Flow (trust tier: on_chain)

```
... same as above up to Dispatcher::execute_direct ...
    │
    ▼ Dispatcher sends EngineCommand::ExecuteOperator to dispatcher_to_engine_tx channel
    │   (same channel used by normal trigger execution)
    │
    ▼ Engine processes, sends DispatcherCommand::EngineResponse back
    │
    ▼ Dispatcher routes through existing Aggregator + Submission path
    │
    ▼ Response: { tx_hash: "0x..." } (async — may return a job ID first)
```

### OCI Pull Flow

```
Service deploy: service.json references Registry { domain: "ghcr.io", package: "...", digest: "..." }
    │
    ▼ Dispatcher::store_components_for_service(service)
    │   EngineManager::store_components_for_service(service)
    │   WasmEngine::store_component_from_source(ComponentSource::Registry { registry })
    │
    ▼ BaseEngine::load_component_from_source
    │   ComponentSource::Registry { registry } => WkgClient::new(domain).fetch(registry)
    │
    ▼ WkgClient::new("ghcr.io")  ← MODIFIED: detects OCI domain, configures oci backend
    │   client.fetch() → downloads bytes, verifies digest
    │   storage.set_data(bytes) → stored in CA store by digest
    │
    ▼ Component ready for execution — subsequent executions hit CA store, no download
```

## Integration Points with Existing Architecture

### Existing Subsystem Touchpoints

| Existing Component | What Changes | What Stays the Same |
|-------------------|--------------|---------------------|
| `Dispatcher` | Add `execute_direct()` method + new HTTP handler routes | All channel-based subsystem communication unchanged |
| `WasmEngine` | No changes to engine itself | `execute_operator_component` called directly, same API |
| `EngineManager` | No changes | `store_components_for_service` unchanged |
| `wavs-mcp/server.rs` | Add execution tools, dynamic `list_tools`, schema tool | All existing management tools untouched |
| `WkgClient` | Add OCI backend config detection | `fetch()` API unchanged, same path for warg/OCI |
| `ComponentSource::Registry` | No changes needed | `domain: Option<String>` already supports ghcr.io |
| CA Store | No changes | Digest-addressed storage works identically for OCI bytes |

### New HTTP Endpoints

| Endpoint | Handler File | Auth Required | Notes |
|----------|-------------|---------------|-------|
| `POST /dev/execute/{service_id}/{workflow_id}` | `service/execute.rs` | Bearer token (same as `/dev/*`) | dev_endpoints_enabled gate |
| `GET /dev/components/{digest}/schema` | `service/schema.rs` | None (read-only) | dev_endpoints_enabled gate |

## Build Order

The three features have a dependency relationship that constrains implementation order:

```
Phase 1: OCI Pull  ────────────────────────────────────┐
(independent, no deps on other features)               │
                                                        │ both unblock
Phase 2a: WIT-to-Schema  ─────────────────────────┐    │ MCP execution
(no deps on OCI or MCP execution)                  │    │
                                                    ▼    ▼
Phase 2b: MCP Execution Interface  ────────────────────────
(depends on: schema for tool descriptions,
 can start with static/hardcoded schemas first)
```

**Recommended build order:**

1. **OCI pull first** — isolated change in `wkg.rs`, unlocks using OCI-hosted example components in all subsequent testing. No API surface changes. Lowest risk, highest leverage for testing.

2. **WIT-to-schema second** — new `wit_schema.rs` module in engine, new HTTP endpoint, new MCP tool. Pure addition, no existing behavior changes. Can be shipped as a standalone developer tool immediately.

3. **MCP execution interface last** — requires both: schema enables auto-generated tool descriptions, OCI enables pulling test components. Trust tier logic is the most complex new behavior; building it last means the surrounding infrastructure is stable.

**Within MCP execution, build sub-order:**
1. `ResultOnly` tier (simplest — no signing, no blockchain)
2. `SignedResult` tier (adds signing, reuses existing signing infrastructure)
3. `OnChain` tier (reuses existing aggregator + submission paths, most complex coordination)

## Anti-Patterns to Avoid

### Anti-Pattern 1: New MCP Server for Execution

**What people do:** Create a separate `wavs-mcp-exec` binary because execution "feels different" from management.

**Why it's wrong:** Agents must configure two MCP servers. The execution server needs to know about deployed services anyway, so it duplicates the `WavsClient` and service registry logic. Bearer token management doubles.

**Do this instead:** Add execution tools to the existing `WavsMcpServer`. Module-separate in `execution.rs` but exposed through the same MCP server instance.

### Anti-Pattern 2: WIT introspection at execution time

**What people do:** Call `component_bytes_to_schema()` inside the execute path to validate input against WIT types.

**Why it's wrong:** Wasmtime engine creation is expensive. WIT introspection is a compile-time/deploy-time concern, not a request-time concern. WAVS operator components all share the same `run(trigger-action)` export — there is nothing to introspect dynamically at execution time.

**Do this instead:** Generate schemas at deploy/upload time or on-demand via the schema endpoint. Cache the schema result keyed by component digest. At execution time, trust the caller or validate against the pre-generated schema.

### Anti-Pattern 3: Separate OCI cache layer

**What people do:** Add a new Redis or local cache specifically for OCI-pulled components, separate from the existing CA store.

**Why it's wrong:** The existing content-addressed store in `packages/engine/src/common/base_engine.rs` already caches by digest. `wasm-pkg-client`'s `FileCache` also handles OS-level caching. Two caches means two sources of truth and stale-entry problems.

**Do this instead:** Rely on `BaseEngine::load_component_from_source` — it checks `storage.data_exists(digest)` before downloading. OCI bytes flow through the same storage path as HTTP-downloaded or directly-uploaded bytes.

### Anti-Pattern 4: Trust tier as separate endpoint

**What people do:** Three endpoints: `/dev/execute/result`, `/dev/execute/signed`, `/dev/execute/onchain`.

**Why it's wrong:** Forces MCP tools to hardcode the tier. Prevents agents from choosing per-call. Triples the HTTP API surface for semantically similar operations.

**Do this instead:** Single endpoint `POST /dev/execute/{service_id}/{workflow_id}` with `trust_tier` in the request body. The trust tier is a parameter of the execution call, not a different kind of call.

## Confidence Assessment

| Area | Confidence | Basis |
|------|------------|-------|
| Existing architecture | HIGH | Direct code inspection of dispatcher, engine, wkg, server.rs |
| WIT introspection mechanism | HIGH | Verified wasmtime 42 `Component::component_type()` API + Wassette component2json source confirms wasmparser approach |
| OCI pull gap | HIGH | `wkg.rs` code shows only warg configs; wasm-pkg-client 0.12 confirmed to have OCI backend |
| MCP execution data flow | HIGH | Existing `simulate_trigger` tool and `EngineCommand::ExecuteOperator` provide clear integration pattern |
| Trust tier signing | MEDIUM | Signing infrastructure exists (`packages/types/src/signing.rs`) but exact call path for on-demand signing not traced |
| Dynamic tool listing performance | MEDIUM | `GET /services` on each `list_tools` call may be slow at scale; caching strategy not designed |

## Open Questions for Phase-Specific Research

1. **Signing for `signed_result` tier:** The existing signing path is driven by the Aggregator collecting multiple operator signatures. For a single-operator signed result, does the existing `SignatureKind` infrastructure support ad-hoc signing without aggregation? Check `packages/types/src/signing.rs` and submission path before implementing.

2. **`list_tools` caching:** MCP clients call `list_tools` frequently. The current implementation would call `GET /services` on every call. A short TTL cache (5s) in `WavsMcpServer` would prevent hammering the node. Design before implementation.

3. **`component_type()` API change in wasmtime 42:** The Wassette `component2json` was built against an earlier wasmtime version. Verify the specific method signature for `Component::component_type().exports(engine)` in wasmtime 42.0.1 before writing the introspection code.

4. **OCI manifest format:** `wasm-pkg-client` expects OCI artifacts packaged as `application/vnd.bytecodealliance.component.v1+wasm`. Standard Docker image layers won't work. Wassette's published components on `ghcr.io` use this format. Verify that any test components use the correct OCI artifact type.

## Sources

- Direct inspection: `packages/wavs-mcp/src/server.rs` — existing tool list and server structure
- Direct inspection: `packages/engine/src/common/base_engine.rs` — component load/store path
- Direct inspection: `packages/utils/src/wkg.rs` — WkgClient and wasm-pkg-client usage
- Direct inspection: `packages/types/src/service.rs` — ComponentSource, Registry types
- Direct inspection: `packages/wavs/src/subsystems/engine.rs` — EngineCommand/EngineResponse
- Direct inspection: `packages/wavs/src/dispatcher.rs` — Dispatcher struct, channel architecture
- Direct inspection: `wit-definitions/operator/wit/operator.wit` — WIT world contract
- wasmtime 42.x docs: `Component::component_type()` method for pre-instantiation introspection
- Wassette `component2json` source: confirms wasmparser + recursive type mapping approach
- `wasm-pkg-client` 0.12 docs: confirmed OCI backend support via `oci-client` + `oci-wasm` dependencies

---
*Architecture research for: WAVS platform — WIT-to-schema, MCP execution, OCI distribution*
*Researched: 2026-03-24*
