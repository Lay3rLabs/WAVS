# Phase 14: Subscription Data Structures - Research

**Researched:** 2026-04-03
**Domain:** Rust data structures and codec patterns for P2P service subscription tracking
**Confidence:** HIGH

## Summary

Phase 14 creates the foundational data structures and wire format for per-service P2P targeting. This is a pure additive phase -- no existing behavior is modified. The deliverables are: (1) `PeerSubscriptionMap` with bidirectional indexing (`service_id -> Set<PeerPubkey>` forward + `PeerPubkey -> Set<service_id>` reverse), (2) `SubscriptionAnnouncement` message type using serde_json for payload encoding (matching the existing `P2pMessage` pattern), (3) the `SUBSCRIPTION_SENTINEL` constant (`[0xFF; 32]`) distinguishable from both real service IDs and the existing `HEARTBEAT_SERVICE_ID` (`[0x00; 32]`), and (4) a `get_recipients()` method that returns `Recipients::All` as a defensive fallback when the subscriber set is empty.

All new code lives in `packages/wavs/src/subsystems/aggregator/p2p.rs` alongside existing types (`P2pMessage`, `ServiceRouter`, `RetryQueue`). No new crate dependencies are needed. The `PeerSubscriptionMap` uses `std::collections::HashMap`/`HashSet` (the bridge loop is single-threaded, so no concurrent map is needed). The `SubscriptionAnnouncement` uses `serde` + `serde_json` for serialization (consistent with how `Submission` payloads are encoded in `P2pMessage`). The `ed25519::PublicKey` from commonware-cryptography 2026.3.0 implements `Clone, Eq, PartialEq, Ord, PartialOrd, Hash` -- confirmed from source -- so it works directly as a `HashMap`/`HashSet` key.

**Primary recommendation:** Build all four deliverables with comprehensive unit tests, following the exact patterns established by the existing `P2pMessage`, `ServiceRouter`, and `RetryQueue` types and their test module `p2p_broadcast_tests`. The `ServiceRouter::subscribed_services_raw()` accessor returns `Vec<[u8; 32]>` for later use by heartbeat subscription sync (Phase 15). No bridge loop modifications in this phase.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SUB-01 | Node maintains a per-service peer subscription map (`service_id -> Set<PeerPubkey>`) updated from announcement messages | `PeerSubscriptionMap.service_to_peers` forward index; `handle_announcement()` method processes subscribe/unsubscribe lists |
| SUB-02 | Node maintains a reverse index (`PeerPubkey -> Set<service_id>`) for efficient cleanup on peer disconnect | `PeerSubscriptionMap.peer_to_services` reverse index; `remove_peer()` method clears all entries for a given peer from both maps |
| SUB-03 | When a peer disconnects, all its subscription entries are removed from both maps | `remove_peer(&ed25519::PublicKey)` method on `PeerSubscriptionMap`; iterates reverse index to find all service_ids, removes peer from each forward entry, then removes the reverse entry |
| ANN-05 | Subscription announcements use a sentinel service_id to multiplex on the existing direct channel (no new channels required) | `SUBSCRIPTION_SENTINEL: [u8; 32] = [0xFF; 32]` constant; `SubscriptionAnnouncement` encoded as JSON payload of a `P2pMessage` with sentinel as `service_id_bytes`; `is_subscription_announcement()` predicate function |
</phase_requirements>

## Standard Stack

### Core

No new dependencies. All code uses existing workspace crates.

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::collections::HashMap` | stdlib | Forward + reverse subscription indexes | Bridge loop is single-threaded; no concurrent access |
| `std::collections::HashSet` | stdlib | Per-service peer sets and per-peer service sets | Same thread safety reasoning |
| `commonware-cryptography` | 2026.3.0 | `ed25519::PublicKey` as peer identity key in subscription map | Already used throughout p2p.rs; implements Hash+Eq+Clone |
| `commonware-p2p` | 2026.3.0 | `Recipients::All` / `Recipients::Some(Vec<PublicKey>)` enum for get_recipients return type | Already imported; `Recipients::Some` verified from source at lib.rs lines 42-46 |
| `serde` + `serde_json` | workspace | Serialize `SubscriptionAnnouncement` to/from JSON bytes as P2pMessage payload | Matches existing pattern: `Submission` is JSON-serialized into `P2pMessage.payload` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `HashMap`/`HashSet` | `DashMap` | DashMap is already in workspace but unnecessary -- bridge loop is single-threaded |
| `serde_json` for announcement | `commonware-codec` Write/Read | Codec would be more compact but SubscriptionAnnouncement has variable-length lists; serde_json is simpler and matches existing P2pMessage payload encoding pattern |
| `[0xFF; 32]` sentinel | New `P2pEnvelope` enum wrapper | Envelope would be cleaner but breaks wire format compatibility; sentinel preserves existing P2pMessage structure |

## Architecture Patterns

### Recommended File Structure

All new code goes in the existing file:
```
packages/wavs/src/subsystems/aggregator/p2p.rs
```

New items are added between the existing `RetryQueue` section (line ~311) and the `Network Management` section (line ~313), following the established section-header pattern.

### Pattern 1: Bidirectional Index Map

**What:** `PeerSubscriptionMap` maintains two `HashMap`s kept in sync -- forward (service -> peers) and reverse (peer -> services). Every mutation updates both maps atomically.

**When to use:** When you need O(1) lookup in both directions and O(services_per_peer) cleanup on peer removal.

**Why:** Without the reverse index, removing a disconnected peer requires scanning every service's peer set -- O(total_services). With the reverse index, removal is O(services_that_peer_subscribed_to), which is typically much smaller.

**Example:**
```rust
// Source: .planning/research/ARCHITECTURE.md patterns + existing p2p.rs code patterns
pub(crate) struct PeerSubscriptionMap {
    /// Forward index: service_id -> set of subscribed peers
    service_to_peers: HashMap<[u8; 32], HashSet<ed25519::PublicKey>>,
    /// Reverse index: peer -> set of subscribed services (for disconnect cleanup)
    peer_to_services: HashMap<ed25519::PublicKey, HashSet<[u8; 32]>>,
}

impl PeerSubscriptionMap {
    pub fn new() -> Self {
        Self {
            service_to_peers: HashMap::new(),
            peer_to_services: HashMap::new(),
        }
    }

    /// Process a subscription announcement from a peer.
    /// Idempotent -- duplicate subscriptions are no-ops.
    pub fn handle_announcement(
        &mut self,
        peer: &ed25519::PublicKey,
        announcement: &SubscriptionAnnouncement,
    ) {
        for service_id in &announcement.subscribe {
            self.service_to_peers
                .entry(*service_id)
                .or_default()
                .insert(peer.clone());
            self.peer_to_services
                .entry(peer.clone())
                .or_default()
                .insert(*service_id);
        }
        for service_id in &announcement.unsubscribe {
            if let Some(peers) = self.service_to_peers.get_mut(service_id) {
                peers.remove(peer);
                if peers.is_empty() {
                    self.service_to_peers.remove(service_id);
                }
            }
            if let Some(services) = self.peer_to_services.get_mut(peer) {
                services.remove(service_id);
                if services.is_empty() {
                    self.peer_to_services.remove(peer);
                }
            }
        }
    }

    /// Remove all subscriptions for a disconnected peer (SUB-03).
    /// Uses the reverse index for efficient cleanup.
    pub fn remove_peer(&mut self, peer: &ed25519::PublicKey) {
        if let Some(services) = self.peer_to_services.remove(peer) {
            for service_id in services {
                if let Some(peers) = self.service_to_peers.get_mut(&service_id) {
                    peers.remove(peer);
                    if peers.is_empty() {
                        self.service_to_peers.remove(&service_id);
                    }
                }
            }
        }
    }

    /// Get the recipient set for targeted delivery.
    /// Returns Recipients::Some(peers) if peers are known, or Recipients::All as fallback.
    pub fn get_recipients(
        &self,
        service_id: &[u8; 32],
    ) -> Recipients<ed25519::PublicKey> {
        match self.service_to_peers.get(service_id) {
            Some(peers) if !peers.is_empty() => {
                Recipients::Some(peers.iter().cloned().collect())
            }
            _ => Recipients::All,
        }
    }
}
```

### Pattern 2: Sentinel-Based Message Discrimination

**What:** Reserved service_id_bytes values distinguish control messages from data messages on the same P2P channel, without introducing a new message envelope or breaking wire format.

**When to use:** Adding new message types to an existing P2P channel where wire format changes are undesirable.

**Why this pattern:** Already proven with `HEARTBEAT_SERVICE_ID = [0x00; 32]`. The ServiceRouter will never accept sentinel values (no real service hashes to all-zeros or all-0xFF), so control messages are automatically excluded from aggregator forwarding.

**Example:**
```rust
/// Existing heartbeat sentinel
const HEARTBEAT_SERVICE_ID: [u8; 32] = [0u8; 32];

/// New subscription announcement sentinel (ANN-05).
/// Distinguished from all-zeros heartbeat and from real service_id hashes (SHA-256).
/// A valid SHA-256 hash of any input is astronomically unlikely to be all-0xFF.
pub(crate) const SUBSCRIPTION_SENTINEL: [u8; 32] = [0xFF; 32];

/// Check if a P2pMessage is a subscription announcement.
fn is_subscription_announcement(msg: &P2pMessage) -> bool {
    msg.service_id_bytes == SUBSCRIPTION_SENTINEL
}
```

### Pattern 3: Announcement Type Matching Existing Payload Pattern

**What:** `SubscriptionAnnouncement` is serialized via `serde_json` and carried as the `payload` field of a `P2pMessage` (same as how `Submission` is carried).

**When to use:** When adding a new message type that reuses the existing P2pMessage wire format.

**Example:**
```rust
/// Subscription announcement carried as P2pMessage payload.
/// The service_id_bytes field is set to SUBSCRIPTION_SENTINEL.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(crate) struct SubscriptionAnnouncement {
    /// Services this peer is subscribing to
    pub subscribe: Vec<[u8; 32]>,
    /// Services this peer is unsubscribing from
    pub unsubscribe: Vec<[u8; 32]>,
}

impl SubscriptionAnnouncement {
    /// Encode as a P2pMessage with the subscription sentinel.
    pub fn to_p2p_message(&self) -> Result<P2pMessage, serde_json::Error> {
        let payload = serde_json::to_vec(self)?;
        Ok(P2pMessage {
            service_id_bytes: SUBSCRIPTION_SENTINEL,
            payload,
        })
    }

    /// Decode from a P2pMessage payload (caller must verify sentinel first).
    pub fn from_payload(payload: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(payload)
    }
}
```

### Pattern 4: ServiceRouter Extension

**What:** Add `subscribed_services_raw()` to return raw `[u8; 32]` values for constructing announcements.

**When to use:** Phase 15 needs the raw service ID bytes to build announcement payloads from the local node's service set.

**Example:**
```rust
impl ServiceRouter {
    /// Return raw bytes of subscribed service IDs (for building announcements).
    pub fn subscribed_services_raw(&self) -> Vec<[u8; 32]> {
        self.subscribed_services.iter().copied().collect()
    }
}
```

### Anti-Patterns to Avoid

- **Using `DashMap` for PeerSubscriptionMap:** The bridge loop is single-threaded (`tokio::select!`). No concurrent access exists. `DashMap` adds complexity and overhead for zero benefit.
- **Using `commonware-codec` derive for `SubscriptionAnnouncement`:** The codec's `Read` trait requires a `Cfg` type for variable-length fields, adding boilerplate. `serde_json` is simpler and matches how `Submission` payloads are already handled inside `P2pMessage`.
- **Introducing a `P2pEnvelope` wrapper:** This would break the wire format by changing how all messages are encoded. The sentinel pattern avoids this while achieving the same discrimination.
- **Making `PeerSubscriptionMap` `pub` or `Arc<RwLock<...>>`:** The map lives entirely within the bridge loop. Making it shared adds complexity that will never be needed -- the bridge loop is the only consumer and mutator.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Thread-safe map | Custom lock wrapper around HashMap | Not needed -- use plain HashMap | Bridge loop is single-threaded |
| Message type discrimination | New P2P channel or envelope format | Sentinel pattern on existing P2pMessage | Channels cannot be added dynamically; sentinel is proven |
| Peer identity key | Custom peer ID type | `ed25519::PublicKey` directly | Already implements Hash, Eq, Clone, Ord -- all traits needed for map keys |
| JSON serialization | Custom binary protocol | `serde_json` | Matches existing P2pMessage payload pattern exactly |

**Key insight:** Phase 14 deliverables are pure Rust data structures with no external dependencies beyond what already exists in the crate. The complexity is in the design (bidirectional indexing, sentinel discrimination, defensive fallback), not in the implementation.

## Common Pitfalls

### Pitfall 1: Empty Peer Set Silently Drops Messages
**What goes wrong:** `get_recipients()` returns `Recipients::Some(vec![])` when no peers are subscribed. commonware-p2p `send()` with empty recipients returns `Ok(vec![])` -- no error, no delivery.
**Why it happens:** A newly deployed service has zero subscription announcements initially. The forward index returns an empty set.
**How to avoid:** The `get_recipients()` implementation MUST check `!peers.is_empty()` and fall back to `Recipients::All` when the set is empty. This check is baked into the data structure, not left to callers.
**Warning signs:** If unit tests only test the non-empty case, this bug will ship undetected.

### Pitfall 2: Reverse Index Inconsistency After Partial Updates
**What goes wrong:** If `handle_announcement()` crashes or is interrupted between updating the forward and reverse indexes, the maps become inconsistent. A peer appears in one index but not the other.
**Why it happens:** The two maps are updated in sequence, not atomically.
**How to avoid:** In Rust, this is not a real risk -- panics unwind the entire function, and the bridge loop is single-threaded with no concurrent access. But tests should verify consistency: after any sequence of operations, the forward and reverse indexes should agree.
**Warning signs:** Tests that only check one index direction.

### Pitfall 3: Sentinel Collision with Real Service IDs
**What goes wrong:** A service_id that happens to be `[0xFF; 32]` would be misidentified as a subscription announcement.
**Why it happens:** Service IDs are SHA-256 hashes of service configuration. SHA-256 producing all-0xFF is astronomically unlikely (~1/2^256) but theoretically possible.
**How to avoid:** Document the sentinel as reserved. The probability is negligible (heat death of the universe before a collision), but the constant should be clearly documented as reserved and tested that ServiceRouter rejects it.
**Warning signs:** No explicit test verifying sentinel is distinguishable from real service IDs.

### Pitfall 4: SubscriptionAnnouncement Serialization Size
**What goes wrong:** A node with many services (e.g., 100) creates an announcement with 100 x 32-byte service IDs. The JSON encoding adds overhead (hex encoding in JSON would be ~6.4KB). If this exceeds the P2P `max_message_size` (default 64KB), the message is dropped.
**Why it happens:** `serde_json` serializes `[u8; 32]` as an array of 32 numbers, not as hex. Each byte becomes up to 3 characters plus comma: `[255,255,...255]` = ~128 chars per service_id.
**How to avoid:** For Phase 14 (data structures only), this is not an operational concern. But the test should verify that encoding 256 services (the expected max) fits within 64KB. Also consider hex-encoding the service IDs in the JSON: `"subscribe": ["abcd..."]` is more compact than `"subscribe": [[171,205,...]]`.
**Warning signs:** No test verifying serialization size for realistic payloads.

## Code Examples

Verified patterns from the existing codebase:

### Existing Test Pattern (from p2p_broadcast_tests)
```rust
// Source: packages/wavs/src/subsystems/aggregator/p2p.rs lines 1394-1689
#[cfg(test)]
mod p2p_broadcast_tests {
    use super::*;
    use commonware_codec::{Encode, ReadRangeExt};
    use commonware_cryptography::Digestible;

    #[test]
    fn test_service_router_subscribe_accept() {
        let service_id_a = ServiceId::hash(b"test-service-a");
        let mut router = ServiceRouter::new();
        router.subscribe(&service_id_a);
        let msg_a = P2pMessage {
            service_id_bytes: service_id_a.inner(),
            payload: vec![],
        };
        assert!(router.should_accept(&msg_a));
    }
}
```

### Creating Ed25519 PublicKeys for Tests
```rust
// Source: commonware-cryptography ed25519 scheme.rs
use commonware_cryptography::ed25519;
use commonware_math::algebra::Random;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::SeedableRng;

fn test_pubkey(seed_byte: u8) -> ed25519::PublicKey {
    let mut rng = ChaCha20Rng::from_seed([seed_byte; 32]);
    let private = ed25519::PrivateKey::random(&mut rng);
    private.public_key()
}
```

### Existing Sentinel Pattern
```rust
// Source: packages/wavs/src/subsystems/aggregator/p2p.rs line 458
/// Reserved service ID used by heartbeat probes to discover connected peers.
/// No real service uses all-zeros service ID, so ServiceRouter filters these out.
const HEARTBEAT_SERVICE_ID: [u8; 32] = [0u8; 32];
```

### P2pMessage Codec Roundtrip Test Pattern
```rust
// Source: packages/wavs/src/subsystems/aggregator/p2p.rs lines 1460-1476
#[test]
fn test_p2p_message_codec_roundtrip() {
    let msg = P2pMessage {
        service_id_bytes: [42u8; 32],
        payload: b"hello broadcast world".to_vec(),
    };
    let encoded = msg.encode();
    let decoded = P2pMessage::read_range(&mut encoded.as_ref(), 0..=65536).unwrap();
    assert_eq!(msg.service_id_bytes, decoded.service_id_bytes);
    assert_eq!(msg.payload, decoded.payload);
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
| SUB-01 | Forward index maps service_id to peer set, updated from announcements | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_peer_subscription_map_forward_index` | Wave 0 |
| SUB-02 | Reverse index maps peer to service set, remove_peer clears both | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_peer_subscription_map_remove_peer` | Wave 0 |
| SUB-03 | remove_peer clears all entries from both maps for a given peer | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_peer_subscription_map_disconnect_cleanup` | Wave 0 |
| ANN-05 | SubscriptionAnnouncement encodes/decodes with sentinel distinguishable from real messages | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_subscription_announcement_roundtrip` | Wave 0 |
| ANN-05 (sentinel) | SUBSCRIPTION_SENTINEL is distinguishable from HEARTBEAT_SERVICE_ID and real service hashes | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_subscription_sentinel_distinguishable` | Wave 0 |
| SUB-01 (fallback) | get_recipients returns Recipients::All when subscriber set is empty | unit | `cargo test -p wavs --lib -- p2p_broadcast_tests::test_get_recipients_empty_fallback` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p wavs --lib -- p2p_broadcast_tests`
- **Per wave merge:** `cargo test -p wavs`
- **Phase gate:** Full `cargo test -p wavs` green before verification

### Wave 0 Gaps
- All tests are new (Wave 0) -- they will be written alongside the data structures in the same file (`p2p.rs`, module `p2p_broadcast_tests`)
- No new test files or framework installs needed -- tests extend the existing `#[cfg(test)] mod p2p_broadcast_tests` module

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `Recipients::All` everywhere (14+ call sites) | `Recipients::Some(service_peers)` for targeted delivery | v1.3 (this milestone) | Phase 14 builds the data structures; Phase 16 does the actual replacement |
| Local-only `ServiceRouter` filtering | `PeerSubscriptionMap` tracking remote peer subscriptions | v1.3 (this milestone) | New capability -- no existing code replaced |
| No control messages on P2P channel | Sentinel-based `SubscriptionAnnouncement` | v1.3 (this milestone) | Extends the existing heartbeat sentinel pattern |

## Open Questions

1. **serde_json serialization of `[u8; 32]` arrays**
   - What we know: serde_json serializes `[u8; 32]` as an array of 32 numbers like `[255,255,...,255]`, which is verbose (~128 chars per service_id)
   - What's unclear: Whether the planner should use `#[serde(with = "hex")]` or a newtype with hex encoding to make announcements more compact
   - Recommendation: Start with default serde_json. At 256 services, the payload is ~33KB -- well within 64KB max_message_size. Optimize to hex encoding only if needed. The data structure design does not depend on this choice.

2. **Whether `SubscriptionAnnouncement` needs `Digestible` implementation**
   - What we know: `P2pMessage` implements `Digestible` for deduplication. Subscription announcements wrapped as P2pMessage will get digested via the outer P2pMessage. The `seen_digests` set will deduplicate repeated heartbeat-carried announcements.
   - What's unclear: Whether this deduplication is desired (heartbeat announcements are idempotent by design, dedup prevents re-processing)
   - Recommendation: Let the existing P2pMessage deduplication handle it. Subscription announcements are idempotent, so dedup just saves a few map operations. No custom Digestible needed.

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

## Sources

### Primary (HIGH confidence)
- `packages/wavs/src/subsystems/aggregator/p2p.rs` (1689 lines) -- full source analysis: existing types (P2pMessage, ServiceRouter, RetryQueue, P2pCommand, P2pHandle), bridge loop patterns, sentinel pattern, test module structure
- `packages/types/src/http.rs` -- P2pStatus struct (lines 134-150)
- `commonware-cryptography-2026.3.0/src/ed25519/scheme.rs` -- PublicKey derives `Clone, Eq, PartialEq, Ord, PartialOrd, Hash` (line 123); implements `Read`/`Write` codec traits (lines 158, 164)
- `commonware-p2p-2026.3.0/src/lib.rs` -- `Recipients` enum: `All`, `Some(Vec<P>)`, `One(P)` (lines 42-46)
- `.planning/research/ARCHITECTURE.md` -- complete component diagrams, data flow sequences, PeerSubscriptionMap design
- `.planning/research/STACK.md` -- confirmed no new dependencies needed; Recipients::Some verified
- `.planning/research/PITFALLS.md` -- 11 pitfalls catalogued; Pitfall 8 (empty recipients) directly relevant to get_recipients() design

### Secondary (MEDIUM confidence)
- `.planning/research/SUMMARY.md` -- project-level research summary with phase ordering rationale

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- zero new dependencies; all types verified from source
- Architecture: HIGH -- patterns directly derived from existing p2p.rs code (1689 lines analyzed)
- Pitfalls: HIGH -- 4 phase-specific pitfalls identified from project-level research + code analysis

**Research date:** 2026-04-03
**Valid until:** 2026-05-03 (stable -- pure data structure design, no external API dependencies)
