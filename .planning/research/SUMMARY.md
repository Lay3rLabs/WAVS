# Project Research Summary

**Project:** WAVS v1.3 Per-Service P2P Targeting
**Domain:** Distributed pub/sub targeting within an authenticated P2P mesh
**Researched:** 2026-04-03
**Confidence:** HIGH

## Executive Summary

WAVS v1.3 replaces the current "broadcast to all peers, filter at receiver" P2P model with "targeted delivery to subscribed peers via `Recipients::Some`." This is not an architectural overhaul — it is a focused application-layer addition on top of the existing commonware-p2p stack. No new crate dependencies are required. The `Recipients::Some(Vec<PublicKey>)` variant already exists in commonware-p2p 2026.3.0 and is already imported but unused at 14+ call sites in `p2p.rs`. The work is to build the subscription tracking infrastructure that tells the sender which peers to target per service.

The recommended approach is a four-component addition: (1) a `PeerSubscriptionMap` data structure inside the existing bridge loop, (2) a `SubscriptionAnnouncement` control message multiplexed on the existing P2P channels via a sentinel service_id `[0xFF; 32]`, (3) replacement of `Recipients::All` with `Recipients::Some(service_peers)` on the direct channel (channel 1) only, and (4) piggybacking subscription state on the existing 2-second heartbeat for eventual consistency. All changes are contained to `p2p.rs` plus a minor extension to `P2pStatus` in types. The Aggregator, Dispatcher, and all other subsystems remain untouched. Estimated delta: ~225 lines in `p2p.rs` + ~5 lines in `packages/types`.

The critical risk is subscription state race conditions leading to quorum failures during service bootstrap and rolling upgrades. The mitigation is to keep channel 0 (broadcast Engine) at `Recipients::All` permanently — it serves as the reliability fallback — and apply targeting only to channel 1 (direct). Unknown peers with no subscription data must be treated as subscribed to all services. Empty subscriber sets must fall back to `Recipients::All` rather than silently dropping messages. Subscription announcements must be replayed periodically via heartbeat, not relied upon as one-shot fire-and-forget delivery.

## Key Findings

### Recommended Stack

No new dependencies are needed. The full feature set is achievable with existing commonware-p2p 2026.3.0 primitives. `Recipients::Some` is already available on both `Sender::send()` (channel 1) and `Mailbox::broadcast()` (channel 0). The subscription protocol is a pure application-layer addition using `commonware-codec` for message encoding (same derive pattern as `P2pMessage`), standard `HashMap`/`HashSet` for the subscription data structure (no concurrent access needed — the bridge loop is single-threaded), and the existing `tokio::sync::mpsc` command channel between the bridge loop and the rest of the application.

**Core technologies (all unchanged from existing stack):**
- `commonware-p2p` 2026.3.0: `Recipients::Some(Vec<PublicKey>)` for targeted send — verified from source at `src/lib.rs` lines 42-46
- `commonware-broadcast` 2026.3.0: Engine remains at `Recipients::All` for catch-up reliability; Engine caches per-peer with no service awareness
- `commonware-codec` 2026.3.0: `Encode`/`Decode` derive for new `SubscriptionAnnouncement` type — same pattern as existing `P2pMessage`
- `commonware-cryptography` 2026.3.0: `ed25519::PublicKey` as peer identity key in subscription map
- `std::collections::HashMap`/`HashSet`: Subscription registry lives in single-threaded bridge loop; no `dashmap` needed

See `STACK.md` for full analysis, the wire format migration plan, and the "what NOT to add" analysis.

### Expected Features

The v1.3 feature set has a clear critical path: subscription registry and announcement protocol must be built before targeted send can be enabled. Observability features are independent and can be added incrementally. Per-service catch-up scoping is explicitly deferred.

**Must have (table stakes — interdependent, build together):**
- `PeerSubscriptionMap` (`service_id -> Set<ed25519::PublicKey>`) — central data structure for outbound routing
- `SubscriptionAnnouncement` control message with sentinel service_id (`[0xFF; 32]`) — multiplexed on existing channels
- Heartbeat-based subscription sync — re-announce full subscription set on every heartbeat for eventual consistency
- Replace `Recipients::All` with `Recipients::Some(service_peers)` on channel 1 (direct), with fallback to `Recipients::All` when subscriber set is empty
- Subscription lifecycle: announce on subscribe/unsubscribe, re-announce full set on node startup after peer connections established
- Backward compatibility: unknown peers (no announcements received) treated as subscribed to all services

**Should have (value-add, low effort):**
- Per-service peer count in `/p2p/status` — low-effort, high-value debugging for quorum failures
- Targeted vs. fallback broadcast metrics — detect subscription protocol issues in production
- Peer disconnect cleanup via `Provider::subscribe()` notifications — remove stale subscription entries on disconnect

**Defer to v1.4+:**
- Per-service catch-up scoping — requires forking `commonware-broadcast` or building a parallel caching layer; the Engine's bounded 128-message deque and receiver-side `ServiceRouter` filtering are sufficient at current scale
- Subscription validation against on-chain operator registries — requires per-service chain queries, fragile at this stage
- Delta-based heartbeat subscription encoding (bloom filters) — optimize only if heartbeat bandwidth exceeds ~8 KB/s threshold (requires >100 services per node)

See `FEATURES.md` for the GossipSub comparison table and full feature dependency graph.

### Architecture Approach

The architecture principle is "extend, don't restructure." The existing two-channel bridge loop pattern (channel 0 = broadcast Engine for reliability/catch-up, channel 1 = direct sender for real-time delivery) is sound and proven. All changes are additive inside the bridge loop. `P2pCommand`, `P2pHandle`, `AggregatorCommand`, `Aggregator`, and `Dispatcher` require zero changes — the `P2pHandle` API surface is identical before and after v1.3.

**Major components (new or modified in `p2p.rs`):**
1. `PeerSubscriptionMap` — NEW struct; `service_to_peers` forward index for outbound targeting + `peer_to_services` reverse index for efficient disconnect cleanup; ~80 lines
2. `SubscriptionAnnouncement` — NEW wire message type encoded as `P2pMessage` with sentinel `[0xFF; 32]` as service_id; detected in inbound handler before `ServiceRouter` routing; ~15 lines
3. Bridge loop inbound handler — MODIFIED to detect subscription sentinel and dispatch to `PeerSubscriptionMap.handle_announcement()`; subscription messages never forwarded to Aggregator
4. Bridge loop publish handler — MODIFIED to call `peer_subscriptions.get_recipients()` returning `Recipients::Some(peers)` or `Recipients::All` fallback; applied to channel 1 only; channel 0 stays at `Recipients::All`
5. Bridge loop heartbeat — MODIFIED to broadcast full subscription state alongside probe; idempotent by design
6. `P2pStatus` in `packages/types/src/http.rs` — EXTENDED with `peer_subscriptions` field; ~5 lines

**Build order** (dependency-aware): Core data structures (Phase 1, no behavioral changes) -> Subscription protocol announce/receive (Phase 2, additive-only) -> Targeted outbound on channel 1 (Phase 3, the behavioral change) -> Observability and status (Phase 4, non-functional).

**Key refactoring note:** Both `run_lookup_network` and `run_discovery_network` bridge loops require identical changes (~60 lines each). Consider extracting shared bridge loop logic into a generic function to avoid divergence.

See `ARCHITECTURE.md` for full component diagrams, data flow sequences, and code-level patterns with specific before/after code examples.

### Critical Pitfalls

1. **Subscription state race on service add** — Peers that join a service late miss targeted sends during the window between when they start executing and when other peers learn of their subscription. Prevention: keep channel 0 (Engine) at `Recipients::All` permanently so the Engine's deque-based catch-up remains functional for all peers regardless of subscription timing. Targeted send on channel 1 is a bandwidth optimization, not a correctness requirement.

2. **`Recipients::Some(vec![])` silently drops messages** — An empty subscriber set returns `Ok(vec![])` from `Sender::send()` with no error, no log, no message delivered. Prevention: explicit check in `get_recipients()` — if `service_peers.is_empty()`, use `Recipients::All` as fallback; add `warn!` log when fallback fires.

3. **Subscription announcement delivery not guaranteed** — A single fire-and-forget announcement can be lost; the sender believes peers know about its subscription but they do not, leading to silent quorum failures on channel 1. Prevention: piggyback full subscription state on every heartbeat; when receiving a heartbeat with subscription data, replace (not merge) the peer's subscription set to handle unsubscriptions correctly.

4. **Dual-channel divergence** — Applying `Recipients::Some` to the Engine (channel 0) prevents it from caching messages for peers not in the original recipient set, permanently breaking catch-up for late-joining peers. Prevention: channel 0 stays at `Recipients::All` unconditionally; only channel 1 gets targeted delivery.

5. **Backward compatibility during rolling upgrades** — Updated operators using `Recipients::Some` exclude old operators who never send subscription announcements, breaking quorum during the transition window. Prevention: treat any peer without subscription data as subscribed to all services; add a feature flag `p2p.targeted_send_enabled` (default false) for coordinated deployment.

See `PITFALLS.md` for all 11 pitfalls with detection strategies, phase mappings, and code-level prevention patterns.

## Implications for Roadmap

Based on combined research, four phases sequenced by dependency:

### Phase 1: Core Data Structures
**Rationale:** All new code, zero behavioral changes. Safe to merge without affecting any existing functionality. Must come first because all subsequent phases depend on `PeerSubscriptionMap` and `SubscriptionAnnouncement` types existing.
**Delivers:** `PeerSubscriptionMap` struct with unit tests, `SubscriptionAnnouncement` type with codec roundtrip tests, `SUBSCRIPTION_SENTINEL` constant, `ServiceRouter::subscribed_services_raw()` method.
**Addresses:** Foundational types only — no features activated yet, but all defensive abstractions established (including the empty-set fallback check in `PeerSubscriptionMap.get_recipients()`).
**Avoids:** Pitfall 8 (empty recipients — fallback logic embedded in data structure from day one).

### Phase 2: Subscription Protocol (Announce + Receive)
**Rationale:** Adds the subscription protocol to the bridge loop but does NOT yet change outbound targeting. Messages still go to `Recipients::All`. This phase can be deployed and validated in production before any targeting behavior changes — zero regression risk. Also the right phase to address the duplicate bridge loop refactoring.
**Delivers:** Inbound subscription announcement detection and dispatch, outbound subscription announcements on `Subscribe`/`Unsubscribe`, heartbeat-carried subscription state (full-set, replace-not-merge), `PeerSubscriptionMap` wired into both `run_lookup_network` and `run_discovery_network` as live state.
**Addresses:** Subscription registry (table stakes), subscription announcement protocol (table stakes), subscription lifecycle (table stakes), backward compatibility (unknown peers = subscribed to all).
**Avoids:** Pitfall 3 (lost announcements — heartbeat sync), Pitfall 5 (backward compat — unknown peers default), Pitfall 6 (staleness — replace-not-merge on heartbeat sync), Pitfall 7 (multi-node testing — extend test setup to `setup_n_nodes` in this phase).

### Phase 3: Targeted Outbound (Direct Channel Only)
**Rationale:** This is the behavioral change that delivers the v1.3 value proposition. Done after Phase 2 has been validated. Reverting to `Recipients::All` is a one-line change if targeting causes issues in production.
**Delivers:** Replace `Recipients::All` with `Recipients::Some(service_peers)` on channel 1 (direct) in both bridge loops, empty-set fallback to `Recipients::All`, retry queue drain using targeted recipients, feature flag `p2p.targeted_send_enabled`.
**Addresses:** Targeted send via `Recipients::Some` (table stakes), fallback to broadcast-all (table stakes).
**Avoids:** Pitfall 1 (race — Engine stays at `Recipients::All`), Pitfall 4 (dual-channel divergence — only channel 1 gets targeted), Pitfall 8 (empty recipients — `get_recipients()` handles), Pitfall 5 (rolling upgrade — feature flag).

### Phase 4: Observability and Status
**Rationale:** Non-functional phase with high operational value. Required for diagnosing subscription protocol issues in production deployments. Low effort (~10 lines total), high value for operators debugging quorum failures. Independent of all other phases.
**Delivers:** `peer_subscriptions: HashMap<String, Vec<String>>` field in `P2pStatus`, extended `/p2p/status` HTTP response, targeted vs. broadcast-fallback metrics counters (`p2p_targeted_sends`, `p2p_broadcast_fallback_sends`).
**Addresses:** Per-service peer count in `/p2p/status` (should-have), targeted vs. fallback metric (should-have).
**Avoids:** Pitfall 11 (hidden subscription state — full map exposed via HTTP API).

### Phase Ordering Rationale

- Phase 1 before everything: the defensive abstractions (empty-set fallback, sentinel constant, `SubscriptionAnnouncement` codec) must exist before any protocol or targeting code is written.
- Phase 2 before Phase 3: subscription state must be tracked and converging before targeted send can be safely enabled; deploying Phase 2 alone validates the protocol without any behavior change.
- Phase 3 is the isolated behavioral change: by deferring it to Phase 3, the diff is small and focused, and rollback is trivial.
- Phase 4 is independent: can be interleaved with Phase 2 or 3 if desired; has no blockers.
- The four-phase order mirrors the ARCHITECTURE.md recommended build sequence, which was derived from direct dependency analysis of the code.

### Research Flags

Phases with standard patterns (all research is complete — skip `research-phase` for all phases):
- **Phase 1:** Pure data structure design, fully specified in ARCHITECTURE.md with code examples and unit test shapes.
- **Phase 2:** Protocol design fully specified; sentinel pattern mirrors existing `HEARTBEAT_SERVICE_ID`; replace-not-merge heartbeat sync is a resolved design decision.
- **Phase 3:** Mechanical replacement of `Recipients::All` with `Recipients::Some`; fallback logic fully specified; feature flag pattern is standard.
- **Phase 4:** Additive struct fields and HTTP handler extension; no novel patterns.

**Post-v1.3 research items (not in scope, flag for v1.4):**
- Per-service catch-up scoping: requires upstream commonware-broadcast changes or a WAVS-level catch-up protocol; start research only when operator counts exceed ~50 services per node or catch-up bandwidth becomes a measurable problem.
- Subscription heartbeat optimization (bloom filters/delta encoding): start research only when heartbeat bandwidth measurement exceeds ~8 KB/s.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | `Recipients::Some` verified from commonware-p2p 2026.3.0 source; no new deps confirmed; 14+ `Recipients::All` call sites identified in `p2p.rs` by direct code inspection |
| Features | HIGH | Feature set derived from GossipSub spec comparison + codebase analysis; critical path (registry -> protocol -> targeted send) clearly defined; all anti-features documented with rationale |
| Architecture | HIGH | All findings from direct source analysis of `p2p.rs` (~1400 lines), `aggregator.rs`, `dispatcher.rs`, and commonware-broadcast Engine source; ~225-line delta estimated with component-level breakdown |
| Pitfalls | HIGH | 11 pitfalls with code-level evidence; all four critical pitfalls (race, silent drop, dual-channel divergence, rolling upgrade) verified from commonware source behavior; each pitfall cites specific file paths |

**Overall confidence:** HIGH

### Gaps to Address

- **`run_discovery_network` vs. `run_lookup_network` code duplication:** Both bridge loops require identical changes. Whether to extract a shared generic function or apply the changes separately should be decided at the start of Phase 2 — the refactoring opportunity exists here, before the changes are duplicated across both loops.

- **Peer disconnect detection mechanism:** The PITFALLS research rates peer disconnect cleanup as low-priority (commonware silently drops messages to offline peers; stale subscription entries cause wasted effort but not message loss). Confirm during Phase 2 whether to implement cleanup immediately via `Provider::subscribe()` notifications, or defer it. This affects whether the `peer_to_services` reverse index in `PeerSubscriptionMap` is used at all in v1.3.

- **Startup re-announcement timing:** After node restart, the node must re-announce subscriptions after peers connect. The research recommends piggybacking on the heartbeat timer, but the exact trigger (first heartbeat tick vs. first non-empty `connected_peers_tracker`) should be confirmed during Phase 2 implementation based on the actual startup sequence in `spawn_commonware_runtime`.

## Sources

### Primary (HIGH confidence — direct source code analysis)
- `commonware-p2p-2026.3.0/src/lib.rs` — `Recipients<P>` enum (`All`, `Some(Vec<P>)`, `One(P)`); `Sender::send()` and `Broadcaster::broadcast()` API
- `commonware-broadcast-2026.3.0/src/buffered/engine.rs` — Engine cache architecture (per-peer deques, global digest BTreeMap, no service awareness, bounded by `deque_size`)
- `packages/wavs/src/subsystems/aggregator/p2p.rs` — Current implementation: `ServiceRouter`, dual-channel bridge loops, `P2pCommand` enum, heartbeat sentinel pattern, `connected_peers_tracker`
- `packages/wavs/src/subsystems/aggregator.rs` — `AggregatorCommand` variants and dispatch logic
- `packages/wavs/src/dispatcher.rs` — `SubscribeService`/`UnsubscribeService` integration points
- `packages/types/src/http.rs` — `P2pStatus` struct
- `packages/wavs/tests/p2p_broadcast_tests.rs` — Existing test coverage (BCAST-01 through CATCH-02, 2-node setup helper)

### Secondary (MEDIUM confidence — specifications and external references)
- [GossipSub v1.0 Specification](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.0.md) — subscription protocol, "hello" packet design, mesh management comparison
- [GossipSub v1.1 Specification](https://github.com/libp2p/specs/blob/master/pubsub/gossipsub/gossipsub-v1.1.md) — peer scoring, extended validation
- [Publish-subscribe pattern](https://en.wikipedia.org/wiki/Publish%E2%80%93subscribe_pattern) — topic-based vs. content-based routing taxonomy
- [crates.io: commonware-p2p](https://crates.io/crates/commonware-p2p) — version 2026.3.0 confirmed latest

### Tertiary (reference)
- [commonware GitHub monorepo](https://github.com/commonwarexyz/monorepo) — upstream source of truth for commonware crates
- [Selective Delivery in P2P Topic-Based Pub/Sub Systems](https://www.researchgate.net/publication/308814521) — academic reference for design space analysis

---
*Research completed: 2026-04-03*
*Ready for roadmap: yes*
