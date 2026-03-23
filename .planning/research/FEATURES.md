# Feature Landscape

**Domain:** WAVS Tauri desktop app v1.2 -- P2P dashboard, BLS service deployment, unified activity events, settings UX
**Researched:** 2026-03-23
**Overall confidence:** MEDIUM-HIGH (based on codebase analysis, ecosystem research, and existing backend API inspection)

## Table Stakes

Features users expect from a node operator desktop app at this level of maturity. Missing = product feels incomplete given the backend capabilities that already exist.

### P2P Network Visibility

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Connected peers count and list | Every node dashboard (Prysm, Lighthouse/Siren, Grafana eth2 dashboards) shows connected peer count as the primary P2P health signal. WAVS backend already exposes this via `GET /p2p/status` returning `P2pStatus { connected_peers, peer_ids }`. Not showing it makes the app feel blind about network state. | Low | Backend `P2pStatus` struct already has `connected_peers: usize` and `peer_ids: Vec<String>` (Ed25519 hex). Poll the endpoint every 5-10s. Display count in header badge + full list on P2P page. |
| Local peer ID display | Operators need to know their own identity to share with other operators for peering configuration, troubleshooting, and verification. `P2pStatus.local_peer_id` (Ed25519 hex) is already available. Standard in all P2P node dashboards. | Low | Display truncated hex with copy-to-clipboard. Show full hex on expand/hover. Ed25519 public key is 32 bytes = 64 hex chars. |
| P2P enabled/disabled/mode indicator | Operators must know whether P2P is active and in what mode (Disabled/Local/Remote). The `GET /info` endpoint returns `p2p_config` (the enum) alongside `p2p_status`. Without this, operators cannot diagnose "why is my node not finding peers?" | Low | `InfoResponse.p2p_config` is already the `P2pConfig` enum (Disabled / Local { ... } / Remote { ... }). Show mode badge: "P2P Disabled", "P2P Local (port 9000)", "P2P Remote (2 bootstrappers)". |
| Listen address display | Operators need to know what address/port their P2P is binding to. `P2pStatus.listen_addresses` is already exposed. Required for configuring peers and firewall rules. | Low | Show socket addresses (e.g., `0.0.0.0:9000`). Available from `P2pStatus.listen_addresses: Vec<String>`. |
| Subscribed services indicator | The P2P page should show which services this node is participating in via P2P (sending/receiving submissions). `P2pStatus.subscribed_services` lists hex service ID hashes. Cross-reference with service names from the service registry. | Low | Map hex service IDs from `P2pStatus.subscribed_services` to service names from `appStore.services`. Display as tagged list. |

### BLS Service Deployment

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Signature algorithm selector in service builder | Backend supports both `secp256k1` and `bls12381` since v1.1. The current service builder UI hardcodes `signatureAlgorithm: 'secp256k1'` in `SubmitDraft`. Without a selector, operators cannot deploy BLS services from the UI at all. This is a blocking gap. | Low | Add `'bls12381'` to the `SignatureAlgorithm` type union. Add radio/toggle to `SubmitEditor` component. When `bls12381` is selected, hide `signaturePrefix` (BLS has no EIP-191 prefix concept). |
| BLS operator key display per service | After deploying a BLS service, operators need to see the BLS G1 public key that the node will use for signing. The `POST /services/signer` endpoint already returns `SignerResponse::Bls12381 { hd_index, g1_pubkey_hex }`. Without this, operators cannot complete the registration step. | Medium | Call `/services/signer` after deploy to retrieve the G1 pubkey. Display the 128-byte (256 hex char) key with truncation and copy button. Show HD index. This is significantly longer than an EVM address, so UI must handle gracefully. |
| Operator registration guidance | After deploying a BLS service, operators must register their BLS key with the POAStakeRegistry contract on-chain. The MCP tool does this via `wavs_register_operator`. The UI must at minimum tell the operator what to do next, even if it does not automate it. Without guidance, operators are stuck after deploy. | Low | Show a post-deploy info card: "BLS service deployed. Register your operator key with the POAStakeRegistry contract to begin participating in quorum." Include the G1 pubkey, the registry address, and a link to docs or CLI command. |

### Unified Activity Events

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Merged trigger + submission cards | Currently activity shows triggers and submissions as separate cards. Operators want to see the lifecycle: trigger fired -> component ran -> submission signed -> quorum reached -> on-chain submit. Separate cards force mental correlation. Merged cards show the journey in one place. | Medium | The existing `ActivityItem` has `kind: 'trigger' | 'submission'` and both share `serviceId`, `workflowId`, `triggerData`. Merge by matching trigger events to their resulting submissions via event ID correlation. Requires backend changes to include a correlation ID in both events, or heuristic matching on (serviceId + workflowId + timestamp window). |
| Error display in activity cards | When execution or submission fails, operators need to see the error inline in the activity feed. Currently errors only appear in logs, which requires cross-referencing timestamps. Every production monitoring tool (Grafana, Datadog, Sentry) surfaces errors inline with the events that caused them. | Medium | Requires backend to emit error details in submission events (e.g., `SubmissionEvent` gains an optional `error: string` field). Frontend `ActivityCard` renders error state with red accent and error message. |
| Submission result status | Activity cards should show whether a submission succeeded, failed, or is pending quorum. The aggregator already tracks `QuorumQueue::Active` vs `QuorumQueue::Burned` (completed). Surfacing this makes the activity feed actionable rather than just informational. | Medium | Requires a new event type or enriched `SubmissionEvent` that includes outcome (success/fail/pending). Display as status badge on the card: green check (submitted), amber clock (pending quorum), red X (failed). |

### Settings UX

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Section collapsibility / accordion | The current Settings page is a single scrollable list of 6 sections (~940 lines of TSX). As more sections are added (P2P config, key display), the page becomes unwieldy. Every complex settings page in professional apps uses collapsible sections or tabs. | Low | Use the existing `Expander` atom component or a simple accordion pattern. Default-open the sections that need attention, default-closed the rest. |
| Section anchoring / scroll-to | Operators should be able to jump directly to a settings section, especially when redirected from another page ("configure your P2P settings"). Without anchoring, operators have to scroll hunt. | Low | Add `id` attributes to section headers. Use URL hash fragments (e.g., `/settings#p2p`) or a sidebar table of contents. |
| Visual feedback on unsaved changes | The current TOML editor tracks `hasUnsavedChanges` but other sections (env vars, MCP settings) lack consistent save-state indicators. This causes confusion about whether settings are persisted. | Low | Apply the same pattern across all sections: show "(unsaved)" badge, disable navigation away warning, consistent Save/Revert buttons. |

## Differentiators

Features that set this app apart from typical node operator tooling. Not expected, but create significant value.

### P2P Network -- Advanced

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Real-time peer connection/disconnection notifications | Toast or inline notification when a peer connects or drops. Goes beyond static peer count to give operators live awareness of network dynamics. No Ethereum client UI does this well. | Medium | Would require a new Tauri event from the backend (emit on peer connect/disconnect from commonware p2p callbacks). Not available from polling alone. Deferred unless backend support is added. |
| Per-service quorum progress visualization | Show a progress bar or ring for each active service: "3/5 operators have submitted for event X". This turns the P2P page from a static info page into a live operations dashboard. No existing AVS operator tool provides this. | High | Requires polling `QuorumQueue` state per active event, per service. The aggregator tracks `QuorumQueue::Active(Vec<Submission>)` internally. Would need a new HTTP endpoint to expose quorum queue sizes per service. Significant backend work. |
| Peer latency / health indicators | Show approximate latency or "last seen" for each connected peer. Makes network quality visible. Grafana eth2 dashboards show peer latency as a standard metric. | High | Commonware p2p does not expose per-peer latency. Would require custom ping/pong or timing the message receipt. Not worth the complexity for v1.2. |
| Network topology mini-map | Visual graph showing this node's connections to peers. Prysm had a "peer map" feature (now deprecated). Visually compelling but of limited operational value. | High | Would require a graph rendering library (e.g., react-force-graph). Cool demo but high effort for low operational value. Defer. |

### BLS Keys -- Advanced

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| One-click BLS operator registration | Instead of just showing the key and telling the operator to register manually, automate the entire `registerOperator` + `updateOperatorSigningKey` flow from the UI. This is what EigenLayer's CLI does but no existing UI provides it as a one-click flow. | High | Requires: (1) Tauri command wrapping `chain_ops::register_operator()`, (2) owner credential input (separate from signing mnemonic), (3) registry address detection, (4) transaction signing and submission, (5) error handling for AlreadyRegistered, InsufficientFunds, etc. The MCP tool already has this logic in `chain_ops.rs`. Port to Tauri command layer. |
| BLS key backup/export warning | When a BLS service is deployed, remind operators that their BLS key is derived from their mnemonic. If the mnemonic is lost, the BLS key cannot be recovered. This is a security UX differentiator. | Low | Show a warning card after BLS service creation: "Your BLS signing key is derived from your recovery phrase. Ensure your recovery phrase is backed up." Link to Settings > Wallet > Export. |
| Dual key display (ECDSA + BLS per service) | Show both the secp256k1 operator address AND the BLS G1 public key for a BLS service. The ECDSA key is the operator identity (for registration); the BLS key is the signing key (for submissions). Making both visible prevents confusion. | Low | Call `/services/signer` per service. For BLS services, the response includes `hd_index` and `g1_pubkey_hex`. Also derive the ECDSA address from the same `hd_index` (already available via wallet store). Display both in service detail page. |
| Registration status checker | After showing the registration guidance, check on-chain whether the operator is actually registered on the POAStakeRegistry. Green check if registered, amber warning if not. Reduces support burden. | Medium | Requires reading `POAStakeRegistry.isRegistered(operator)` or equivalent view function on-chain. Need the registry address (from service manager -> get stake registry address). Uses existing viem/alloy infrastructure. |

### Activity -- Advanced

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Activity timeline / Gantt-style view | Instead of a flat list, show events on a time axis with service lanes. Makes patterns visible: "service A triggers every 5 blocks, service B has a 2-minute cron." No node operator tool provides this view. | High | Would require a custom canvas/SVG rendering or a library like vis-timeline. High effort, impressive demo. Defer to later milestone. |
| Event correlation chains | Link related events: trigger -> engine execution -> submission -> aggregation -> on-chain submit. Click one event, see the entire chain highlighted. This is the "distributed trace" view that Jaeger provides but integrated natively. | High | Requires a trace ID or correlation ID propagated through the system. The backend already integrates with Jaeger (OpenTelemetry). Could fetch trace data from Jaeger API, but this creates an external dependency. |
| Export activity as CSV/JSON | Operators may need to export activity data for analysis, reporting, or debugging. Simple export button. Low effort, real value. | Low | Serialize the filtered `activityList` to JSON or CSV. Use Tauri's file dialog to save. Straightforward feature with genuine utility. |

### Settings -- Advanced

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Guided first-run wizard | Instead of dumping new operators on a settings page, walk them through: (1) set WAVS home, (2) configure P2P, (3) set up wallet, (4) deploy first service. Progressive disclosure reduces overwhelm. | Medium | The app already has `WalletSetup` as a first-run gate. Extend this concept to cover full initial setup. Would replace the current "settings page as landing page" pattern. |
| P2P configuration editor | Currently P2P is configured only through `wavs.toml`. A dedicated P2P section in Settings with mode selector (Disabled/Local/Remote), port input, peer address list editor, and bootstrapper config would make P2P setup accessible without editing TOML. | Medium | Read current P2P config from `GET /info` response. Build a form that maps to the `P2pConfig` enum variants. Write changes back to `wavs.toml` via existing TOML write commands. The form fields directly mirror the config struct. |
| Settings import/export | Operators running multiple nodes or reinstalling need to transfer settings. Export as JSON file, import to restore. | Low | Serialize `Settings` struct + `wavs.toml` content to a single JSON blob. Import reverses the process. Low effort, useful for power users. |

## Anti-Features

Features to explicitly NOT build in v1.2.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| In-app key generation (BLS or ECDSA) | Keys are deterministically derived from the mnemonic. There is no separate BLS key generation step. Building a key generation UI implies a key model that does not match WAVS's architecture (HKDF-SHA256 derivation from mnemonic + HD index + domain separator). | Show the derived keys. Explain they come from the mnemonic. Do not offer "generate new key" or "import key" flows. |
| Custom P2P peer management (add/remove/block) | Commonware p2p manages peer connections automatically via discovery or lookup. Manual peer add/remove would conflict with the automated peer set. Peer blocking is a backend feature that does not need a UI in v1.2. | Show connected peers read-only. Configuration of authorized peers happens in `wavs.toml` peer_addresses / authorized_peers arrays. |
| Transaction history / on-chain explorer | Showing on-chain transaction history (submitted envelopes, registration txs) requires indexing on-chain data. This is the domain of block explorers (Etherscan) not a node operator app. | Link to block explorer for relevant transactions. Show tx hash with explorer link when available. |
| Multi-node management | Managing multiple WAVS node instances from one app would require SSH/remote connections and dramatically increase scope. | WAVS app manages the local node only. Operators needing multi-node use Grafana + Prometheus (already supported). |
| BLS threshold / DKG UI | Threshold BLS (DKG key ceremonies) is explicitly out of scope per PROJECT.md. Building UI for it prematurely creates expectations. | Do not mention DKG in the UI. The algorithm selector should be "ECDSA (secp256k1)" and "BLS (BLS12-381)". No "threshold" option. |
| Cosmos BLS services | BLS submission is EVM-only per the current implementation constraint. Building Cosmos BLS UI would create a dead path. | Only show BLS algorithm option when the service manager is EVM. If Cosmos manager selected, disable BLS with tooltip "BLS is currently EVM-only." |
| Real-time log streaming to activity | Merging raw log lines into the activity feed conflates two different views. Logs are for debugging (verbose, noisy). Activity is for operations (structured, filtered). | Keep Logs and Activity as separate pages. Activity shows structured events. Logs shows raw tracing output. They serve different audiences. |

## Feature Dependencies

```
Existing: Service Builder (4-step wizard)
         |
         +-- [NEW] Signature algorithm selector (modify SubmitEditor)
         |     |
         |     +-- [NEW] BLS-aware deploy step (different contract ABI)
         |           |
         |           +-- [NEW] Post-deploy BLS key display
         |                 |
         |                 +-- [NEW] Registration guidance card
         |                       |
         |                       +-- [DIFFERENTIATOR] One-click registration

Existing: Activity Feed (virtualized list, filter/search)
         |
         +-- [NEW] Enriched SubmissionEvent (status, error, correlation)
         |     |
         |     +-- [NEW] Merged trigger+submission cards
         |     |
         |     +-- [NEW] Error display in cards
         |     |
         |     +-- [NEW] Status badges (success/pending/failed)

[NEW] P2P Page (new top-level route)
  |
  +-- Polls GET /p2p/status and GET /info
  |
  +-- [TABLE STAKES] Peer count + list
  +-- [TABLE STAKES] Local peer ID
  +-- [TABLE STAKES] P2P mode indicator
  +-- [TABLE STAKES] Listen addresses
  +-- [TABLE STAKES] Subscribed services
  +-- [DIFFERENTIATOR] Quorum progress visualization

Existing: Settings Page (6 sections)
  |
  +-- [TABLE STAKES] Section collapsibility
  +-- [TABLE STAKES] Section anchoring
  +-- [TABLE STAKES] Consistent save-state UX
  +-- [DIFFERENTIATOR] P2P configuration editor
```

Key dependency observations:
- BLS selector is the entry point -- everything downstream depends on it
- P2P page is fully independent of other features (no dependencies)
- Activity enrichment requires backend event schema changes
- Settings refactor is independent and can be done any time
- The P2P page depends only on existing backend endpoints (no new backend work)

## MVP Recommendation

Prioritize (in order):

1. **P2P page with peer visibility** -- Entirely frontend, zero backend changes needed. `GET /p2p/status` and `GET /info` already return all needed data. Highest value-to-effort ratio. Gives operators the one thing they most lack: network visibility.

2. **BLS/ECDSA algorithm selector in service builder** -- Unblocks BLS service deployment from the UI. Small type change (`SignatureAlgorithm` union), small UI change (radio buttons), large capability unlock.

3. **Post-deploy BLS key display + registration guidance** -- Completes the BLS deploy flow. Without this, operators deploy but cannot finish setup. Uses existing `/services/signer` endpoint.

4. **Settings page collapsible sections** -- Quick win that improves UX for all future settings additions. Use existing `Expander` component.

5. **Activity event enrichment (status + errors)** -- Requires backend changes (new event fields). Do after the simpler frontend-only features are done.

Defer to later milestone:
- **One-click BLS registration**: High complexity, requires owner credential handling. Provide guidance card first.
- **Merged trigger+submission cards**: Requires correlation ID infrastructure. Current separate cards work.
- **Per-service quorum progress**: Requires new backend endpoints. Show subscribed services list first.
- **P2P config editor in settings**: Operators can use TOML editor for now. Build after validating demand.
- **Activity timeline/Gantt view**: Cool but not critical. Activity list with filters is sufficient.

## Sources

### Codebase Analysis (HIGH confidence)
- `packages/types/src/http.rs` -- `P2pStatus`, `SignerResponse` (Bls12381 variant), `InfoResponse` structs
- `packages/wavs/src/http/handlers/p2p.rs` -- `GET /p2p/status` handler
- `packages/wavs/src/http/handlers/info.rs` -- `GET /info` handler returning P2P config + status
- `packages/wavs/src/http/handlers/service/key.rs` -- `POST /services/signer` returning BLS G1 pubkey
- `packages/wavs/src/subsystems/aggregator/p2p.rs` -- `P2pConfig` enum (Disabled/Local/Remote)
- `packages/wavs-mcp/src/chain_ops.rs` -- `register_operator()` function (BLS registration flow)
- `packages/types/src/solidity_types/bls.rs` -- BLS contract ABI bindings
- `app/src/stores/serviceBuilderStore.ts` -- Current builder hardcodes `secp256k1`
- `app/src/types/index.ts` -- Current `SignatureAlgorithm` type only has `'secp256k1'`
- `app/src/components/activity/ActivityFeed.tsx` -- Existing virtualized activity list
- `app/src/pages/Settings.tsx` -- Current settings page structure (~940 lines)

### Ecosystem Research (MEDIUM confidence)
- [Prysm Web UI docs](https://prysm.offchainlabs.com/docs/prysm-usage/web-interface/) -- Validator dashboard features: wallet management, key management, peer map, validator state. Notably deprecated in favor of Grafana dashboards.
- [eth-docker Web UI docs](https://ethdocker.com/Usage/WebUI/) -- Lighthouse Siren and Prysm web UI comparison
- [Grafana eth2 dashboards](https://github.com/metanull-operator/eth2-grafana) -- Standard peer count, peer list, connection metrics for Ethereum nodes
- [EigenLayer AVS Dashboard onboarding](https://docs.eigenlayer.xyz/developers/HowTo/onboard-avs-dashboard) -- AVS dashboard shows operator list, quorum status, restaked strategies
- [EigenLayer CLI key management](https://docs.eigencloud.xyz/products/eigenlayer/concepts/keys-and-signatures) -- ECDSA for operator identity, BLS for attestation signatures
- [EigenLayer operator registration](https://blog.unit410.com/engineering/eigenlayer/ethereum/2024/07/23/secure-operator-registration-in-eigenlayer.html) -- Secure registration flow: ECDSA + BLS keys, quorum registration
- [Ava Protocol operator docs](https://avaprotocol.org/docs/ethereum/EigenLayer-AVS/3-operator) -- Registration with ECDSA private key and BLS keypair
- [OWASP Key Management](https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html) -- Never display private keys in plaintext, log access, overwrite after use
- [Crypto UX/UI Design Patterns](https://avark.agency/learn/article/blockchain-ux-design-guide/) -- Dark theme standard for crypto apps, progressive disclosure, minimalist design
