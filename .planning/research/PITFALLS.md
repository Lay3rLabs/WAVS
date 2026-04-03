# Domain Pitfalls

**Domain:** Adding per-service P2P targeting to existing broadcast-all system (commonware-p2p)
**Researched:** 2026-04-03
**Previous research:** 2026-03-23 (v1.2 Tauri desktop app pitfalls), 2026-03-17 (v1.0/v1.1 commonware migration)

This document covers pitfalls specific to the **v1.3 per-service P2P targeting milestone**: replacing `Recipients::All` with `Recipients::Some(service_peers)` for targeted delivery, adding a subscription announcement protocol, scoping catch-up per service, and maintaining backward compatibility with existing broadcast-all services.

## Critical Pitfalls

Mistakes that cause message loss, quorum failures, or require architectural rewrites.

### Pitfall 1: Subscription State Race Between Service Add and In-Flight Messages

**What goes wrong:** A peer adds a new service (triggering a subscription announcement) while messages for that service are already in flight from other peers. The current system uses `Recipients::All` so every message reaches every peer regardless of subscription timing. When switching to `Recipients::Some(service_peers)`, there is a window between:
1. Peer A starts the service and begins execution
2. Peer A broadcasts its subscription announcement
3. Other peers receive the announcement and add A to their `service_id -> Set<PeerPubkey>` map
4. Other peers begin sending to A via `Recipients::Some`

During steps 1-3, other peers may have already completed execution and sent their submissions via `Recipients::Some` -- but Peer A was not yet in their peer set for that service. Peer A misses those submissions. The quorum queue on Peer A stalls because it never receives enough signatures.

**Why it happens:** The existing system avoids this entirely because `Recipients::All` ensures every peer gets every message. The ServiceRouter filters locally. With targeted send, the sender must know the receiver's subscriptions *before* sending. This creates a distributed consensus problem: "who is subscribed to what?" with no strong consistency guarantee.

**Consequences:** Missed submissions lead to quorum stalls. The aggregator on the late-joining peer accumulates only its own signature and waits indefinitely for quorum (or until the quorum timeout fires). In a 3-of-5 quorum, if 2 peers add a service slightly late, they miss each other's first round of submissions entirely.

**Prevention:**
- **Keep `Recipients::All` as fallback for the Engine (channel 0)**. The broadcast Engine's catch-up mechanism caches messages globally per-peer in a deque. If you change the Engine channel to `Recipients::Some`, the Engine will not cache messages for peers that were not in the recipient set at broadcast time. Those messages are lost for catch-up forever. Only apply targeted send to the direct channel (channel 1). The Engine channel must remain `Recipients::All` so that its deque-based catch-up can deliver messages to any peer that reconnects or joins late.
- **Announce-then-wait pattern**: After sending a subscription announcement, do not immediately start publishing. Wait for at least one heartbeat round-trip (2 seconds in the current config) to allow peers to update their maps before the first trigger fires.
- **Receiver-side filter keeps working**: Even with `Recipients::All` on the Engine channel, the receiver-side ServiceRouter still filters. This means the fallback is correct -- peers only process messages for services they run. The targeting on channel 1 is a bandwidth optimization, not a correctness requirement.

**Detection:** Monitor quorum queue depth per service. If a service consistently has queues with only 1 signature (self), it suggests peers are not receiving targeted sends. Log `Recipients::Some` send results -- if `sent_to` count is lower than expected peer count, subscription state is stale.

**Phase mapping:** Must be the first thing addressed. The subscription protocol design determines whether targeted send is safe to enable.

### Pitfall 2: Broadcast Engine Global Deque Incompatible with Per-Service Catch-Up

**What goes wrong:** The commonware-broadcast `Engine` maintains a single global deque per peer (bounded by `deque_size`, default 128). When a peer reconnects, the Engine replays all cached messages from that peer's deque -- regardless of which service they belong to. The v1.3 goal of "per-service catch-up scoping" (reconnecting peers only replay messages for their subscribed services) is fundamentally at odds with the Engine's architecture.

The Engine's `insert_message` method (verified from source at `commonware-broadcast-2026.3.0/src/buffered/engine.rs`) caches messages by digest in a per-peer `VecDeque<Digest>` with a global `BTreeMap<Digest, M>` for the actual messages. There is no concept of "service" or "topic" in the Engine -- it just sees `P2pMessage` objects and their digests. When a peer reconnects, the Engine re-broadcasts all cached messages to that peer, including messages for services the peer does not subscribe to.

**Why it happens:** The Engine was designed as a general-purpose broadcast caching layer. It does not inspect message content. Adding per-service awareness would require forking commonware-broadcast or adding an application-level interception layer between the Engine and the network.

**Consequences:**
1. **Bandwidth waste on reconnect**: A peer subscribing to 1 of 10 services still receives all 128 cached messages on reconnect, 90% of which are filtered by ServiceRouter. For large messages (BLS signatures are ~300+ bytes per submission), this multiplies reconnection bandwidth by 10x.
2. **False catch-up completion**: The Engine replays all 128 messages, but if 120 were for other services, the peer effectively only catches up on 8 messages for its service. If the service had 50 messages while the peer was disconnected, 42 are still missing. The operator sees "caught up" (Engine deque drained) but quorum queues are incomplete.
3. **Deque eviction bias**: High-traffic services push out low-traffic service messages from the shared 128-slot deque. A peer running a niche service alongside a high-frequency service may never catch up on the niche service because its messages were evicted before the peer reconnected.

**Prevention:**
- **Do NOT attempt to modify the Engine for per-service scoping in v1.3.** This is an upstream change to commonware-broadcast that would be fragile and out of scope.
- **Accept that catch-up remains global for now.** Document this as a known limitation. The ServiceRouter already filters irrelevant catch-up messages, so correctness is maintained -- only bandwidth efficiency is affected.
- **Increase `deque_size` proportionally to service count.** If operators are expected to run N services, recommend `deque_size = 128 * N` or similar scaling. Add a note in the P2P config documentation.
- **Separate per-service catch-up as a future milestone.** The right solution is either: (a) upstream enhancement to commonware-broadcast with topic-aware caching, or (b) a WAVS-level catch-up protocol that queries specific peers for missed messages by service ID. Neither belongs in v1.3.
- **Track catch-up gaps per service**: Add metrics tracking "messages received via catch-up" per service to detect when deque eviction is causing data loss.

**Detection:** After reconnection, compare quorum queue completeness per service against expected peer count. If a service's quorum queue has fewer signatures than expected peers, catch-up likely missed messages due to deque eviction.

**Phase mapping:** This should be addressed in the research/planning phase as a scope limitation. Do not attempt per-service catch-up scoping in v1.3 -- document it as a v1.4+ goal.

### Pitfall 3: Subscription Announcement Delivery Not Guaranteed

**What goes wrong:** The subscription protocol needs peers to announce which services they run. These announcements travel over the same P2P channels as regular messages. If an announcement is lost (peer temporarily disconnected, message dropped by rate limiter, direct channel send failure), the sender believes it announced but other peers never update their `service_id -> Set<PeerPubkey>` map. The sender is silently excluded from targeted sends for that service.

Unlike regular submissions which have quorum-based retry logic, subscription announcements are metadata about the peer itself. There is no quorum check for "did everyone receive my subscription?" The peer has no way to verify that all peers have the correct subscription state.

**Why it happens:** The current P2P system treats all messages as fire-and-forget at the application level (the Engine and direct channel both drop messages for offline recipients). Heartbeat messages (`HEARTBEAT_SERVICE_ID`) probe connectivity but do not carry subscription state. A subscription announcement that fails on one peer creates an asymmetric view: peer A thinks peers B/C/D know about its subscription, but peer C missed the announcement and never sends to A for that service.

**Consequences:** Silent message loss. Peer A runs a service, signs submissions correctly, but never receives submissions from peer C (which uses `Recipients::Some` that excludes A). Peer A's quorum queue for that service has N-1 signatures instead of N, potentially below quorum threshold. The operator sees "quorum not reached" errors with no indication that the root cause is a stale subscription map.

**Prevention:**
- **Periodic re-announcement**: Do not rely on a single announcement. Piggyback subscription state on heartbeat messages. Every 2-second heartbeat already probes the mesh -- extend the heartbeat payload to include the sender's subscribed service IDs. This makes subscription state eventually consistent even if individual announcements are lost. The HEARTBEAT_SERVICE_ID sentinel already exists and is filtered by ServiceRouter, so extending heartbeat payload is backward-compatible.
- **Heartbeat-as-subscription-sync**: When a peer receives a heartbeat with subscription data, it updates its `service_id -> Set<PeerPubkey>` map. Stale entries (peer no longer subscribed) are cleaned up. This provides continuous consistency repair.
- **Log subscription map mismatches**: When `Recipients::Some` returns fewer `sent_to` peers than the subscription map suggests, log a warning. This detects cases where a peer is in the map but no longer connected.
- **Fallback to `Recipients::All` if subscription map is empty**: If a peer has no subscription data from other peers yet (fresh start, all announcements lost), fall back to `Recipients::All` rather than sending to nobody. This prevents total communication failure during subscription protocol bootstrap.

**Detection:** Compare the subscription map size per service against the connected_peers count from heartbeat. If `subscribed_peers(service_id).len()` is consistently less than `connected_peers.len()` for a service that all peers should run, subscription announcements are being lost.

**Phase mapping:** Must be addressed in the subscription protocol design phase. The heartbeat-based sync approach should be the default implementation.

### Pitfall 4: Dual-Channel Divergence When One Channel Gets Recipients::Some and the Other Gets Recipients::All

**What goes wrong:** The current publish path sends every message on BOTH channels:
- Channel 0 (Engine): `mailbox.broadcast(Recipients::All, msg.clone()).await`
- Channel 1 (direct): `direct_sender.send(Recipients::All, encoded_bytes, false).await`

When switching to targeted send, the obvious approach is changing both to `Recipients::Some(service_peers)`. But this creates a subtle bug: the Engine (channel 0) caches messages per-peer and uses `Recipients::All` to resolve which peers to send to via the `Connected` trait (Oracle). If you pass `Recipients::Some(subset)`, the Engine only sends to that subset, but still caches the message as coming from the local peer. When a peer NOT in the original subset reconnects, the Engine may relay the cached message to them during catch-up (because the Engine's deque is keyed by sender peer, not by original recipients). This creates inconsistent delivery: some peers get the message only on channel 0 catch-up but not channel 1 direct, leading to deduplication-set mismatches and potential double-processing.

**Why it happens:** The Engine's cache is sender-keyed, not recipient-keyed. The `insert_message` method stores `(peer, digest)` tuples regardless of who was in the original recipient set. The catch-up replay sends to ALL connected peers that the Engine knows about, not just the original recipients. This is by design (the Engine is a best-effort replication layer), but it means `Recipients::Some` on the Engine channel only affects the initial send, not catch-up replays.

**Consequences:**
- Peer X subscribes to service S after Peer Y already sent a submission via `Recipients::Some` that excluded X.
- X reconnects (or was temporarily disconnected).
- Engine catch-up replays Y's cached message to X on channel 0.
- X receives the message on channel 0 but its `seen_digests` set (channel 1 dedup) does not have this digest, so it is processed.
- Meanwhile, X's ServiceRouter accepts it (X is subscribed to service S now).
- This is actually CORRECT behavior -- the message reaches X via catch-up. But it masks a problem: if the subscription map was wrong, the operator has no way to distinguish "message arrived via targeted send" from "message arrived via catch-up fallback." Debugging subscription protocol issues becomes nearly impossible.

**Prevention:**
- **Keep Engine channel (channel 0) at `Recipients::All` always.** The Engine is the reliability layer. Its catch-up mechanism is the safety net. Do not restrict it. The bandwidth cost of `Recipients::All` on the Engine channel is minimal because the Engine only caches the digest + message once regardless of recipient count.
- **Apply `Recipients::Some` ONLY to the direct channel (channel 1).** This is the "fast path" optimization. If a peer is not in the targeted set, it misses the direct delivery but still gets the message via Engine catch-up when it next connects. This gives you bandwidth savings on channel 1 without breaking catch-up reliability on channel 0.
- **If you must target both channels**, accept that catch-up will "leak" messages to non-targeted peers. Document this as expected behavior, not a bug.

**Detection:** Track message delivery source (channel 0 catch-up vs channel 1 direct) per message. If a high percentage of messages for a service arrive via catch-up rather than direct, the subscription map for that service is likely stale.

**Phase mapping:** This is an architectural decision that must be made before any code changes. The "Engine=All, Direct=Some" split should be the recommended approach.

## Moderate Pitfalls

### Pitfall 5: Backward Compatibility -- Existing Services Break During Rolling Migration

**What goes wrong:** Existing deployed services (both secp256k1 and BLS) expect `Recipients::All` behavior. If the v1.3 update is deployed to some operators before others (rolling update), the updated operators start using `Recipients::Some` while the non-updated operators still use `Recipients::All` and have no subscription map. The updated operators send targeted messages to peers in their subscription map, but the non-updated operators are not in the map (they never sent subscription announcements). Messages from updated operators never reach non-updated operators on the direct channel.

**Why it happens:** The subscription protocol is a new wire protocol. Old nodes do not understand subscription announcements and do not send them. They do not appear in the subscription map. If the updated node switches to `Recipients::Some` for the direct channel, old nodes are excluded.

**Consequences:** Quorum breaks during rolling update. If 3-of-5 operators update and 2 do not, the 3 updated operators only target each other, and the 2 old operators only receive via Engine catch-up (if Engine stays at `Recipients::All`). If Engine is also targeted, the old operators are completely isolated. The service becomes non-functional until all operators update.

**Prevention:**
- **`Recipients::All` remains the default for peers without subscription data.** If a connected peer has not sent any subscription announcements, treat it as subscribed to ALL services (backward-compatible assumption). Only use `Recipients::Some` when you have positive evidence of a peer's subscriptions from received announcements.
- **Feature flag**: Add a config option `p2p.targeted_send_enabled = true/false` (default false). Operators opt in to targeted send only after confirming all peers in their network have updated. This allows deployment without behavior change.
- **Version negotiation**: Include a protocol version in the heartbeat payload. If a peer's heartbeat does not include subscription data (old protocol), assume it subscribes to everything.
- **Never remove a peer from "all services" unless it explicitly says so.** The subscription map should be additive: start with "this peer is subscribed to everything" and narrow down only based on explicit announcements.

**Detection:** Monitor broadcast `sent_to` counts. If `sent_to` drops after a deployment (e.g., from 4 to 2 peers), the non-updated peers are being excluded. Alert on `sent_to < expected_peer_count`.

**Phase mapping:** Must be addressed in the initial protocol design. The backward-compatibility assumption (unknown peers = subscribed to all) is a Phase 1 requirement.

### Pitfall 6: Subscription Map Memory and Staleness

**What goes wrong:** The `service_id -> Set<PeerPubkey>` map grows without bounds as services are added and never cleaned up. If a peer crashes and restarts with a different set of services, its old subscriptions remain in other peers' maps until they explicitly hear an unsubscription. With dynamic service add/remove, the map can accumulate stale entries.

**Why it happens:** The current ServiceRouter is local-only (`HashSet<[u8; 32]>` of subscribed services on this node). It has no concept of remote peer subscriptions. The new subscription map is a distributed data structure that must be maintained across all peers. There is no garbage collection mechanism.

**Consequences:**
- Stale entries: Peer A removes service S, but peer B's map still shows A as subscribed to S. B sends targeted messages to A for service S. A receives them but ServiceRouter rejects them. Wasted bandwidth.
- Memory leak: Over time with many services being added and removed, the map grows. Each entry is 32 bytes (service_id) + 32 bytes (peer pubkey) = 64 bytes. At 1000 services x 100 peers = 6.4 MB. Not catastrophic, but grows unboundedly.
- Incorrect peer counts: `get_status()` reports subscription counts including stale peers, misleading operators.

**Prevention:**
- **Heartbeat-based full subscription sync**: As recommended in Pitfall 3, piggyback the full subscription list on heartbeats. When a heartbeat arrives, REPLACE the peer's subscription set rather than merging. This provides self-healing: if peer A removes service S and sends a heartbeat without S, all peers automatically drop A from service S's subscriber set.
- **Subscription TTL**: If no heartbeat with subscription data is received from a peer within 3 heartbeat intervals (6 seconds), remove all of that peer's subscriptions. This handles the crash-without-unsubscribe case.
- **Bound the map**: Cap at `MAX_SERVICES_PER_PEER` (e.g., 256) and `MAX_PEERS_PER_SERVICE` (e.g., 128). Reject subscription announcements that would exceed these limits.

**Detection:** Log subscription map size periodically. Alert if map size exceeds expected bounds.

**Phase mapping:** Addressed as part of the subscription protocol implementation. The heartbeat-based full-sync approach prevents staleness by design.

### Pitfall 7: Testing Multi-Node Subscription Coordination is Fundamentally Harder

**What goes wrong:** The existing P2P tests (in `packages/wavs/tests/p2p_broadcast_tests.rs`) verify message delivery between 2 nodes with pre-configured subscriptions. Testing per-service targeting requires:
1. Dynamic subscription changes during test execution
2. Verifying that targeted sends exclude non-subscribed peers
3. Testing subscription protocol convergence (multiple announcement rounds)
4. Verifying catch-up correctness after subscription changes
5. Testing the rolling update scenario (mixed old/new protocol peers)

The current test infrastructure (`setup_two_nodes`) creates nodes with static configurations. There is no mechanism to change subscriptions during a test run and verify the effect on message delivery.

**Why it happens:** `P2pHandle::subscribe()` sends a command to the P2P bridge loop, which updates the local ServiceRouter. But the ServiceRouter update is local -- there is no mechanism for node A to discover that node B has subscribed to a service. The subscription protocol (announcement messages) is the new thing that must be tested, and it requires observing state changes across multiple nodes.

**Consequences:**
- Tests pass with 2 nodes but fail with 3+ nodes due to subscription race conditions not reproduced in 2-node tests.
- Test flakiness due to timing-dependent subscription convergence.
- Missing test coverage for stale subscription cleanup, rolling updates, and partial announcement delivery.

**Prevention:**
- **Extend `setup_two_nodes` to `setup_n_nodes` with parameterized subscriptions.** The helper should support creating N nodes with configurable initial subscriptions and the ability to change subscriptions during the test.
- **Add a `wait_for_subscription_convergence` helper**: After changing subscriptions, poll `get_status()` on all nodes until they all report consistent subscription maps (or timeout). This eliminates timing-dependent flakiness.
- **Test the negative case explicitly**: Verify that a peer NOT subscribed to service X does NOT receive targeted messages for service X on channel 1 (it may still receive via Engine catch-up on channel 0, which is expected).
- **Use `tokio::time::pause()` for deterministic timing**: The existing tests use `tokio::time::sleep(Duration::from_secs(5))` for connection establishment. This makes tests slow and flaky. Consider using Tokio's time-mocking facilities for subscription convergence testing.
- **Test 3-node minimum for subscription coordination**: 2-node tests cannot reproduce "peer C excludes peer A because C thinks A is not subscribed." The minimum for meaningful subscription protocol testing is 3 nodes.

**Detection:** Test failure rate on CI. If P2P subscription tests fail > 5% of runs, the test timing assumptions are wrong.

**Phase mapping:** Test infrastructure should be extended in the same phase as the subscription protocol implementation. Tests and implementation evolve together.

### Pitfall 8: `Recipients::Some` with Empty Vec Silently Drops Messages

**What goes wrong:** If the subscription map returns an empty set for a service (no peers known to be subscribed), `Recipients::Some(vec![])` is passed to `direct_sender.send()`. The commonware-p2p `send()` implementation for an empty recipient list returns `Ok(vec![])` (no error, zero recipients). The message is silently dropped. There is no retry, no warning, no fallback to `Recipients::All`.

**Why it happens:** The `Sender::send` trait implementation in commonware-p2p handles `Recipients::Some` by iterating over the provided peers and sending to each. An empty vec means zero iterations, zero sends, and a successful return. This is technically correct behavior for the P2P layer, but the application layer should never construct an empty recipient set.

**Consequences:** Messages for newly deployed services (where no peer has announced subscriptions yet) are silently lost. The operator sees successful publish (no error returned) but no peer receives the message. Quorum is never reached. The service appears broken with no error logs.

**Prevention:**
- **Never construct `Recipients::Some(vec![])`.** Add an assertion or fallback: if `service_peers.is_empty()`, use `Recipients::All` instead of `Recipients::Some(vec![])`.
- **Treat empty subscriber set as "subscription protocol not yet converged"**: Fall back to broadcast. This is the safe default.
- **Add a `warn!` log**: If the subscription map lookup returns an empty set for a service that this node is publishing for, log a warning. The node is running the service, so at minimum it should be in its own subscriber set.
- **Include self in the subscriber set**: When looking up peers for `Recipients::Some`, always include the local peer's pubkey if the local node subscribes to that service. This ensures at least one recipient (self is always available for local loopback through the aggregator's `Receive` path, but the P2P layer does not deliver to self).

**Detection:** Log `Recipients::Some` recipient count per publish. Alert if count is 0.

**Phase mapping:** Must be a defensive check in the publish path. Add in the same phase as the targeted send implementation.

## Minor Pitfalls

### Pitfall 9: Heartbeat Messages Become Bandwidth-Heavy with Subscription Data

**What goes wrong:** The current heartbeat is a minimal `P2pMessage` with `service_id_bytes: [0u8; 32]` and `payload: vec![]` (32 bytes + overhead). If heartbeats carry the full subscription list (N service IDs x 32 bytes each), the heartbeat size grows to 32 + (N * 32) bytes. At 10 services, that is 352 bytes. At 100 services, 3.2 KB. With heartbeats every 2 seconds to every peer, the bandwidth is: `100 services * 3.2 KB * 5 peers * 0.5 Hz = 8 KB/s`. Not catastrophic but noticeable.

**Prevention:**
- **Use a compact representation**: Send a Bloom filter or bitmap of subscribed service IDs instead of the full list. A 256-bit bitmap can represent 256 services with only 32 bytes of overhead.
- **Delta encoding**: Only send changes since the last heartbeat. A counter + diff is more compact than the full list.
- **Separate cadence**: Subscription sync does not need to be as frequent as heartbeat. Send subscription state every 10th heartbeat (every 20 seconds) instead of every heartbeat. The initial announcement covers the first sync; heartbeat-carried state is just consistency repair.

**Phase mapping:** Optimization. Can be addressed after the basic protocol works. Start with the simple "full list in every heartbeat" approach and optimize if bandwidth becomes an issue.

### Pitfall 10: Deduplication Set (`seen_digests`) Does Not Distinguish Channel Source

**What goes wrong:** The current deduplication uses a `HashSet<sha256::Digest>` with a cap of 1024 entries. When a message arrives on channel 1 (direct), its digest is added to `seen_digests`. If the same message later arrives via Engine catch-up on channel 0 (e.g., because channel 1 delivery was delayed), it is deduplicated and dropped. This is correct behavior. However, if the "Engine=All, Direct=Some" strategy is adopted (Pitfall 4 recommendation), a peer that is NOT in the `Recipients::Some` set will only receive the message via Engine catch-up. It has no prior entry in `seen_digests` for this message. The first channel 0 delivery is accepted, which is correct. But if the Engine replays the same message multiple times during catch-up (reconnect cycles), `seen_digests` prevents double processing, which is also correct. No actual bug here -- but the dedup set must be sized to handle the increased traffic from Engine catch-up messages that would not have existed under pure `Recipients::All`.

**Prevention:** Monitor `seen_digests` set size. If it hits `MAX_SEEN_DIGESTS = 1024` frequently, increase the cap. With per-service targeting, the direct channel delivers fewer messages, but the Engine channel may replay more during catch-up, so the total message rate through dedup does not decrease.

**Phase mapping:** Low priority. Monitor after deployment. Increase cap if needed.

### Pitfall 11: P2pHandle API Does Not Expose Subscription Map for Observability

**What goes wrong:** The current `P2pHandle::get_status()` returns `P2pStatus` with `subscribed_services` (local node's subscriptions) and `connected_peers` (count of connected peers). It does not expose the subscription map (`service_id -> Set<PeerPubkey>`). Operators cannot see which peers are subscribed to which services. When quorum stalls due to subscription state issues, there is no diagnostic tool.

**Prevention:**
- Extend `P2pStatus` to include `peer_subscriptions: HashMap<String, Vec<String>>` (service_id_hex -> list of peer_id_hex).
- Expose this in the `/p2p/status` HTTP endpoint and the Tauri desktop app's P2P page.
- Add a `P2pCommand::GetSubscriptionMap` variant that returns the full map.

**Phase mapping:** Should be implemented alongside the subscription protocol. Observability enables debugging.

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Subscription protocol design | Pitfall 1 (race), Pitfall 3 (lost announcements), Pitfall 5 (backward compat) | Heartbeat-based full sync, unknown peers = subscribed to all |
| Targeted send implementation | Pitfall 4 (dual-channel divergence), Pitfall 8 (empty recipients) | Engine=All / Direct=Some split, fallback on empty subscriber set |
| Per-service catch-up | Pitfall 2 (Engine incompatibility) | Defer to v1.4+, increase deque_size as interim measure |
| Subscription lifecycle (add/remove) | Pitfall 6 (staleness), Pitfall 1 (race) | Heartbeat-based full-sync with TTL, replace-not-merge on update |
| Testing | Pitfall 7 (multi-node testing) | 3-node minimum, subscription convergence helper, negative case tests |
| Migration / rolling update | Pitfall 5 (backward compat) | Feature flag, version negotiation, default-to-all for unknown peers |
| Observability | Pitfall 11 (hidden subscription state) | Extend P2pStatus, expose subscription map in API |
| Performance | Pitfall 9 (heartbeat bandwidth) | Start simple, optimize later with bloom filters or delta encoding |

## Sources

- commonware-p2p 2026.3.0 source: `Recipients` enum definition at `src/lib.rs` lines 40-46 (`All`, `Some(Vec<P>)`, `One(P)`)
- commonware-broadcast 2026.3.0 source: `Engine` cache implementation at `src/buffered/engine.rs` (per-peer deque, global digest cache, no topic awareness)
- commonware-p2p `Sender::send` trait: offline recipients silently dropped, empty recipient set returns `Ok(vec![])`
- WAVS P2P module: `packages/wavs/src/subsystems/aggregator/p2p.rs` (ServiceRouter, dual-channel broadcast, heartbeat, retry queue)
- WAVS P2P tests: `packages/wavs/tests/p2p_broadcast_tests.rs` (BCAST-01 through CATCH-02 test coverage, 2-node setup helper)
- [GossipSub specification](https://research.protocol.ai/blog/2019/a-new-lab-for-resilient-networks-research/PL-TechRep-gossipsub-v0.1-Dec30.pdf) -- per-topic subscription with mesh management provides reference for subscription protocol design
- [Gossip protocol split-brain considerations](https://en.wikipedia.org/wiki/Gossip_protocol) -- eventual consistency guarantees and split-brain scenarios in gossip-based systems
