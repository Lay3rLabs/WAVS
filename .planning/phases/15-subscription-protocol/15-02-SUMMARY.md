---
phase: 15-subscription-protocol
plan: 02
subsystem: p2p
tags: [commonware, subscription, bridge-loop, announcement, peer-tracking, direct-sender]

# Dependency graph
requires:
  - phase: 15-subscription-protocol plan 01
    provides: full_state field on SubscriptionAnnouncement, set_peer_subscriptions(), has_announced()
  - phase: 14-subscription-data-structures
    provides: SubscriptionAnnouncement, PeerSubscriptionMap, SUBSCRIPTION_SENTINEL, is_subscription_announcement, ServiceRouter::subscribed_services_raw
provides:
  - Subscription protocol wired into both P2P bridge loops (lookup and discovery)
  - Subscribe/unsubscribe announcements broadcast to all connected peers via direct_sender
  - Inbound subscription announcement interception and PeerSubscriptionMap updates
  - Heartbeat-piggybacked full subscription state for self-healing consistency
  - Hello message on first contact with new peers (full subscription set)
  - peer_subscriptions and known_peers state variables in both bridge loops
affects: [16-targeted-delivery plan using get_recipients() and has_announced() from peer_subscriptions]

# Tech tracking
tech-stack:
  added: []
  patterns: [announce-on-command with direct_sender only, intercept-before-filter for subscription messages, heartbeat piggybacking for eventual consistency, hello-on-first-contact for immediate sync]

key-files:
  created: []
  modified:
    - packages/wavs/src/subsystems/aggregator/p2p.rs

key-decisions:
  - "Subscription announcements sent via direct_sender.send() only -- never mailbox.broadcast() -- to avoid Engine caching stale subscription state"
  - "Inbound subscription announcements intercepted BEFORE ServiceRouter filtering and consumed with continue (never forwarded to Aggregator)"
  - "full_state=true dispatches to set_peer_subscriptions (replace-not-merge); full_state=false dispatches to handle_announcement (incremental)"
  - "Hello on first contact uses Recipients::One(peer) for targeted delivery to only the new peer"
  - "Heartbeat subscription piggybacking guarded by !my_services.is_empty() to avoid broadcasting empty sets"

patterns-established:
  - "Direct-channel-only for control messages: all subscription announcements use direct_sender, not mailbox"
  - "Intercept-before-filter: subscription announcements processed before ServiceRouter.should_accept()"
  - "Identical bridge loop changes: both run_lookup_network and run_discovery_network get character-for-character identical subscription protocol code"

requirements-completed: [ANN-01, ANN-02, ANN-03, ANN-04, COMPAT-03]

# Metrics
duration: 7min
completed: 2026-04-03
---

# Phase 15 Plan 02: Bridge Loop Subscription Protocol Summary

**Subscription announcement protocol wired into both P2P bridge loops with subscribe/unsubscribe broadcasting, inbound interception, heartbeat piggybacking, and new-peer hello messages via direct_sender**

## Performance

- **Duration:** 7 min
- **Started:** 2026-04-03T14:55:56Z
- **Completed:** 2026-04-03T15:03:22Z
- **Tasks:** 2
- **Files modified:** 1

## Accomplishments
- Wired subscription protocol into run_lookup_network bridge loop at all 5 integration points
- Wired identical subscription protocol into run_discovery_network bridge loop at all 5 integration points
- Both bridge loops now: broadcast announcements on subscribe/unsubscribe (ANN-01/02), intercept inbound announcements before ServiceRouter filtering, send hello on first contact with new peers (ANN-04), and piggyback full subscription state on heartbeat (ANN-03)
- peer_subscriptions (PeerSubscriptionMap) and known_peers (HashSet) state variables added to both loops
- All 32 unit tests pass, 97 full crate tests pass, zero regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire subscription protocol into run_lookup_network bridge loop** - `13c79255` (feat)
2. **Task 2: Wire identical subscription protocol into run_discovery_network bridge loop** - `90e2170f` (feat)

## Files Created/Modified
- `packages/wavs/src/subsystems/aggregator/p2p.rs` - Both bridge loops (lookup and discovery) extended with subscription announcement broadcasting, inbound interception, heartbeat piggybacking, and new-peer hello messages (+192 lines)

## Decisions Made
- Subscription announcements use `direct_sender.send()` only (never `mailbox.broadcast()`) per Research Pitfall 1 -- Engine caching would replay stale subscription state on reconnect
- Inbound subscription announcements intercepted AFTER deduplication but BEFORE ServiceRouter filtering -- ServiceRouter would reject SUBSCRIPTION_SENTINEL since it is not a subscribed service
- `full_state` field dispatches between replace-not-merge (`set_peer_subscriptions`) and incremental (`handle_announcement`) processing, matching the Phase 15 Plan 01 design
- Hello on first contact sends to `Recipients::One(peer)` to avoid broadcasting to all peers on every new peer connection
- Heartbeat subscription piggybacking is separate from the heartbeat probe -- the probe populates connected_peers_tracker while the subscription announcement is a separate direct_sender message

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Pre-existing clippy errors in wavs crate (dead_code for Phase 14 methods not yet consumed by Phase 16, too_many_arguments, clone_on_copy in submit.rs) -- these are out of scope and not caused by this plan's changes. Documented in Plan 01 SUMMARY as well.

## User Setup Required
None - no external service configuration required.

## Known Stubs
None -- all functionality is fully implemented and tested. The `peer_subscriptions` variable is written to but `get_recipients()` is not yet called -- this is intentional; Phase 16 will consume it for targeted delivery.

## Next Phase Readiness
- Both bridge loops have peer_subscriptions populated with live subscription data from all connected peers
- Phase 16 can use `peer_subscriptions.get_recipients(service_id)` to replace `Recipients::All` with `Recipients::Some(service_peers)` for targeted delivery
- Phase 16 can use `peer_subscriptions.has_announced(peer)` for COMPAT-03 backward-compatible fallback
- known_peers HashSet tracks which peers have been seen for hello protocol
- All 32 subscription-related tests pass as regression safety net for Phase 16 changes

## Self-Check: PASSED

- FOUND: packages/wavs/src/subsystems/aggregator/p2p.rs
- FOUND: .planning/phases/15-subscription-protocol/15-02-SUMMARY.md
- FOUND: commit 13c79255 (Task 1)
- FOUND: commit 90e2170f (Task 2)
- Structural check: 2 instances each of all 9 integration point markers across both bridge loops

---
*Phase: 15-subscription-protocol*
*Completed: 2026-04-03*
