# Domain Pitfalls

**Domain:** Adding P2P dashboard, BLS deployment, unified events, and settings UX to existing Tauri 2 + React desktop app
**Researched:** 2026-03-23
**Previous research:** 2026-03-17 (P2P migration, commonware integration -- v1.0/v1.1 pitfalls)

This document covers pitfalls specific to the **v1.2 Tauri app milestone**: integrating P2P operator visibility, BLS service deployment with key registration, unified activity events, and settings page reorganization into the existing Tauri 2 desktop app.

## Critical Pitfalls

Mistakes that cause rewrites, security issues, or major UX breakage.

### Pitfall 1: P2P Status Polling Creates Memory Leak via Tauri Event Listeners

**What goes wrong:** The P2P dashboard requires real-time data about connected peers, Ed25519 identity, subscribed services, and quorum progress. The natural approach is polling the `/p2p/status` endpoint (or a new Tauri command wrapping `dispatcher.aggregator.get_p2p_status()`) on an interval. However, if the polling fires Tauri events or uses `listen()` callbacks that are not properly cleaned up, each poll cycle accumulates callbacks on the window object. Tauri 2 has a known bug where `transformCallback` saves callbacks but never deletes them ([Issue #13133](https://github.com/tauri-apps/tauri/issues/13133)). Over hours/days of P2P dashboard polling, memory grows unboundedly.

**Why it happens:** The existing codebase already has a polling pattern in `Settings.tsx` (MCP status poll every 3 seconds with `setInterval`). This pattern works fine for settings because users navigate away. But the P2P dashboard is likely a "leave it open" page -- operators will keep it visible while monitoring their node. A 3-second poll running for 8 hours = 9,600 accumulated callbacks if cleanup is wrong.

**Consequences:** Webview memory grows from ~50MB to 500MB+ over a day. On macOS, Tauri's webview process (WKWebView) eventually gets killed by the OS. The entire app crashes with no error message, and the operator loses visibility into their node at the worst possible time.

**Prevention:**
- Use `invoke()` (Tauri commands) for polling, NOT `listen()` (Tauri events) for periodic status checks. The `invoke()` pattern creates a one-shot promise that is garbage-collected. The `listen()` pattern creates a persistent callback that must be manually unlistened.
- Follow the existing `getMcpStatus()` pattern from `Settings.tsx`: `setInterval` + `invoke` + cancelled flag. This is correct.
- If using Tauri events (push model) for real-time P2P updates, store the `UnlistenFn` and call it in `useEffect` cleanup. The existing `listeners.ts` does this correctly at the app level -- but page-level listeners (P2P page mount/unmount) need the same discipline.
- Add a `MAX_P2P_HISTORY` constant (similar to `MAX_LOG_ITEMS = 5000` in `appStore.ts`) to bound any stored P2P status history.
- Consider using `document.visibilityState` to pause polling when the app is minimized/hidden.

**Detection:** Memory profiler in DevTools shows growing retained size of `window.__TAURI_INTERNALS__` callbacks. `performance.memory.usedJSHeapSize` grows linearly over time on the P2P page.

**Phase mapping:** Must be addressed in the P2P dashboard phase. The polling architecture should be designed before UI components.

### Pitfall 2: BLS Public Key Displayed in UI Creates Operator Registration Confusion

**What goes wrong:** The v1.2 milestone adds BLS key display (128-byte G1 pubkey) alongside Ed25519 peer ID and secp256k1 EVM addresses in the P2P page. Operators need their BLS G1 public key to register with the BLS service manager contract. The pitfall is displaying the wrong key, at the wrong time, in the wrong format. Specifically:

1. **Key derivation is per-service**: BLS keys are derived via HKDF-SHA256 from the mnemonic with a per-service HD index. There is no single "BLS public key" -- each service has its own. Displaying a single BLS key without service context misleads operators.
2. **128-byte uncompressed G1 format**: The BLS G1 pubkey is 128 bytes (256 hex chars) in EIP-2537 uncompressed format. This is massive compared to a 20-byte EVM address. Copy errors are extremely likely.
3. **Registration timing matters**: The `referenceBlock` constraint means operators must register their BLS key before the reference block used at submission. If the UI shows the key but the operator hasn't registered it on-chain, submissions will revert with signature verification failures.

**Why it happens:** The existing `SignerResponse` enum in `packages/types/src/http.rs` already distinguishes `Secp256k1 { hd_index, evm_address }` from `Bls12381 { hd_index, g1_pubkey_hex }`. The `/signer` endpoint returns the right data. But the UI must make it clear which key belongs to which service, and that registration is a prerequisite -- not just information.

**Consequences:** Operator registers wrong BLS key (e.g., from a different HD index), or copies a truncated key from the UI, or does not register at all. On-chain BLS signature verification fails. The service appears broken even though the node is operating correctly. Debugging requires understanding the HD derivation path, which most operators will not.

**Prevention:**
- Display BLS keys in a **per-service context**, not globally. The P2P overview can show "BLS keys: N services" but the full key should appear on the service detail page.
- Add a copy button that copies the complete hex string (not relying on text selection of 256 characters).
- Show registration status: query the BLS service manager contract for whether the operator's G1 pubkey is registered. Display "Registered" / "Not Registered" badge.
- Add a registration action button that calls `registerOperatorWithSignature` or equivalent directly from the UI.
- Truncate display to first 8 + last 8 hex chars with "..." in between, but copy always gets the full key.
- Show the HD index alongside the key so advanced operators can verify derivation.

**Detection:** Operators report "BLS verification failed" errors in submission logs. The aggregator receives signatures but on-chain submission reverts. The error is not visible in the app because the submission event doesn't include the revert reason.

**Phase mapping:** Must be addressed in the BLS deployment phase. The key display component should be built with registration flow awareness from the start.

### Pitfall 3: Trigger-to-Submission Event Correlation Leaks Memory or Loses References

**What goes wrong:** The v1.2 milestone adds unified event cards that merge trigger events with their corresponding submission results. The current implementation stores triggers and submissions as separate `ActivityItem` entries with a monotonic `id` counter. There is no correlation ID linking a trigger to its resulting submission. Attempting to correlate them post-hoc (by matching `serviceId` + `workflowId` + time window) is fragile and produces incorrect matches under load.

**Why it happens:** Looking at the existing event system:
- `TriggerEvent` contains `action.config.service_id`, `action.config.workflow_id`, `action.data`
- `SubmissionEvent` contains `service_id`, `workflow_id`, `trigger_data`
- Both carry `trigger_data` but there is no shared correlation ID

The `EventId` (from `signing.rs`) is the canonical correlation key -- it's derived from `service_id + workflow_id + trigger_data`. But it is not currently emitted in either event. Adding it requires changes to the Rust event emitters.

**Consequences:**
1. **Without correlation**: If two triggers fire for the same service within 100ms (common for block interval triggers), the UI cannot determine which submission corresponds to which trigger. It either pairs them incorrectly or shows them as unlinked.
2. **With naive time-window matching**: A HashMap from `(serviceId, workflowId)` to "pending trigger" grows unboundedly if submissions are delayed or fail. Triggers that never receive a submission (component crash, quorum failure) remain in the pending map forever.
3. **With EventId correlation**: Works correctly, but EventId computation requires `Ripemd160(service_id + workflow_id + bincode(trigger_data))` which involves binary encoding in the frontend. Mismatching the Rust bincode serialization in TypeScript is likely.

**Prevention:**
- **Add EventId to both events on the Rust side.** Modify `TriggerEvent` and `SubmissionEvent` in `packages/gui/shared/src/event.rs` to include `event_id: String` (hex-encoded). Compute it in the dispatcher where `TriggerAction` already exists. This is a backend change, not a frontend one.
- Use `event_id` as the Map key for correlation in the Zustand store. When a trigger arrives, create an entry. When a submission arrives with the same `event_id`, merge them.
- Set a TTL (e.g., 5 minutes matching `submission_ttl_secs`) on unmatched triggers. After the TTL, mark them as "no submission" rather than holding them forever.
- Bound the correlation map to `MAX_ACTIVITY_ITEMS` (currently 2000). When the map exceeds the limit, evict oldest entries.
- Do NOT attempt bincode serialization in TypeScript. The Rust backend should provide the EventId.

**Detection:** Activity feed shows triggers and submissions that don't match (wrong pairing). Memory of the Zustand store grows over time (inspect via React DevTools or Zustand devtools middleware). Operators see "pending" triggers that never resolve.

**Phase mapping:** This is a cross-cutting concern: the backend event change should happen first (early phase), then the frontend correlation UI can be built on top.

### Pitfall 4: Settings State Migration Breaks Existing Operator Installations

**What goes wrong:** The v1.2 milestone reorganizes the Settings page and adds new fields (BLS-related settings, P2P display preferences, possibly new env var patterns). The `Settings` struct in `packages/gui/shared/src/settings.rs` is serialized to `settings.json` in the app config directory. Adding new fields without `#[serde(default)]` breaks deserialization of existing settings files, causing the app to fail to load on operators who upgrade from v1.1.

**Why it happens:** The current `Settings` struct already uses `#[serde(default)]` on most fields. But every new field MUST also have this attribute. The Rust serde default behavior is: if a field is missing from JSON and has no `#[serde(default)]`, deserialization fails. This is a common issue in apps that persist state across versions.

**Consequences:** Operator upgrades from v1.1 to v1.2. The app opens, tries to load `settings.json`, deserialization fails because a new field like `bls_registration_cache: Vec<BlsRegistration>` is missing from the old file. The app shows the initialization error screen: "Initialization Error: Json: missing field `bls_registration_cache`". The operator has to manually edit or delete `settings.json` to recover.

**Prevention:**
- Every new field in `Settings` MUST have `#[serde(default)]`.
- Write an explicit migration function that runs at startup: read raw JSON, check for missing keys, add defaults, write back. This is more robust than relying on serde defaults alone because it handles nested structure changes.
- The corresponding TypeScript `Settings` interface must have all new fields as optional (`?:`) with fallback values in the Zustand store initializer.
- Test the upgrade path: take a v1.1 `settings.json`, load it with v1.2 code, verify no errors.
- Do NOT rename existing fields. If a field name changes, keep the old one with `#[serde(alias = "old_name")]`.

**Detection:** QA testing on a clean install works fine. The bug only appears when upgrading from a previous version with an existing `settings.json`. The initialization error screen appears with a JSON deserialization error.

**Phase mapping:** Must be addressed at the start of the settings refactoring phase. The migration strategy should be decided before any new fields are added.

## Moderate Pitfalls

### Pitfall 5: Service Builder Type Mismatch -- SignatureAlgorithm Only Has secp256k1

**What goes wrong:** The existing `serviceBuilderStore.ts` hardcodes `signatureAlgorithm: 'secp256k1'` in `SubmitDraft` and the `SignatureAlgorithm` TypeScript type is `export type SignatureAlgorithm = 'secp256k1'`. The Rust backend now supports `SignatureAlgorithm::Bls12381`. Adding BLS to the UI requires updating the TypeScript types, the service builder store defaults, the `SubmitEditor.tsx` component, and the `buildSubmit()` function. Missing any one of these creates a type mismatch where the UI sends `"secp256k1"` but the operator intended BLS.

**Prevention:**
- Update `SignatureAlgorithm` type to `'secp256k1' | 'bls12381'`.
- Add algorithm selector dropdown in `SubmitEditor.tsx` (currently only has `SignaturePrefix` dropdown).
- Update `SubmitDraft.signatureAlgorithm` default to remain `'secp256k1'` for backward compatibility, but show the selector when submit type is `'aggregator'`.
- Update `buildSubmit()` to pass the selected algorithm through to the service JSON.
- When BLS is selected, hide the signature prefix dropdown (BLS does not use EIP-191 prefix).
- Test that editing an existing secp256k1 service (`hydrateFromService`) correctly preserves the algorithm field.

**Phase mapping:** BLS service deployment phase.

### Pitfall 6: Tauri Command Handler List Grows to 40+ Commands

**What goes wrong:** The existing `lib.rs` already registers 30 commands in `tauri::generate_handler![]`. Adding P2P status, BLS signer info, BLS registration, P2P peer details, quorum status, event correlation, and settings subcommands could add 10-15 more. Tauri's `generate_handler!` macro has compile-time cost proportional to command count. More importantly, the flat list becomes unmaintainable.

**Why it happens:** Tauri 2 requires all commands to be listed in a single `generate_handler![]` invocation. You cannot call `invoke_handler()` multiple times -- only the last call is used ([Issue #11447](https://github.com/tauri-apps/tauri/issues/11447)).

**Prevention:**
- Group commands into modules: `commands/mod.rs`, `commands/p2p.rs`, `commands/bls.rs`, `commands/settings.rs`, etc.
- Import all command functions in `lib.rs` but keep the actual logic in separate files. The current `commands.rs` single file is already ~1200 lines.
- Consider a "proxy" pattern: instead of one Tauri command per backend query, create a `cmd_query_wavs_api(endpoint: String, params: serde_json::Value)` command that forwards to the Axum HTTP API. This avoids duplicating every HTTP handler as a Tauri command. The existing `cmd_get_health_status` already does this (fetches from `http://host:port/health`).
- For P2P status specifically, use the existing HTTP API endpoint (`/p2p/status`) via the proxy pattern rather than adding a direct dispatcher access command.

**Phase mapping:** Should be addressed at the start of the milestone before adding new commands. Refactoring `commands.rs` into modules is a prerequisite.

### Pitfall 7: Quorum Progress Display Requires Data the P2pStatus Struct Doesn't Have

**What goes wrong:** The v1.2 milestone includes "quorum progress per service" in the P2P page. This requires knowing: (a) how many operators have submitted for each active event, (b) what the quorum threshold is, and (c) what the total operator set is. The current `P2pStatus` struct only has `connected_peers`, `peer_ids`, and `subscribed_services`. It does not include per-service quorum progress.

**Why it happens:** Quorum tracking lives in the aggregator's `QuorumQueue` (in `packages/wavs/src/subsystems/aggregator/queue.rs`), not in the P2P layer. The P2P status endpoint only knows about network connectivity, not about aggregation state. Exposing quorum progress requires a new API endpoint or extending the existing `/p2p/status` response, which mixes concerns.

**Consequences:** The P2P page either shows incomplete information (connected peers but no quorum data), or a new endpoint is created that duplicates aggregator state, or the `P2pStatus` struct is bloated with aggregation data that doesn't belong there.

**Prevention:**
- Create a separate `/aggregator/status` or `/services/quorum` endpoint that returns per-service quorum progress. Do NOT add it to `P2pStatus`.
- The response should include: `service_id`, `active_events` (count), `per_event_progress` (submissions received / quorum needed), `total_operators` (from Oracle peer set).
- The P2P page in the UI can make two parallel requests: one for P2P connectivity status, one for quorum status. Display them in separate sections.
- If the aggregator is not running (P2P disabled), the quorum endpoint should return empty data, not an error.

**Phase mapping:** P2P dashboard phase. The backend endpoint should be added before the UI.

### Pitfall 8: Tauri Serialization of Large BLS Keys Causes IPC Slowness

**What goes wrong:** BLS G1 public keys are 128 bytes (256 hex chars) and G2 signatures are 256 bytes (512 hex chars). When the UI requests signer info for all services, and each service has BLS keys, the IPC payload grows significantly. Tauri serializes command return values as JSON over the webview bridge. For 50 services with BLS keys, this is 50 * 512 hex chars = ~25KB per poll -- manageable but noticeable if polled frequently.

**Prevention:**
- Do NOT poll BLS key data on a timer. Fetch it once when the P2P page mounts and on service add/remove events.
- Use the service event listener (already in `listeners.ts`) to invalidate cached key data.
- For quorum display, only transmit event counts and progress percentages, not raw signature bytes.
- Consider a lazy-load pattern: show service names and connection status immediately, load BLS key details on-demand when the user expands a service card.

**Phase mapping:** P2P dashboard and BLS deployment phases.

### Pitfall 9: Activity Feed Unbounded Array Growth Under High Throughput

**What goes wrong:** The current `addActivity` in `appStore.ts` creates a new array on every event: `const next = [...state.activityList, item]`. For high-throughput services (block interval triggers every 12 seconds across multiple chains and services), this creates significant GC pressure. The `MAX_ACTIVITY_ITEMS = 2000` cap helps, but the `[...spread, item].slice()` pattern allocates a 2001-element array, copies all elements, then allocates a 2000-element array and copies again. Under sustained 10 events/second, this is 20 full-array copies per second.

**Why it happens:** The spread-copy pattern is idiomatic React/Zustand for immutable updates. It works fine at low volumes. The current app already handles this load level, but adding unified events (where each trigger creates a pending entry AND each submission updates it) doubles the mutation rate.

**Prevention:**
- Switch to a ring buffer implementation: pre-allocate a fixed-size array and track head/tail indices. Mutations are O(1) instead of O(n).
- Alternatively, use `immer` middleware with Zustand to make in-place mutations that are automatically wrapped in immutable updates.
- For the unified event model, consider a separate `Map<EventId, UnifiedEvent>` store with LRU eviction rather than an array. Lookups by EventId are O(1) instead of O(n) scans.
- The virtualized list (`@tanstack/react-virtual`, already in use) handles rendering fine -- the bottleneck is state mutation, not rendering.

**Phase mapping:** Activity/events unification phase. The store refactoring should happen before adding correlation logic.

## Minor Pitfalls

### Pitfall 10: Ed25519 Peer ID Display Format Confusion

**What goes wrong:** The P2P page shows the operator's Ed25519 peer ID (hex-encoded public key). This is a 64-character hex string -- visually similar to an EVM address but longer and with no `0x` prefix or checksum. Operators may confuse it with their EVM address or try to use it in contract interactions.

**Prevention:**
- Label clearly: "P2P Peer ID (Ed25519)" with a tooltip explaining what it is.
- Use a different visual treatment (different color, different font, different truncation pattern) from EVM addresses.
- The existing `AddressDisplay` component formats EVM addresses. Create a `PeerIdDisplay` component with distinct styling.
- Show the full hex on hover or copy, truncated by default.

**Phase mapping:** P2P dashboard phase.

### Pitfall 11: Settings Page Scroll Position Lost on Section Reorganization

**What goes wrong:** The current Settings page is a single scrollable column with 6 sections. The v1.2 milestone adds more sections (P2P configuration, BLS settings). If sections are reorganized (reordered, grouped into tabs, or moved to sub-pages), users who have muscle memory for "scroll down to MCP Server" will be confused and frustrated.

**Prevention:**
- If switching to tabs, use URL-based tab navigation (`/settings/general`, `/settings/p2p`, `/settings/mcp`) so browser back/forward work.
- Preserve the section order for existing sections. Add new sections at logical positions (P2P after Wallet, BLS within the service context).
- If keeping a single scroll layout, add a mini-nav sidebar or anchor links for quick section jumping.
- Consider whether P2P/BLS settings belong in Settings at all, or should be on the P2P page / service detail page respectively.

**Phase mapping:** Settings UX phase.

### Pitfall 12: Service Pause/Resume Commands Don't Account for BLS Registration State

**What goes wrong:** The existing `cmd_pause_service` and `cmd_resume_service` commands toggle `ServiceStatus`. For BLS services, pausing and resuming is fine at the dispatcher level. But if an operator pauses a BLS service, deregisters their BLS key from the on-chain contract, and then resumes -- the service will execute but submissions will fail on-chain because the key is no longer registered. The UI shows the service as "Active" with no indication of the registration problem.

**Prevention:**
- On service resume for BLS services, check BLS key registration status against the contract.
- Display a warning if the BLS key is not registered: "Service resumed but BLS key not registered. Submissions will fail."
- Consider adding a health check for BLS services that periodically verifies key registration.

**Phase mapping:** BLS deployment phase, after the basic key display is working.

### Pitfall 13: Multiple Signature Algorithm Display in Service Detail

**What goes wrong:** A service can have multiple workflows, each potentially with different signature algorithms (one secp256k1, one BLS). The service detail page must display per-workflow signature information, not a single "this service uses BLS" badge. The current `WorkflowViewer.tsx` shows trigger and component info but the submission section is minimal.

**Prevention:**
- Display signature algorithm per-workflow in the service detail view.
- In the P2P operator section, show the relevant key (EVM address for secp256k1 workflows, G1 pubkey for BLS workflows).
- The service list page can show a summary badge (e.g., "ECDSA + BLS" if mixed).

**Phase mapping:** BLS deployment and P2P dashboard phases.

### Pitfall 14: React Router Navigation Adds P2P Page Without Updating Header Navigation

**What goes wrong:** The current `Header.tsx` has navigation links for existing pages. Adding a P2P page requires updating both `App.tsx` (route definition) and `Header.tsx` (navigation link). If the route is added but the nav link is forgotten, the page is unreachable except by direct URL.

**Prevention:**
- Define routes and navigation items from a single source of truth (e.g., a `ROUTES` constant that both the router and header consume).
- Add the P2P route alongside the existing pattern in `App.tsx`.
- Test navigation between all pages after adding new routes.

**Phase mapping:** First phase that adds the P2P page.

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| P2P dashboard | Polling memory leak (Pitfall 1) | Use invoke(), not listen(). Add visibility-based pausing. Bound stored history. |
| P2P dashboard | Missing quorum data (Pitfall 7) | Create separate aggregator status endpoint. Don't bloat P2pStatus. |
| P2P dashboard | Peer ID confusion (Pitfall 10) | Distinct PeerIdDisplay component. Clear labeling. |
| BLS deployment | Key display confusion (Pitfall 2) | Per-service key display. Registration status check. Copy button for full key. |
| BLS deployment | Type mismatch in builder (Pitfall 5) | Update all TypeScript types. Add algorithm selector. Test hydration. |
| BLS deployment | IPC payload size (Pitfall 8) | Lazy-load BLS keys. Don't poll key data. |
| BLS deployment | Registration state (Pitfall 12) | Check registration on resume. Health check for BLS services. |
| Unified events | Correlation memory leak (Pitfall 3) | Add EventId to Rust events. TTL-based cleanup. Bound correlation map. |
| Unified events | Array growth performance (Pitfall 9) | Ring buffer or Map-based store. Avoid spread-copy at high volume. |
| Settings UX | State migration (Pitfall 4) | serde(default) on all new fields. Test upgrade from v1.1 settings.json. |
| Settings UX | Scroll/navigation disruption (Pitfall 11) | Preserve existing section order. URL-based tabs if splitting. |
| All phases | Command handler bloat (Pitfall 6) | Refactor commands.rs into modules first. Use HTTP proxy pattern. |
| All phases | Multi-workflow display (Pitfall 13) | Per-workflow algorithm display. Mixed badge on service list. |

## Sources

- [Tauri event listener memory leak (Issue #13133)](https://github.com/tauri-apps/tauri/issues/13133) - HIGH confidence (confirmed Tauri bug)
- [Tauri event emission memory leak (Issue #12724)](https://github.com/tauri-apps/tauri/issues/12724) - HIGH confidence (confirmed Tauri bug)
- [Tauri multiple invoke_handler limitation (Issue #11447)](https://github.com/tauri-apps/tauri/issues/11447) - HIGH confidence (Tauri design constraint)
- [Tauri command boilerplate (Issue #10075)](https://github.com/tauri-apps/tauri/issues/10075) - MEDIUM confidence (feature request, documents the problem)
- [Zustand memory leak discussion (#2540)](https://github.com/pmndrs/zustand/discussions/2540) - MEDIUM confidence (community pattern)
- [OWASP Key Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html) - HIGH confidence (authoritative security guidance)
- WAVS codebase analysis: `app/src-tauri/src/commands.rs` (1244 lines, 30 commands) - HIGH confidence (direct code inspection)
- WAVS codebase analysis: `app/src/tauri/listeners.ts` (87 lines, 5 event types) - HIGH confidence (direct code inspection)
- WAVS codebase analysis: `app/src/stores/appStore.ts` (spread-copy pattern, MAX caps) - HIGH confidence (direct code inspection)
- WAVS codebase analysis: `packages/gui/shared/src/event.rs` (TriggerEvent, SubmissionEvent -- no EventId) - HIGH confidence (direct code inspection)
- WAVS codebase analysis: `packages/types/src/http.rs` (P2pStatus struct, SignerResponse enum) - HIGH confidence (direct code inspection)
- WAVS codebase analysis: `packages/types/src/signing.rs` (EventId, WavsSignature, SignatureData enums) - HIGH confidence (direct code inspection)
- WAVS codebase analysis: `app/src/stores/serviceBuilderStore.ts` (hardcoded secp256k1 type) - HIGH confidence (direct code inspection)
- WAVS codebase analysis: `packages/gui/shared/src/settings.rs` (Settings struct with serde defaults) - HIGH confidence (direct code inspection)
