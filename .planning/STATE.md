---
gsd_state_version: 1.0
milestone: v1.3
milestone_name: Per-Service P2P Targeting
status: verifying
stopped_at: Completed 15-02-PLAN.md
last_updated: "2026-04-03T15:05:56.032Z"
last_activity: 2026-04-03
progress:
  total_phases: 4
  completed_phases: 2
  total_plans: 3
  completed_plans: 3
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-03)

**Core value:** Multi-operator signature aggregation over P2P must work reliably -- operators broadcast signed submissions, reach quorum, and submit on-chain
**Current focus:** Phase 15 — subscription-protocol

## Current Position

Phase: 16
Plan: Not started
Status: Phase complete — ready for verification
Last activity: 2026-04-03

Progress (v1.3): [##........] 25% (1/4 phases)

## Performance Metrics

**Velocity:**

- v1.0: 11 plans in ~2.8 hours (avg 15 min/plan)
- v1.1: 9 plans in ~102 min (avg ~11 min/plan)
- v1.2: 9 plans across 5 phases (~10 min/plan avg)

## Accumulated Context

### Decisions

Archived to PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [v1.3 research]: All changes contained to p2p.rs (~225 lines) + ~5 lines in types
- [v1.3 research]: Channel 0 (Engine) stays Recipients::All permanently; only channel 1 gets targeting
- [v1.3 research]: Sentinel service_id [0xFF; 32] for subscription announcements on existing channels
- [v1.3 research]: Unknown peers (no announcements) treated as subscribed-to-all for backward compat
- [v1.3 research]: Replace-not-merge on heartbeat subscription sync
- [Phase 14]: HashMap/HashSet for PeerSubscriptionMap (bridge loop is single-threaded)
- [Phase 14]: serde_json for SubscriptionAnnouncement encoding (matches existing P2pMessage payload pattern)
- [Phase 14]: get_recipients() returns Recipients::All as defensive fallback for empty subscriber sets
- [Phase 15]: serde(default) on full_state ensures backward compat with Phase 14 announcements
- [Phase 15]: set_peer_subscriptions uses remove_peer + reinsert for clean replace-not-merge semantics
- [Phase 15]: has_announced checks peer_to_services.contains_key for COMPAT-03 tracking
- [Phase 15]: Subscription announcements sent via direct_sender only (never mailbox.broadcast) to avoid Engine caching stale subscription state
- [Phase 15]: Inbound subscription announcements intercepted BEFORE ServiceRouter filtering and consumed with continue
- [Phase 15]: full_state=true dispatches to set_peer_subscriptions (replace); full_state=false dispatches to handle_announcement (incremental)

### Pending Todos

None.

### Blockers/Concerns

- Both `run_lookup_network` and `run_discovery_network` bridge loops need identical changes -- consider shared extraction to avoid divergence (flagged in research)

## Session Continuity

Last session: 2026-04-03T15:04:57.870Z
Stopped at: Completed 15-02-PLAN.md
Resume file: None
