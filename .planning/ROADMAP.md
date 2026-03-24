# Roadmap: WAVS Improvements

## Overview

Three capability extensions to the WAVS platform — OCI component distribution, WIT-to-schema tooling, and an MCP execution interface — that position WAVS as a cryptographically verifiable upgrade path from Microsoft Wassette for AI agent developers. OCI pull ships first because it is independent and enables the rest of testing to use real registry-hosted components. WIT-to-schema ships second because MCP execution tools require generated `inputSchema` and `outputSchema` fields. MCP execution ships last and combines all three trust tiers in one phase since WAVS already has the submission pipeline.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: OCI Component Pull** - Service definitions accept `oci://` URIs; components are pulled, verified, and cached at deploy time
- [ ] **Phase 2: WIT-to-Schema Tooling** - Developer can inspect any compiled WASM component and get a JSON Schema describing its interface
- [ ] **Phase 3: MCP Execution Interface** - Deployed service components appear as callable MCP tools with three explicit trust tiers

## Phase Details

### Phase 1: OCI Component Pull
**Goal**: Developers can deploy WAVS services that reference OCI-hosted WASM components by URI, with digest-verified pull and content-addressed caching
**Depends on**: Nothing (first phase)
**Requirements**: OCI-01, OCI-02, OCI-03, OCI-04, OCI-05, OCI-06
**Success Criteria** (what must be TRUE):
  1. A `service.json` with an `oci://ghcr.io/...` component URI deploys successfully without requiring a local `.wasm` file
  2. WAVS refuses to deploy a service whose pulled component does not match the declared `@sha256:` digest
  3. Deploying the same service twice does not re-pull the component from the registry (cache hit confirmed in logs)
  4. A deploy using only a mutable tag (no `@sha256:` pin) emits a visible warning before proceeding
  5. Pulling from a private registry succeeds when credentials are provided via environment variables
**Plans**: 2 plans
Plans:
- [x] 01-01-PLAN.md — Add ComponentSource::Oci type variant and create OCI puller module
- [x] 01-02-PLAN.md — Wire OCI pull into engine, fix digest() Option callers, full integration

### Phase 2: WIT-to-Schema Tooling
**Goal**: Developers and the MCP execution layer can retrieve a machine-readable JSON Schema describing the input and output types of any compiled WASM component
**Depends on**: Phase 1
**Requirements**: SCHEMA-01, SCHEMA-02, SCHEMA-03, SCHEMA-04, SCHEMA-05
**Success Criteria** (what must be TRUE):
  1. Running `wavs wit-schema <component.wasm>` on any compiled WAVS component prints a valid JSON Schema to stdout
  2. A component whose WIT interface uses primitives (`u32`, `string`, `bool`, `option<T>`) produces a schema with correct JSON Schema type mappings
  3. A component with WIT record and enum/variant types produces a schema with `object` and `oneOf` entries including a required discriminator field
  4. WIT doc comments on functions and types appear as `description` fields in the generated schema
  5. Running the schema command twice on the same unchanged binary takes measurably less time than the first run (cache hit)
**Plans**: TBD

### Phase 3: MCP Execution Interface
**Goal**: AI agents can discover and invoke deployed WAVS service components as MCP tools, choosing an explicit trust tier per call — from raw result through cryptographically signed result to on-chain submission
**Depends on**: Phase 2
**Requirements**: EXEC-01, EXEC-02, EXEC-03, EXEC-04, EXEC-05, EXEC-06, EXEC-07, EXEC-08
**Success Criteria** (what must be TRUE):
  1. An MCP client calling `tools/list` sees one `wavs_run_` tool per deployed service workflow, with a populated `inputSchema` derived from the service's trigger type
  2. An agent calling `tools/call` with `trust_tier: "result_only"` receives the component execution output within 25 seconds or a structured timeout error
  3. An agent calling with `trust_tier: "signed_result"` receives a response envelope containing the result, operator signature, and signer public key
  4. An agent calling with `trust_tier: "on_chain"` receives a transaction hash confirming the result was submitted to the configured chain, and the call is gated by a `--exec-enabled` flag and a service-level flag in `service.json`
  5. Deploying or removing a service causes `notifications/tools/list_changed` to fire so agents discover tool changes without reconnecting
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. OCI Component Pull | 2/2 | Complete | 2026-03-24 |
| 2. WIT-to-Schema Tooling | 0/? | Not started | - |
| 3. MCP Execution Interface | 0/? | Not started | - |
