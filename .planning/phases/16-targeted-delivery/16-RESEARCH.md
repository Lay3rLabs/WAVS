# Phase 16: Targeted Delivery - Research

**Researched:** 2026-04-03
**Domain:** Replacing Recipients::All with targeted Recipients::Some on the P2P direct channel for per-service delivery
**Confidence:** HIGH

## Summary

Phase 16 is the payoff phase for the v1.3 milestone. Phases 14 and 15 built the infrastructure: `PeerSubscriptionMap` with `get_recipients()` and `has_announced()` methods, subscription announcement protocol wired into both bridge loops, heartbeat piggybacking for eventual consistency, and hello-on-first-contact for immediate sync. Phase 16 consumes these building blocks to replace `Recipients::All` with `Recipients::Some(service_peers)` on the direct channel (channel 1) in the Publish handler and retry queue drain paths.

The scope is precisely bounded: change 4 `direct_sender.send(Recipients::All, ...)` call sites in each bridge loop (8 total across both loops) to use targeted recipients from `peer_subscriptions.get_recipients()`. The Engine channel (channel 0) stays `Recipients::All` unconditionally per the locked architectural decision. Subscription announcement sends and heartbeat probes also stay `Recipients::All` -- they are control messages that must reach all peers. The retry queue drain must re-resolve recipients from `peer_subscriptions` at drain time, not use stale cached recipient sets.

This phase also resolves the existing `dead_code` compiler warnings for `has_announced()` and `get_recipients()` that have been flagged since Phase 14, because these methods are finally consumed by production code.

**Primary recommendation:** In the Publish handler, call `peer_subscriptions.get_recipients(&service_id.inner())` to get the targeted recipient set for `direct_sender.send()`. The `get_recipients()` method already returns `Recipients::All` as fallback when the subscriber set is empty (TGT-02). For COMPAT-03, no additional code is needed beyond what `get_recipients()` already does -- unknown peers (never announced) are not in `service_to_peers`, so `get_recipients()` returns `Recipients::All` when only unannounced peers exist. For retry queue drain, extract `service_id_bytes` from each queued `P2pMessage` and call `get_recipients()` fresh at drain time.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TGT-01 | Submissions on the direct channel (channel 1) use `Recipients::Some(service_peers)` instead of `Recipients::All` | Replace `Recipients::All` with `peer_subscriptions.get_recipients(&service_id.inner())` in the `direct_sender.send()` call within the Publish handler of both bridge loops (lines 836, 1311). The `get_recipients()` method already returns the correct `Recipients::Some(peers)` or `Recipients::All` fallback. |
| TGT-02 | When the subscriber set for a service is empty or a peer hasn't announced yet, the node falls back to `Recipients::All` | Already implemented in `PeerSubscriptionMap::get_recipients()` (line 450-455): returns `Recipients::All` when `service_to_peers` has no entry or an empty set. No additional code needed for the fallback. |
| TGT-03 | The broadcast Engine channel (channel 0) continues using `Recipients::All` for catch-up reliability | Do NOT change any `mailbox.broadcast(Recipients::All, ...)` calls. These stay at `Recipients::All` unconditionally. Only `direct_sender.send()` calls in the Publish and retry paths change. |
| TGT-04 | Retry queue messages re-resolve recipients at drain time (not cached from original send) | In the retry queue drain loop, call `peer_subscriptions.get_recipients(&queued_msg.service_id_bytes)` for each queued message. The `P2pMessage` stores `service_id_bytes`, enabling fresh recipient resolution per message. |
| COMPAT-01 | Existing secp256k1 e2e tests pass unchanged | The change is purely in the recipient selection for `direct_sender.send()`. The Engine channel remains `Recipients::All`, providing catch-up reliability. The `get_recipients()` fallback to `Recipients::All` ensures messages are delivered even without subscription state. E2E tests should pass without modification. |
| COMPAT-02 | Existing BLS e2e tests pass unchanged | Same reasoning as COMPAT-01. The targeted delivery is a bandwidth optimization on channel 1 only. Channel 0 (Engine) provides the reliability safety net. |
</phase_requirements>

## Standard Stack

### Core

No new dependencies. All code uses existing types and methods already present in p2p.rs.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `PeerSubscriptionMap::get_recipients()` | Phase 14 | Map service_id -> Recipients enum | Already built and tested; 7 unit tests cover it |
| `PeerSubscriptionMap::has_announced()` | Phase 15 | Check if peer has sent subscription data | Already built and tested; used for logging/observability only in this phase |
| `commonware-p2p::Recipients` | 2026.3.0 | `Recipients::All` / `Recipients::Some(Vec<P>)` enum | Already imported; `get_recipients()` returns this type directly |
| `std::collections::HashMap/HashSet` | stdlib | PeerSubscriptionMap internals | Already in use |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `get_recipients()` returning Recipients enum | Manual Vec building + Recipients::Some constructor | `get_recipients()` already encapsulates the fallback logic; using it directly is simpler and tested |
| Re-resolving retry queue recipients | Caching recipients alongside P2pMessage in RetryQueue | Stale cache defeats TGT-04 requirement; re-resolution uses current subscription state |

## Architecture Patterns

### Recommended File Structure

All changes in one existing file:
```
packages/wavs/src/subsystems/aggregator/p2p.rs
```

No new files. No changes outside this file.

### Pattern 1: Channel-Specific Recipient Strategy

**What:** The two P2P channels have fundamentally different recipient strategies:
- **Channel 0 (Engine/mailbox):** Always `Recipients::All` -- reliability layer with catch-up caching
- **Channel 1 (direct_sender):** Targeted `Recipients::Some(service_peers)` for submissions, `Recipients::All` for control messages (announcements, heartbeat probes)

**When to use:** Every send call in the bridge loop must be categorized by channel and message type.

**Call site classification (lookup bridge loop -- discovery is identical):**

| Line | Channel | Message Type | Recipient Strategy | Change? |
|------|---------|-------------|-------------------|---------|
| 830 | 0 (mailbox) | Submission | `Recipients::All` | NO (TGT-03) |
| 836 | 1 (direct) | Submission | `peer_subscriptions.get_recipients(&service_id.inner())` | YES (TGT-01) |
| 859 | 0 (mailbox) | Retry submission | `Recipients::All` | NO (TGT-03) |
| 861 | 1 (direct) | Retry submission | `peer_subscriptions.get_recipients(&queued_msg.service_id_bytes)` | YES (TGT-04) |
| 888 | 1 (direct) | Subscribe announcement | `Recipients::All` | NO (control) |
| 904 | 1 (direct) | Unsubscribe announcement | `Recipients::All` | NO (control) |
| 1062 | 0 (mailbox) | Heartbeat probe | `Recipients::All` | NO (control) |
| 1064 | 1 (direct) | Heartbeat probe | `Recipients::All` | NO (control) |
| 1078 | 0 (mailbox) | Heartbeat retry submission | `Recipients::All` | NO (TGT-03) |
| 1080 | 1 (direct) | Heartbeat retry submission | `peer_subscriptions.get_recipients(&queued_msg.service_id_bytes)` | YES (TGT-04) |
| 1099 | 1 (direct) | Heartbeat subscription announcement | `Recipients::All` | NO (control) |

**Discovery bridge loop (lines 1306-1569):** Identical pattern, same 3 YES sites per loop.

**Total changes: 6 direct_sender.send() calls (3 per bridge loop) change from `Recipients::All` to targeted.**

### Pattern 2: Recipient Resolution at Send Time

**What:** For each `direct_sender.send()` that carries a submission, resolve recipients from `peer_subscriptions.get_recipients()` at the moment of sending. Never cache or pre-compute recipient sets.

**Why:** The subscription map is live state updated from inbound announcements. Between the time a message is queued and the time it is drained, the subscription state may have changed (peers added/removed services, peers disconnected). TGT-04 explicitly requires re-resolution at drain time.

**Example (Publish handler):**
```rust
Some(P2pCommand::Publish { service_id, submission }) => {
    match P2pMessage::from_submission(&service_id, &submission) {
        Ok(msg) => {
            // Channel 0: Engine always gets Recipients::All (TGT-03)
            let ack_rx = mailbox.broadcast(Recipients::All, msg.clone()).await;

            // Channel 1: Direct channel uses targeted recipients (TGT-01)
            let direct_recipients = peer_subscriptions.get_recipients(&service_id.inner());
            let encoded_bytes = Encode::encode(&msg);
            if let Err(e) = direct_sender.send(direct_recipients, encoded_bytes, false).await {
                tracing::warn!("Direct channel send failed: {:?}", e);
            }
            // ... rest of ack_rx handling unchanged ...
```

**Example (Retry queue drain):**
```rust
let queued = retry_queue.drain_all();
for queued_msg in queued {
    // Channel 0: Engine always gets Recipients::All (TGT-03)
    let _ = mailbox.broadcast(Recipients::All, queued_msg.clone()).await;
    // Channel 1: Re-resolve recipients at drain time (TGT-04)
    let retry_recipients = peer_subscriptions.get_recipients(&queued_msg.service_id_bytes);
    let queued_bytes = Encode::encode(&queued_msg);
    if let Err(e) = direct_sender.send(retry_recipients, queued_bytes, false).await {
        tracing::warn!("Direct channel retry send failed: {:?}", e);
    }
}
```

### Pattern 3: Tracing with Recipient Counts

**What:** Add `tracing::debug!` calls that log the recipient strategy used for each submission send. This is essential for debugging subscription state issues in production.

**Example:**
```rust
let direct_recipients = peer_subscriptions.get_recipients(&service_id.inner());
match &direct_recipients {
    Recipients::All => tracing::debug!(
        "Publishing to direct channel: Recipients::All (fallback) for service {}",
        const_hex::encode(service_id.inner()),
    ),
    Recipients::Some(peers) => tracing::debug!(
        "Publishing to direct channel: {} targeted peers for service {}",
        peers.len(),
        const_hex::encode(service_id.inner()),
    ),
    _ => {} // Recipients::One is not used in publish path
}
```

### Anti-Patterns to Avoid

- **Changing `mailbox.broadcast()` to use targeted recipients:** The Engine channel MUST remain `Recipients::All`. The Engine is the reliability layer with catch-up caching (TGT-03). Targeting the Engine channel breaks catch-up for late-joining peers (see Pitfall 4 in PITFALLS.md).
- **Caching recipients at publish time for retry:** The retry queue stores `P2pMessage` objects, not recipient sets. This is correct by design -- TGT-04 requires fresh resolution at drain time. Do not add a recipient cache to `RetryQueue`.
- **Changing subscription announcement recipients to targeted:** Subscription announcements (`SUBSCRIPTION_SENTINEL`) and heartbeat probes (`HEARTBEAT_SERVICE_ID`) MUST always use `Recipients::All`. They are control plane messages that need to reach every peer.
- **Adding new methods to PeerSubscriptionMap:** `get_recipients()` and `has_announced()` already exist and are tested. Do not add alternative lookup methods.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Service-to-peers lookup | Manual HashMap lookup + Recipients construction | `PeerSubscriptionMap::get_recipients()` | Already built in Phase 14 with empty-set fallback to `Recipients::All` |
| Empty set fallback | `if peers.is_empty() { Recipients::All } else { Recipients::Some(peers) }` | `get_recipients()` already does this | Encapsulated in the method, 7 unit tests cover it |
| Retry queue recipient resolution | Store `(P2pMessage, Recipients)` tuples | Extract `queued_msg.service_id_bytes` at drain time | TGT-04 requires fresh resolution, not cached |
| COMPAT-03 backward compat | Manual "if no announcement received, treat as all" logic | `get_recipients()` naturally returns `Recipients::All` for unknown services | Peers that never announce are not in `service_to_peers`, so the fallback fires automatically |

**Key insight:** Phase 16 is the smallest phase in v1.3. All the infrastructure is already built. The only code changes are replacing `Recipients::All` with `peer_subscriptions.get_recipients(...)` at 3 call sites per bridge loop (6 total), plus adding tracing for observability.

## Common Pitfalls

### Pitfall 1: Changing Engine Channel (Channel 0) Recipients

**What goes wrong:** Applying targeted recipients to `mailbox.broadcast()` (Engine channel) breaks catch-up for peers that join a service late. The Engine caches messages for catch-up delivery, but only sends cached messages to peers that were in the original recipient set at broadcast time.
**Why it happens:** Natural instinct to apply targeting consistently to both channels.
**How to avoid:** Only change `direct_sender.send()` calls. Leave all `mailbox.broadcast()` calls at `Recipients::All`. The pattern is clear: if the call is `mailbox.broadcast()`, it stays untouched. If it is `direct_sender.send()` carrying a submission, it gets targeted.
**Warning signs:** Compilation passes but E2E tests fail with quorum stalls.

### Pitfall 2: Forgetting One of the Three Retry Drain Sites Per Loop

**What goes wrong:** Each bridge loop has two retry drain sites: one in the Publish handler's ack_rx success path (line 857/1328) and one in the heartbeat tick handler (line 1075/1545). Both must be updated. Missing one means some retry messages use `Recipients::All` while others use targeted delivery.
**Why it happens:** The retry drain logic is duplicated in two places within each bridge loop.
**How to avoid:** Search for all `retry_queue.drain_all()` call sites. There are 2 per bridge loop, 4 total. All 4 drain sites have `direct_sender.send()` calls that must use `peer_subscriptions.get_recipients()`.
**Warning signs:** `cargo clippy` will not catch this -- it is a logic error. The E2E tests should catch inconsistent delivery but may not depending on timing. Manual code review is the primary check.

### Pitfall 3: Bridge Loop Duplication Divergence

**What goes wrong:** The same 3 changes must be applied identically to both `run_lookup_network` and `run_discovery_network`. If one loop is updated but not the other, behavior differs between local and remote P2P modes.
**Why it happens:** These functions are ~500 lines each with near-identical bridge loop structures.
**How to avoid:** Make changes in `run_lookup_network` first, then copy the exact same changes to `run_discovery_network`. Verify both modes with `cargo test -p wavs`.
**Warning signs:** Tests pass for one P2P mode but not the other. Since E2E tests may only exercise one mode, unit tests are the primary safety net.

### Pitfall 4: Overly Verbose Logging in Hot Path

**What goes wrong:** Adding `tracing::info!` or frequent logging in the publish path (which fires for every submission) creates log spam in production.
**Why it happens:** Natural desire for observability.
**How to avoid:** Use `tracing::debug!` (not `info!`) for per-submission recipient logging. The default `RUST_LOG` level is `info`, so debug-level logs are suppressed unless explicitly enabled. Only log at `info!` for exceptional cases (first-ever fallback to `Recipients::All` for a service, or significant changes in peer counts).
**Warning signs:** Log output is overwhelmed with per-submission messages.

## Code Examples

Verified patterns based on the current codebase at p2p.rs after Phase 15.

### Publish Handler -- Direct Channel Targeting (TGT-01)

```rust
// Source: Modification of p2p.rs line 836 (lookup) / line 1311 (discovery)
// BEFORE:
// if let Err(e) = direct_sender.send(Recipients::All, encoded_bytes, false).await {

// AFTER:
let direct_recipients = peer_subscriptions.get_recipients(&service_id.inner());
let encoded_bytes = Encode::encode(&msg);
if let Err(e) = direct_sender.send(direct_recipients, encoded_bytes, false).await {
    tracing::warn!("Direct channel send failed: {:?}", e);
}
```

### Retry Queue Drain -- Re-Resolution at Drain Time (TGT-04)

```rust
// Source: Modification of p2p.rs lines 857-864 (lookup Publish ack_rx path)
// Also lines 1075-1083 (lookup heartbeat path)
// And equivalent in discovery loop
// BEFORE:
// let queued_bytes = Encode::encode(&queued_msg);
// if let Err(e) = direct_sender.send(Recipients::All, queued_bytes, false).await {

// AFTER:
let retry_recipients = peer_subscriptions.get_recipients(&queued_msg.service_id_bytes);
let queued_bytes = Encode::encode(&queued_msg);
if let Err(e) = direct_sender.send(retry_recipients, queued_bytes, false).await {
    tracing::warn!("Direct channel retry send failed: {:?}", e);
}
```

### Full Publish Handler (for context)

```rust
Some(P2pCommand::Publish { service_id, submission }) => {
    match P2pMessage::from_submission(&service_id, &submission) {
        Ok(msg) => {
            // Channel 0: Engine always uses Recipients::All (TGT-03)
            let ack_rx = mailbox.broadcast(Recipients::All, msg.clone()).await;

            // Channel 1: Direct channel uses targeted recipients (TGT-01)
            let direct_recipients = peer_subscriptions.get_recipients(&service_id.inner());
            let encoded_bytes = Encode::encode(&msg);
            if let Err(e) = direct_sender.send(direct_recipients, encoded_bytes, false).await {
                tracing::warn!("Direct channel send failed: {:?}", e);
            }

            // Check Engine broadcast acknowledgment (unchanged)
            match ack_rx.await {
                Ok(recipients) if recipients.is_empty() => {
                    retry_queue.push(msg);
                    tracing::warn!("No peers received broadcast, queued for retry");
                }
                Ok(recipients) => {
                    let peer_hexes: Vec<String> = recipients
                        .iter()
                        .map(|pk| const_hex::encode(pk.as_ref()))
                        .collect();
                    *connected_peers_tracker.write().unwrap() = peer_hexes;
                    tracing::debug!("Broadcast delivered to {} peers", recipients.len());

                    // Flush retry queue with re-resolved recipients (TGT-04)
                    if !retry_queue.is_empty() {
                        let queued = retry_queue.drain_all();
                        for queued_msg in queued {
                            let _ = mailbox.broadcast(Recipients::All, queued_msg.clone()).await;
                            let retry_recipients = peer_subscriptions.get_recipients(&queued_msg.service_id_bytes);
                            let queued_bytes = Encode::encode(&queued_msg);
                            if let Err(e) = direct_sender.send(retry_recipients, queued_bytes, false).await {
                                tracing::warn!("Direct channel retry send failed: {:?}", e);
                            }
                        }
                    }
                }
                Err(_) => {
                    tracing::error!("Broadcast engine shut down");
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to create P2pMessage: {:?}", e);
        }
    }
}
```

### Heartbeat Retry Drain (TGT-04)

```rust
// Source: Modification of p2p.rs lines 1075-1083 (lookup heartbeat)
// Also lines 1545-1553 (discovery heartbeat)
if !retry_queue.is_empty() {
    let queued = retry_queue.drain_all();
    for queued_msg in queued {
        let _ = mailbox.broadcast(Recipients::All, queued_msg.clone()).await;
        // TGT-04: Re-resolve recipients at drain time
        let retry_recipients = peer_subscriptions.get_recipients(&queued_msg.service_id_bytes);
        let queued_bytes = Encode::encode(&queued_msg);
        if let Err(e) = direct_sender.send(retry_recipients, queued_bytes, false).await {
            tracing::warn!("Retry send failed: {:?}", e);
        }
    }
}
```

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework (cargo test) |
| Config file | `Cargo.toml` workspace |
| Quick run command | `cargo test -p wavs --lib -- p2p_broadcast_tests` |
| Full suite command | `cargo test -p wavs` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TGT-01 | `get_recipients` returns `Recipients::Some(peers)` for service with known subscribers | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_peer_subscription_map_forward_index` | Exists (Phase 14) |
| TGT-02 | `get_recipients` returns `Recipients::All` when subscriber set is empty | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_get_recipients_empty_fallback` | Exists (Phase 14) |
| TGT-02 | `get_recipients` returns `Recipients::All` for unknown service_id | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_get_recipients_empty_fallback` | Exists (Phase 14) |
| TGT-04 | Retry re-resolution: `get_recipients` uses current state (not cached) after `set_peer_subscriptions` changes | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_retry_re_resolution` | Wave 0 |
| TGT-03 | Engine channel uses Recipients::All (verified by code review, not runtime test) | manual | Code review: all `mailbox.broadcast()` calls use `Recipients::All` | N/A |
| COMPAT-01 | Existing secp256k1 e2e tests pass unchanged | e2e | `cargo test -p layer-tests` | Exists |
| COMPAT-02 | Existing BLS e2e tests pass unchanged | e2e | `cargo test -p layer-tests` | Exists |

### Sampling Rate
- **Per task commit:** `cargo test -p wavs --lib -- p2p_broadcast_tests`
- **Per wave merge:** `cargo test -p wavs`
- **Phase gate:** Full `cargo test -p wavs` green + `cargo test -p layer-tests` (E2E) before verification

### Wave 0 Gaps
- [ ] `test_retry_re_resolution` -- verifies that calling `get_recipients` before and after `set_peer_subscriptions` returns different results (proves re-resolution works). Covers TGT-04 data structure behavior.
- No new test files or framework installs needed
- Existing 32 tests in `p2p_broadcast_tests` provide regression safety
- Note: The actual bridge loop integration (that `direct_sender.send()` receives targeted recipients) cannot be unit tested without mocking the commonware P2P network. E2E tests (`cargo test -p layer-tests`) provide this coverage. The unit tests verify the data structure methods return correct recipient sets.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| All direct_sender.send() use Recipients::All | Submission sends use Recipients::Some(service_peers) | Phase 16 (this phase) | Bandwidth optimization: peers only receive submissions for services they handle on channel 1 |
| Retry queue drain uses Recipients::All | Retry drain re-resolves recipients from current subscription state | Phase 16 (this phase) | Correct targeting for retried messages that may have been queued before subscription state converged |
| `get_recipients()` and `has_announced()` are dead_code | Both methods consumed by production code | Phase 16 (this phase) | Eliminates compiler warnings from Phase 14/15 |

## Open Questions

1. **Should we log at info! level the first time a service uses targeted delivery?**
   - What we know: Logging every submission at info! is too noisy. Logging nothing makes debugging hard.
   - What's unclear: Whether a one-time-per-service "now using targeted delivery for service X with N peers" info! log is valuable enough.
   - Recommendation: Claude's discretion. A `debug!` level log per submission with recipient count is sufficient. The Phase 17 `/p2p/status` endpoint will provide observability into subscription state.

## Project Constraints (from CLAUDE.md)

- **Build system:** `justfile`-based -- all test commands via `cargo test -p wavs`
- **Lint:** `cargo clippy --all-targets --all-features` with `-D warnings` (deny all warnings)
- **Format:** `cargo fmt` enforced
- **Test placement:** Inline `#[cfg(test)] mod tests { ... }` modules within Rust source files
- **Naming:** `snake_case` for functions, variables, module names; `PascalCase` for types and structs; `SCREAMING_SNAKE_CASE` for constants
- **Error handling:** Use `Result<T, E>` for fallible operations
- **Comments:** Three-slash doc comments (`///`) for public items; explain the "why" not the "what"
- **Logging:** `tracing` macros (`info!`, `warn!`, `debug!`, `trace!`); structured fields
- **Module visibility:** `pub(crate)` for items used within the crate but not exported; `pub` only for true public API
- **GSD Workflow:** Follow GSD workflow for all changes
- **Dead code:** Phase 16 resolves existing `dead_code` warnings for `has_announced` and `get_recipients` by consuming them

## Sources

### Primary (HIGH confidence)
- `packages/wavs/src/subsystems/aggregator/p2p.rs` (2662 lines) -- complete source analysis post-Phase 15: PeerSubscriptionMap (lines 357-456), get_recipients (line 450-455), has_announced (line 422-424), both bridge loops (lookup 631-1107, discovery 1125-1577), Publish handlers (lines 826-876, 1306-1348), retry queue drain sites (lines 856-865, 1075-1084, 1328-1337, 1545-1554)
- `.planning/phases/14-subscription-data-structures/14-01-SUMMARY.md` -- Phase 14 deliverables: PeerSubscriptionMap, get_recipients with Recipients::All fallback, 11 unit tests
- `.planning/phases/15-subscription-protocol/15-02-SUMMARY.md` -- Phase 15 deliverables: peer_subscriptions populated in both bridge loops, 32 total tests passing
- `.planning/research/PITFALLS.md` -- Pitfalls 1, 4, 8 directly relevant: race conditions, dual-channel divergence, empty recipients
- `cargo check -p wavs` output -- confirmed `has_announced` and `get_recipients` are currently dead_code (Phase 16 resolves this)

### Secondary (MEDIUM confidence)
- `.planning/REQUIREMENTS.md` -- TGT-01 through TGT-04, COMPAT-01, COMPAT-02 requirement definitions
- `.planning/STATE.md` -- Accumulated decisions: Engine=All permanently, only channel 1 gets targeting
- `.planning/research/STACK.md` -- Recipients enum confirmed: `All`, `Some(Vec<P>)`, `One(P)`

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- zero new dependencies; all methods already exist and are tested (7 unit tests for get_recipients, 4 for has_announced)
- Architecture: HIGH -- exact call sites identified by line number; change pattern is mechanical (replace Recipients::All with get_recipients call)
- Pitfalls: HIGH -- 4 phase-specific pitfalls from code analysis cross-referenced with PITFALLS.md research

**Research date:** 2026-04-03
**Valid until:** 2026-05-03 (stable -- pure bridge loop modifications, no external API dependencies)
