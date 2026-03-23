# Project Research Summary

**Project:** WAVS v1.2 Tauri App — New Feature Milestone
**Domain:** Tauri 2 desktop app — node operator tooling for decentralized off-chain computation
**Researched:** 2026-03-23
**Confidence:** HIGH

## Executive Summary

The v1.2 milestone adds four distinct feature areas to an existing, well-structured Tauri 2 + React 19 desktop app: P2P operator visibility, BLS service deployment, unified activity events, and settings UX overhaul. The research finding across all four areas is consistent — **no new npm dependencies, no new Cargo dependencies, and no architectural shifts are required**. The existing stack (Tauri commands, Zustand stores, hand-rolled Tailwind components, Viem, existing WAVS HTTP API) is sufficient. Every feature either consumes existing backend endpoints (`/p2p/status`, `/services/signer`) or requires small, scoped additions to existing Tauri command handlers and Rust event structs.

The recommended approach is incremental: start with type system updates and settings decomposition (zero risk), then build the P2P dashboard (frontend-only, highest value-to-effort ratio), then BLS service builder support (moderate scope, unblocks a critical operator workflow), and finally activity event enrichment (requires backend event schema changes, moderate risk). Each phase is self-contained and delivers visible value. The BLS key registration flow is the most complex area — it requires a new `cmd_derive_bls_pubkey` Tauri command that keeps all BLS crypto in Rust and exposes only hex strings to the frontend.

The primary risks are infrastructure-level rather than feature-level: Tauri's known event listener memory leak (Issue #13133) makes the polling architecture for the P2P dashboard a critical design decision; the settings struct deserialization is fragile across upgrades without `#[serde(default)]` on every new field; and the activity event correlation system requires a backend-emitted `event_id` to be correct rather than relying on fragile heuristic matching. Address these three risks explicitly before building the corresponding features — they are not implementation details, they are architectural prerequisites.

## Key Findings

### Recommended Stack

The existing stack is validated and complete. See `.planning/research/STACK.md` for full detail.

**Core additions (not new dependencies — new usage of existing plumbing):**
- **New Tauri commands (Rust):** `cmd_get_p2p_status`, `cmd_get_service_signer`, `cmd_get_operator_keys`, `cmd_derive_bls_pubkey` — proxy existing dispatcher methods or HTTP endpoints into the IPC layer following the `cmd_get_health_status` pattern
- **New TypeScript types:** `P2pStatus`, `SignerResponse` (already defined in `packages/types/src/http.rs`), updated `SignatureAlgorithm = 'secp256k1' | 'bls12381'`
- **New Zustand store:** `p2pStore.ts` — P2P status polling/push state, separate lifecycle from app/service stores
- **New ABI file:** `POABlsStakeRegistry.ts` — BLS-specific `updateOperatorSigningKey(bytes blsKey, bytes blsSigProof)` variant; separate from existing secp256k1 ABI since the two contracts have incompatible function signatures

BLS crypto must remain entirely in the Rust backend. The `cmd_derive_bls_pubkey` command returns hex-encoded G1 pubkey and G2 proof-of-possession; the frontend passes these as opaque byte strings to the contract via Viem. No JavaScript BLS library should be introduced.

### Expected Features

See `.planning/research/FEATURES.md` for the full table-stakes / differentiator / anti-feature breakdown.

**Must have (table stakes):**
- Connected peer count + peer list with Ed25519 peer ID display — operators need network health visibility; backend already exposes via `P2pStatus`
- P2P mode indicator (Disabled / Local / Remote) and listen address display — required for diagnosing peer connectivity issues
- Subscribed services list on P2P page — correlate hex service IDs with service names from the app store
- BLS/ECDSA algorithm selector in service builder — currently hardcoded to secp256k1; this is a blocking gap for BLS service deployment
- Post-deploy BLS key display (per-service, not global) with registration guidance card
- Settings page collapsible sections — the 942-line monolith must be decomposed before adding more sections
- Submission result status (success/fail/pending) and error display in activity cards — requires `SubmissionResult` field in `SubmissionEvent`

**Should have (differentiators):**
- One-click BLS operator registration (`registerOperator` + `updateOperatorSigningKey` from the UI) — high value, ports existing MCP tool logic to Tauri commands
- BLS key backup warning after service creation — security UX, low effort
- Registration status checker (on-chain read of `POAStakeRegistry.isRegistered`) — reduces support burden
- Export activity as CSV/JSON — low effort, genuine utility for power users
- P2P configuration editor in Settings (form over raw TOML for P2P mode/port/peers)

**Defer to v2+:**
- Per-service quorum progress visualization — requires new `/aggregator/status` endpoint; significant backend work
- Activity timeline / Gantt-style view — high effort, low operational necessity
- Real-time peer connection/disconnection notifications — requires new Tauri event from P2P callbacks
- Guided first-run wizard — valuable but not v1.2 scope
- Network topology mini-map — impressive demo, low operational value

**Explicit anti-features (do not build):**
- In-app BLS key generation — keys derive from mnemonic; separate generation UI contradicts the architecture
- Manual peer add/remove UI — peer management is automated by commonware p2p
- Transaction history / on-chain explorer — block explorer's domain
- BLS DKG / threshold UI — out of scope per PROJECT.md
- Cosmos BLS services — BLS submission is EVM-only in the current implementation

### Architecture Approach

The v1.2 features follow the existing Tauri command + Zustand store + React page architecture exactly. No structural changes. The four features add 5 new Tauri commands, 1 new Tauri event type (`P2pStatusEvent`), 1 new store (`p2pStore`), 1 new page (`Operators.tsx`), and 6 extracted settings sub-components. See `.planning/research/ARCHITECTURE.md` for complete data flow diagrams and component boundaries.

**Major components affected:**
1. **`commands.rs` (Rust backend)** — Add `cmd_get_p2p_status`, `cmd_get_service_signer`, `cmd_get_operator_keys`, `cmd_derive_bls_pubkey`; refactor into modules to manage 40+ command scale (currently 30 commands in one 1244-line file)
2. **`gui_shared/event.rs` (Rust backend)** — Add `P2pStatusEvent`; enrich `SubmissionEvent` with `SubmissionResult { success: { tx_hash, algorithm } | error: { message } }` and `event_id`
3. **`p2pStore.ts` + `Operators.tsx` (Frontend)** — New store populated by push events and on-demand command; new page showing Ed25519 identity, connected peers, subscribed services, operator keys
4. **`serviceBuilderStore.ts` + `SubmitEditor.tsx` (Frontend)** — Widen `SignatureAlgorithm` type; add algorithm selector; handle BLS-specific post-deploy key display flow
5. **`Settings.tsx` + `components/settings/` (Frontend)** — Decompose 942-line monolith into 6 section components; no behavior changes, pure structural refactor
6. **`ActivityCard.tsx` + `listeners.ts` (Frontend)** — Correlation logic using backend-emitted `event_id`; result/error/algorithm badges; merged trigger+submission display

**Key architectural pattern:** Prefer direct dispatcher access over HTTP proxy in Tauri commands — avoids HTTP server startup race conditions. P2P status uses a background Tokio task emitting `P2pStatusEvent` every 5s, not frontend polling.

### Critical Pitfalls

See `.planning/research/PITFALLS.md` for the full 14-pitfall catalog with phase mappings.

1. **Tauri event listener memory leak (CRITICAL)** — Tauri 2 has a confirmed bug (Issue #13133) where `transformCallback` accumulates without cleanup. Use `invoke()` + `setInterval` for P2P status polling (not `listen()`), or store `UnlistenFn` and call it in `useEffect` cleanup. The P2P page is a "leave open" page — failure here causes app crash after hours of operation on macOS.

2. **BLS key display without service context (CRITICAL)** — BLS keys are per-service (derived from mnemonic + HD index). Displaying a single global BLS key misleads operators into registering the wrong key. Always show BLS G1 pubkey in per-service context with a copy button (256 hex chars requires a button, not text selection), truncated display, and registration status badge.

3. **Activity correlation memory leak without EventId (CRITICAL)** — Heuristic correlation by `(serviceId, workflowId, triggerData)` leaks pending-trigger entries when submissions fail or never arrive. The correct fix is adding `event_id: String` to both `TriggerEvent` and `SubmissionEvent` in the Rust event structs. This is a backend change that must precede the frontend correlation UI.

4. **Settings migration breaks existing installations (CRITICAL)** — Every new field in the `Settings` Rust struct must have `#[serde(default)]`. Without it, upgrading from v1.1 fails to deserialize `settings.json` and shows an initialization error. Write an explicit migration function. Test the upgrade path from a v1.1 settings file before shipping.

5. **`commands.rs` handler bloat at 40+ commands (MODERATE)** — Tauri's `generate_handler![]` only accepts one invocation; refactoring `commands.rs` into modules (`commands/p2p.rs`, `commands/bls.rs`, `commands/settings.rs`) is a prerequisite to adding more commands without creating an unmaintainable 1500-line file.

## Implications for Roadmap

Based on combined research, four phases in dependency order:

### Phase 1: Foundation — Types, Settings Decomposition, and Command Modules

**Rationale:** Type changes affect every other phase. Settings decomposition is a low-risk structural prerequisite that reduces merge conflict surface for subsequent changes. Refactoring `commands.rs` into modules is required before adding 10+ new commands. Zero backend behavior changes — this phase cannot break anything.

**Delivers:**
- Extended `types/index.ts`: `P2pStatus`, `SignerResponse`, `SubmissionResult`, `SignatureAlgorithm = 'secp256k1' | 'bls12381'`
- `Settings.tsx` decomposed into 6 section components (`WalletSection`, `WavsHomeSection`, `TomlEditorSection`, `EnvVarsSection`, `McpSection`, `ResetSection`)
- `commands.rs` refactored into `commands/` module structure
- `#[serde(default)]` audit on `Settings` struct + migration function
- `serviceBuilderStore.ts` algorithm field widened (type change only, no UI yet)

**Addresses:** Settings section collapsibility (prerequisite), BLS type support (prerequisite)

**Avoids:** Pitfall 4 (settings migration), Pitfall 6 (command handler bloat), Pitfall 5 (SignatureAlgorithm type mismatch)

**Research flag:** Standard patterns — no additional research needed.

### Phase 2: P2P Operator Dashboard

**Rationale:** P2P visibility is entirely frontend after Phase 1 types are in place. `GET /p2p/status` and `GET /info` already return all needed data. Adding `cmd_get_p2p_status` follows an exact existing pattern. Highest value-to-effort ratio of any v1.2 feature. Independent of BLS and activity changes.

**Delivers:**
- `cmd_get_p2p_status` and `cmd_get_node_info` Tauri commands (direct dispatcher access)
- `cmd_get_operator_keys` Tauri command (returns signer info per service)
- `P2pStatusEvent` background emitter (5s interval from `cmd_start_wavs`)
- `p2pStore.ts` — populated by push event and on-demand command
- `Operators.tsx` — new page with Ed25519 identity, peer count/list, listen addresses, P2P mode badge, subscribed services, operator key display per service
- Header nav item + `App.tsx` route for `/operators`

**Addresses:** All P2P table-stakes features (peer count, peer ID, mode indicator, listen addresses, subscribed services)

**Avoids:** Pitfall 1 (memory leak — use `invoke` + `setInterval`, not `listen`), Pitfall 10 (peer ID labeling — `PeerIdDisplay` component distinct from `AddressDisplay`), Pitfall 14 (nav routing — add to both `App.tsx` and `Header.tsx`)

**Research flag:** Standard patterns. The push-event architecture (background tokio task + `emit_ext`) is well-documented in the existing codebase.

### Phase 3: BLS Service Deployment

**Rationale:** Depends on Phase 1 type widening. Unblocks BLS service deployment from the UI, which is currently impossible. The `cmd_get_service_signer` and `cmd_derive_bls_pubkey` Tauri commands expose existing Rust BLS functionality. The BLS `POAStakeRegistry` ABI is already in `packages/types`. This phase completes the end-to-end BLS operator workflow.

**Delivers:**
- `cmd_get_service_signer` Tauri command
- `cmd_derive_bls_pubkey` Tauri command (returns `{ g1_pubkey_hex, proof_of_possession_hex }` — all BLS crypto stays in Rust)
- `POABlsStakeRegistry.ts` ABI file (separate from secp256k1 registry)
- Algorithm selector radio in `SubmitEditor.tsx` (secp256k1 / BLS12-381)
- Post-deploy BLS key display card (per-service, truncated + copy, HD index shown)
- Registration guidance card with BLS G1 pubkey, registry address, and next steps
- `updateOperatorSigningKey(blsKey, blsSigProof)` call via Viem (BLS ABI path)
- BLS algorithm indicator badge in service list and detail views
- Cosmos-BLS guard: disable BLS option when service manager is Cosmos

**Addresses:** BLS algorithm selector (table stakes), BLS key display (table stakes), operator registration guidance (table stakes), dual key display per service (differentiator)

**Avoids:** Pitfall 2 (key context confusion — per-service display, registration status check), Pitfall 8 (IPC payload size — fetch BLS keys once on mount, not on timer), Pitfall 12 (registration state on service resume — check on-chain before enabling), Pitfall 13 (per-workflow algorithm display)

**Research flag:** The `cmd_derive_bls_pubkey` proof-of-possession computation needs verification against the exact encoding expected by `IPOAStakeRegistry.updateOperatorSigningKey`. The Rust-side logic exists in `packages/wavs-mcp/src/chain_ops.rs` — this command should be a direct port, not a reimplementation. Verify the G2 signature encoding (uncompressed vs compressed, coordinate order) before writing the command.

### Phase 4: Unified Activity Events

**Rationale:** Depends on Phase 1 types. Requires backend changes to `SubmissionEvent` (adding `SubmissionResult` and `event_id`) which touch the dispatcher's submission pipeline — moderate risk compared to earlier phases. Building this last ensures the backend change is isolated and the correlation infrastructure is solid before the UI is layered on top.

**Delivers:**
- `event_id: String` field added to both `TriggerEvent` and `SubmissionEvent` in `gui_shared/event.rs`
- `SubmissionResult` enum with `Success { tx_hash, algorithm }` and `Error { message }` variants
- Dispatcher pipeline change to propagate tx hash and algorithm through `DispatcherCommand::SubmissionConfirmed`
- Frontend correlation logic in `listeners.ts` — Map keyed by `event_id`, TTL-based cleanup (5 minutes, matching `submission_ttl_secs`), bounded to `MAX_ACTIVITY_ITEMS`
- Enhanced `ActivityCard.tsx` — result badge (green/amber/red), algorithm pill (BLS/ECDSA), tx hash with block explorer link, error message (collapsible), merged trigger+submission single-card view

**Addresses:** Submission result status (table stakes), error display in activity (table stakes), merged trigger+submission cards (table stakes), algorithm badge

**Avoids:** Pitfall 3 (correlation leak — `event_id` from backend, TTL cleanup, bounded map), Pitfall 9 (array growth — consider `Map<EventId, UnifiedEvent>` with LRU eviction instead of spread-copy array at high volume)

**Research flag:** The `DispatcherCommand::SubmissionConfirmed` change propagates through the submission pipeline. Needs review of how the submission subsystem reports results back to the dispatcher to confirm `tx_hash` and `algorithm` are available at the point `emit_ext(SubmissionEvent)` is called.

### Phase Ordering Rationale

- Phase 1 before everything: types and structure changes touch every file; doing them last creates conflicts across all other phases
- Phase 2 before Phase 3: P2P page is frontend-only and validates the Tauri command pattern before more complex BLS commands are added
- Phase 3 before Phase 4: BLS commands can be reviewed and tested independently; Phase 4's backend changes (dispatcher pipeline) are higher-risk and benefit from being isolated in the final phase
- Phase 4 last: only phase with substantive backend risk; if it slips, the other three phases are already shipped and provide value

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3 (BLS deployment):** The `cmd_derive_bls_pubkey` proof-of-possession encoding must match exactly what the BLS `IPOAStakeRegistry` contract expects. The `chain_ops.rs` MCP implementation is the reference; verify the G2 signature encoding (uncompressed vs compressed, coordinate order) before implementation.
- **Phase 4 (Unified events):** The dispatcher submission pipeline path from `SubmissionConfirmed` back through the submission subsystem needs tracing to confirm `tx_hash` availability at the event emit point.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Foundation):** Type widening, settings decomposition, and serde defaults are all well-established patterns with zero uncertainty.
- **Phase 2 (P2P dashboard):** `P2pStatus` struct, HTTP endpoint, and Tauri command pattern are all verified against the codebase. The push-event architecture mirrors existing patterns exactly.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All source files verified directly against codebase. No new dependencies. All patterns confirmed with code references to specific files and line numbers. |
| Features | HIGH | Table-stakes derived from direct codebase gap analysis (missing types, hardcoded values confirmed in `app/src/types/index.ts`). Ecosystem research confirms P2P visibility expectations. |
| Architecture | HIGH | All component boundaries and data flows verified against `commands.rs`, `listeners.ts`, `appStore.ts`, `event.rs`. Phase build order confirmed by dependency tracing. |
| Pitfalls | HIGH | Pitfalls 1, 3, 6 confirmed against Tauri GitHub issues. Pitfalls 2, 4, 5, 7-14 confirmed against direct codebase inspection. Each pitfall cites specific files and line ranges. |

**Overall confidence:** HIGH

### Gaps to Address

- **Proof-of-possession encoding:** Confirm exact byte format expected by `IPOAStakeRegistry.updateOperatorSigningKey` for the BLS G2 proof — uncompressed vs compressed, endianness, domain tag. The Rust implementation in `chain_ops.rs` is the reference but needs explicit comparison against the contract's Solidity verification logic before Phase 3 implementation.

- **Quorum progress data source:** `P2pStatus` does not include per-service quorum state. If quorum visualization is desired in v1.2, a new `/aggregator/status` endpoint is needed. Research confirms this is a separate concern from P2P connectivity. Recommend deferring to a later milestone unless there is explicit operator demand.

- **Activity store data structure:** At high throughput (10+ events/second), the current spread-copy array in `appStore.ts` will show GC pressure. A `Map<EventId, UnifiedEvent>` with LRU eviction is architecturally cleaner for Phase 4. The choice between array-with-ring-buffer and Map-with-LRU should be decided at Phase 4 start, not mid-implementation.

## Sources

### Primary (HIGH confidence — direct codebase inspection)

- `packages/types/src/http.rs` — `P2pStatus`, `SignerResponse` structs
- `packages/types/src/service.rs` — `SignatureAlgorithm` enum with `Bls12381` variant
- `packages/types/src/contracts/solidity/abi/bls/IPOAStakeRegistry.json` — BLS registry ABI
- `app/src-tauri/src/commands.rs` — existing 30-command pattern, 1244 lines
- `app/src/stores/` — all 4 Zustand stores including `appStore.ts` spread-copy pattern
- `app/src/tauri/listeners.ts` — event listener pattern, 5 event types
- `app/src/types/index.ts` — `SignatureAlgorithm = 'secp256k1'` (confirmed gap)
- `app/src/pages/Settings.tsx` — 942-line monolith (confirmed)
- `packages/gui/shared/src/event.rs` — `TriggerEvent`, `SubmissionEvent` without `event_id`
- `packages/gui/shared/src/settings.rs` — `Settings` struct with serde defaults
- `packages/wavs/src/subsystems/aggregator.rs` — `get_p2p_status()` method
- `packages/wavs-mcp/src/chain_ops.rs` — BLS operator registration reference implementation

### Secondary (HIGH confidence — confirmed Tauri/framework issues)

- [Tauri Issue #13133](https://github.com/tauri-apps/tauri/issues/13133) — `transformCallback` memory leak
- [Tauri Issue #12724](https://github.com/tauri-apps/tauri/issues/12724) — event emission memory leak
- [Tauri Issue #11447](https://github.com/tauri-apps/tauri/issues/11447) — single `invoke_handler()` constraint

### Tertiary (MEDIUM confidence — ecosystem research)

- Prysm Web UI, Lighthouse Siren, Grafana eth2 dashboards — P2P node operator dashboard feature expectations
- OWASP Key Management Cheat Sheet — key display security guidance (copy buttons for long keys, never display private keys)

---
*Research completed: 2026-03-23*
*Ready for roadmap: yes*
