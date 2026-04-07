# Roadmap: WAVS Improvements

## Overview

Three capability extensions to the WAVS platform — OCI component distribution, WIT-to-schema tooling, and an MCP execution interface — that position WAVS as a cryptographically verifiable upgrade path from Microsoft Wassette for AI agent developers. OCI pull ships first because it is independent and enables the rest of testing to use real registry-hosted components. WIT-to-schema ships second because MCP execution tools require generated `inputSchema` and `outputSchema` fields. MCP execution ships last and combines all three trust tiers in one phase since WAVS already has the submission pipeline.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: OCI Component Pull** - Service definitions accept `oci://` URIs; components are pulled, verified, and cached at deploy time
- [x] **Phase 2: WIT-to-Schema Tooling** - Developer can inspect any compiled WASM component and get a JSON Schema describing its interface
- [ ] **Phase 3: MCP Execution Interface** - Deployed service components appear as callable MCP tools with three explicit trust tiers
- [x] **Phase 4: Rust Event Foundation** - Correlation IDs on trigger/submission events and submission failure surfacing to the GUI
- [ ] **Phase 5: Settings Decomposition** - Settings page restructured into sidebar-navigated layout with isolated section components
- [ ] **Phase 6: Unified Activity Frontend** - Activity feed displays triggers and submissions as nested parent-child events with error surfacing

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
**Plans**: 2 plans
Plans:
- [x] 02-01-PLAN.md — Create wit-schema library crate with core type conversion, traversal, cache, and doc enrichment
- [x] 02-02-PLAN.md — Wire CLI command into wavs-cli, end-to-end verification with real components

### Phase 3: MCP Execution Interface
**Goal**: AI agents can discover and invoke deployed WAVS service components as MCP tools, choosing an explicit trust tier per call — from raw result through cryptographically signed result to on-chain submission
**Depends on**: Phase 2
**Requirements**: EXEC-01, EXEC-02, EXEC-03, EXEC-04, EXEC-05, EXEC-06, EXEC-07, EXEC-08
**Success Criteria** (what must be TRUE):
  1. An MCP client calling `tools/list` sees one `wavs_exec_` tool per deployed service workflow, with a populated `inputSchema` including trust_tier and timeout_ms parameters
  2. An agent calling `tools/call` with `trust_tier: "result_only"` receives the component execution output within 25 seconds or a structured timeout error
  3. An agent calling with `trust_tier: "signed_result"` receives a response envelope containing the result, operator signature, and signer public key
  4. An agent calling with `trust_tier: "on_chain"` receives a gas estimate on the first call and a submission result on the confirmation call, gated by `--exec-enabled` flag and service submit config
  5. Deploying or removing a service causes `notifications/tools/list_changed` to fire so agents discover tool changes without reconnecting
**Plans**: 3 plans
Plans:
- [ ] 03-01-PLAN.md — Execution foundation: exec types, errors, schema merging, service cache, --exec-enabled flag, /dev/execute node endpoint
- [ ] 03-02-PLAN.md — Dynamic tool discovery and Tier 1 execution: list_tools merge, call_tool dispatch, timeout, notifications
- [ ] 03-03-PLAN.md — Tier 2 signed_result signing and Tier 3 on_chain two-step estimate-then-submit

### Phase 4: Rust Event Foundation
**Goal**: The WAVS backend emits a correlation ID on every trigger and submission event, and surfaces submission failures to the GUI
**Depends on**: Nothing (independent infrastructure phase)
**Requirements**: EVT-01, ERR-01
**Success Criteria** (what must be TRUE):
  1. Every TriggerEvent and SubmissionEvent reaching the desktop app includes a correlation_id that uniquely identifies the trigger execution and links a trigger to its submission
  2. When a submission fails (signing error or dispatch error), a SubmissionFailedEvent reaches the GUI with an error message and correlation_id
**Plans**: 1 plans
Plans:
- [x] 04-01-PLAN.md — Add correlation_id to TriggerAction, SubmissionFailed event path, and TypeScript type mirroring

### Phase 5: Settings Decomposition
**Goal**: The Settings page is restructured into a sidebar-navigated layout with each section extracted into an isolated component, without breaking OAuth flows or the unsaved-changes banner
**Depends on**: Phase 3 (independent of Phase 4; can run in parallel)
**Requirements**: SET-01, SET-02, SET-03, SET-04, SET-05, SET-06
**Success Criteria** (what must be TRUE):
  1. The Settings page displays a sidebar with labeled items for all sections; clicking an item shows only that section's content
  2. The currently active section is visually distinguished in the sidebar
  3. The restart / unsaved-changes banner remains visible at all times regardless of which section is selected
  4. An OAuth agent API key flow that spans a redirect-and-callback survives navigating between sidebar sections without losing its listener
  5. Each settings section (Wallet, Node, Env Vars, Agent, MCP, Reset) is an isolated component; no section directly reads another section's local state
**Plans**: 2 plans
Plans:
- [x] 05-01-PLAN.md — Create SettingsSidebar, extract Wallet/Node/Environment sections, rewrite Settings.tsx shell with sidebar layout
- [x] 05-02-PLAN.md — Extract Agent/MCP/Reset sections, finalize Settings.tsx as minimal orchestrating shell
**UI hint**: yes

### Phase 6: Unified Activity Frontend
**Goal**: The activity feed on both the Activity page and the Service detail tab displays triggers and submissions as nested parent-child events, shows inline error messages for failed submissions, and replaces the kind-filter tabs with event-appropriate filtering
**Depends on**: Phase 4
**Requirements**: EVT-02, EVT-03, EVT-04, EVT-05, ERR-02, ERR-03, ERR-04
**Success Criteria** (what must be TRUE):
  1. A trigger with a completed submission appears as a single expandable card; expanding it reveals the submission result nested underneath
  2. A trigger whose submission has not yet arrived shows a visible pending/in-flight indicator on its card
  3. A failed submission shows an error badge on the collapsed card and the full error message when expanded
  4. Failed events are never automatically removed from the activity feed; successful events follow existing retention behavior
  5. The unified event model (nested submissions, pending states, error badges) is present on both the standalone Activity page and the per-service activity tab
**Plans**: 2 plans
Plans:
- [ ] 06-01-PLAN.md — Data layer: GroupedActivityEvent type, useGroupedActivity hook, appStore eviction guard, status filter types
- [ ] 06-02-PLAN.md — UI layer: GroupedActivityCard component, ActivityFeed refactor with status tabs and grouped virtualizer
**UI hint**: yes

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. OCI Component Pull | 2/2 | Complete | 2026-03-24 |
| 2. WIT-to-Schema Tooling | 2/2 | Complete | 2026-03-25 |
| 3. MCP Execution Interface | 0/3 | In progress | - |
| 4. Rust Event Foundation | 1/1 | Complete | 2026-04-07 |
| 5. Settings Decomposition | 0/2 | Not started | - |
| 6. Unified Activity Frontend | 0/2 | Not started | - |
