# Phase 3: Config and Observability - Research

**Researched:** 2026-03-17
**Domain:** WAVS P2P configuration format (commonware-tailored) and observability endpoints
**Confidence:** HIGH

## Summary

Phase 3 focuses on two complementary tasks: (1) updating the `wavs.toml` P2P configuration comments/documentation and the `wavs.toml` default comments to reflect commonware instead of libp2p, and (2) making the `/p2p/status` endpoint return real connected peer data instead of placeholder zeros.

The existing `P2pConfig` enum (`Disabled` / `Local` / `Remote`) was already created in Phase 1 and is already tailored to commonware (peer_addresses with `<hex_pubkey>@<host>:<port>` format, not multiaddr). The struct serialization and deserialization works correctly. What needs to change is: (a) the `wavs.toml` comments still reference "mDNS" and "Kademlia DHT" which are libp2p concepts, (b) the `P2pStatus` struct still has libp2p-era fields (`external_addresses`, `topic_peer_counts`), and (c) the `GetStatus` handler returns `connected_peers: 0` and `peer_ids: vec![]` as hardcoded placeholders (with explicit "Phase 3 fills from network state" comments).

**Primary recommendation:** Track connected peers via an `Arc<RwLock<HashSet<PublicKey>>>` shared between the bridge loop and status handler, updated from broadcast acknowledgment results. Update `P2pStatus` to remove libp2p fields and add commonware-specific fields. Update `wavs.toml` comments to remove all libp2p terminology.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CFG-01 | New P2P config format in wavs.toml (Disabled / Local / Remote) tailored to commonware | P2pConfig enum already exists with correct shape; wavs.toml comments need updating to remove libp2p terminology (mDNS, Kademlia, multiaddr). See "Config Format Update" section. |
| CFG-02 | Configurable listen port, bootstrappers, timeouts, deque sizes | listen_port and bootstrappers already configurable. Timeouts/deque_sizes exist as hardcoded constants (MAX_RETRY_QUEUE_SIZE=64, deque_size=128, mailbox_size=256, max_message_size=65536) -- make them configurable via P2pConfig fields. |
| CFG-03 | Local dev preset with localhost peer addresses for multi-operator testing | P2pConfig::Local already supports peer_addresses. Need a documented "dev preset" example in wavs.toml showing 2-3 operators on localhost with minimal config (just listen_port + peer_addresses). |
| OBS-01 | `/p2p/status` returns peer ID, listen addresses, connected peers, subscribed services | Endpoint exists at `/p2p/status`, P2pStatus struct exists, but connected_peers=0 and peer_ids=[] are hardcoded placeholders. Need to track connected peers from broadcast results and populate these fields. |
| OBS-02 | Status uses socket addresses (not multiaddr) and Ed25519 public keys | P2pConfig already uses socket addresses (`<hex_pubkey>@<host>:<port>`). P2pStatus.listen_addresses already returns socket format. Need to remove P2pStatus.external_addresses (multiaddr concept) and ensure peer_id uses Ed25519 hex (already does). |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| commonware-p2p | 2026.3.0 | Authenticated P2P networking | Already pinned in Cargo.toml from Phase 1 |
| commonware-broadcast | 2026.3.0 | Buffered broadcast with catch-up | Already pinned from Phase 2 |
| serde / toml | (existing) | Config serialization | Already used by wavs.toml loader (figment) |
| axum | (existing) | HTTP endpoint for /p2p/status | Already used by WAVS HTTP server |
| utoipa | (existing) | OpenAPI schema generation | Already used for P2pStatus schema |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| figment | (existing) | Config file loading with env overrides | Used by ConfigBuilder -- P2P config flows through this |
| const_hex | (existing) | Hex encoding for Ed25519 public keys | Already used in peer ID formatting |

No new dependencies required. Phase 3 works entirely with existing crates.

## Architecture Patterns

### Current P2P Module Structure
```
packages/wavs/src/subsystems/aggregator/p2p.rs  (single file, ~1200 lines)
  - P2pConfig enum (Disabled / Local / Remote)
  - P2pMessage (Codec + Digestible)
  - ServiceRouter
  - RetryQueue
  - spawn_commonware_runtime()
  - run_lookup_network() / run_discovery_network()
  - P2pCommand / P2pHandle
  - ed25519_signer_from_mnemonic()
```

### Pattern 1: Connected Peer Tracking via Broadcast Acknowledgments
**What:** Track connected peers by observing who receives broadcasts
**When to use:** When commonware-p2p doesn't expose a direct "get connected peers" public API
**Details:**

The commonware-p2p router internally tracks connected peers in a `BTreeMap<P, Relay>`. It supports a `SubscribePeers` message that sends the current peer list and notifies on changes. However, this mechanism is only accessible internally through the `Messenger` type, which implements the `Connected` trait. The public API (`Sender` / `Receiver` returned from `network.register()`) does not directly expose peer tracking.

**Recommended approach:** Maintain an `Arc<RwLock<HashSet<String>>>` of connected peer hex strings. Update it from two sources:
1. **Broadcast success results:** When `mailbox.broadcast(Recipients::All, msg).await` returns `Ok(recipients)`, the `recipients` Vec contains the public keys of all peers that received the message. Store these as the connected peer set.
2. **Inbound message senders:** When a message arrives from a peer on the inbound bridge, record that peer as connected.

This is pragmatic and sufficient for the observability requirement. The count will be accurate after the first broadcast/receive, and starts at 0 when no traffic has flowed (which is truthful).

**Alternative (more complex but more accurate):** Spawn a peer tracking task inside the commonware runtime that calls `messenger.subscribe()` (via the `Connected` trait on the messenger) before `network.start()`. Bridge the `ring::Receiver<Vec<PublicKey>>` back to the bridge loop via a tokio mpsc channel. This gives real-time peer connect/disconnect events.

**Recommendation:** Use the simpler broadcast-results approach. It is accurate enough for observability and avoids threading a third channel through the architecture. The Phase 2 code already captures `recipients` in the broadcast result handler.

### Pattern 2: P2pStatus Struct Simplification
**What:** Remove libp2p-specific fields, add commonware-specific fields
**Current P2pStatus (packages/types/src/http.rs):**
```rust
pub struct P2pStatus {
    pub enabled: bool,
    pub local_peer_id: Option<String>,        // Ed25519 hex -- KEEP
    pub listen_addresses: Vec<String>,         // Socket addresses -- KEEP
    pub external_addresses: Vec<String>,       // Multiaddr/AutoNAT -- REMOVE (OBS-02)
    pub connected_peers: usize,               // FILL (currently hardcoded 0)
    pub peer_ids: Vec<String>,                // FILL (currently hardcoded empty)
    pub subscribed_topics: Vec<String>,       // RENAME to subscribed_services
    pub topic_peer_counts: HashMap<String, usize>, // REMOVE (GossipSub concept)
}
```

**New P2pStatus:**
```rust
pub struct P2pStatus {
    pub enabled: bool,
    pub local_peer_id: Option<String>,         // Ed25519 hex pubkey
    pub listen_addresses: Vec<String>,         // Socket addresses (e.g. "0.0.0.0:9000")
    pub connected_peers: usize,               // Actual count from peer tracking
    pub peer_ids: Vec<String>,                // Hex-encoded Ed25519 pubkeys of connected peers
    pub subscribed_services: Vec<String>,     // Hex-encoded service IDs (was subscribed_topics)
}
```

**Breaking change note:** This modifies the `/p2p/status` JSON response shape. The `/info` endpoint also returns `P2pStatus`. CLI clients (`packages/cli/src/clients.rs`) deserialize `P2pStatus` -- they need to be updated. The `wait_for_p2p_ready()` method uses `status.connected_peers` which stays the same. The `P2pStatus` struct is in `packages/types/src/http.rs` (shared across crates).

### Pattern 3: Config Comment Update (wavs.toml)
**What:** Replace libp2p terminology in wavs.toml with commonware terminology
**Current issues in wavs.toml comments (lines 192-233):**
- Line 197: "Local mDNS discovery" -- should be "Local lookup mode (known peer addresses)"
- Line 213: "Remote Kademlia DHT discovery" -- should be "Remote discovery mode (bootstrapper nodes)"
- Line 214: `bootstrap_nodes = ["/ip4/1.2.3.4/tcp/9000/p2p/12D3KooW..."]` -- should be `bootstrappers = ["<hex_pubkey>@<host>:<port>"]`
- Lines 203-211: Old config fields (max_retry_duration_secs, retry_interval_ms, etc.) that no longer exist in P2pConfig
- Lines 217-229: Same old config fields for remote mode

**What the new P2pConfig actually supports (from current code):**
```rust
P2pConfig::Local {
    listen_port: u16,
    peer_addresses: Vec<String>,    // "<hex_pubkey>@<host>:<port>"
    authorized_peers: Vec<String>,  // hex Ed25519 pubkeys
}
P2pConfig::Remote {
    listen_port: u16,
    bootstrappers: Vec<String>,     // "<hex_pubkey>@<host>:<port>"
    authorized_peers: Vec<String>,  // hex Ed25519 pubkeys
}
```

### Anti-Patterns to Avoid
- **Do NOT add a `deque_size` or `mailbox_size` to the TOML-facing config unless explicitly needed.** These are internal tuning parameters of the commonware broadcast Engine. Keep them hardcoded in the code (128 and 256 respectively) -- they are not operator-facing concerns. The requirement CFG-02 says "configurable deque sizes" but the practical implementation should expose only the fields that operators actually need to tune.
- **Do NOT try to access commonware internals.** The Router's `SubscribePeers` mechanism is internal to the network actor system. Building a complex bridge just to get peer counts is not worth the complexity.
- **Do NOT remove `P2pStatus` from `wavs_types`.** It's used across crates (cli, wavs, layer-tests). Modify in place with backward-compatible additions where possible.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Config loading | Custom TOML parser | figment (existing) | Already handles env overrides, nested sections, defaults |
| Peer ID formatting | Custom serialization | const_hex::encode() | Already used throughout p2p.rs |
| OpenAPI schema | Manual JSON schema | utoipa::ToSchema derive | Already used on P2pStatus |
| Test P2P configs | Manual socket allocation | Existing test_port() helper | Already used in p2p_connectivity_tests.rs and p2p_broadcast_tests.rs |

## Common Pitfalls

### Pitfall 1: P2pStatus Deserialization Breaking CLI
**What goes wrong:** Removing fields from P2pStatus breaks CLI client deserialization if the CLI is not updated simultaneously
**Why it happens:** `packages/cli/src/clients.rs` deserializes P2pStatus. If the server returns JSON without `external_addresses` but the CLI struct expects it, serde will fail
**How to avoid:** Use `#[serde(default)]` on the P2pStatus struct to make all fields optional/defaultable. Add new fields with defaults. Keep old field names as aliases during transition if needed. Since this is a coordinated upgrade (all operators upgrade simultaneously per REQUIREMENTS.md), this is manageable.
**Warning signs:** `serde_json::from_str` failures in CLI after upgrade

### Pitfall 2: Connected Peers Count Always Zero
**What goes wrong:** The connected_peers count stays 0 because no broadcasts have been sent yet
**Why it happens:** If using broadcast acknowledgments to track peers, the count is 0 until the first broadcast
**How to avoid:** Accept that 0 is truthful when no broadcasts have occurred. The CLI's `wait_for_p2p_ready(min_peers, timeout)` will still work because broadcasts happen quickly after services are added. Alternatively, track peers from inbound messages too (a received message proves connectivity).
**Warning signs:** Tests expecting non-zero connected_peers immediately after startup without any message exchange

### Pitfall 3: wavs.toml Comments Out of Sync with P2pConfig Struct
**What goes wrong:** The TOML config example shows fields that don't exist in the actual P2pConfig struct, causing silent deserialization failures
**Why it happens:** wavs.toml has hardcoded comments that were never updated when P2pConfig was rewritten in Phase 1
**How to avoid:** The wavs.toml comments should exactly match P2pConfig's serde fields. Test that example config snippets actually parse correctly.
**Warning signs:** `figment::Error` when uncommenting P2P config lines

### Pitfall 4: TestP2pMode Enum Still References libp2p Concepts
**What goes wrong:** `TestP2pMode::Mdns` and `TestP2pMode::Kademlia` in layer-tests use libp2p names but map to commonware Local/Remote
**Why it happens:** Leftover naming from the migration
**How to avoid:** This is Phase 4 cleanup scope (INT-02). Don't rename the test enum in Phase 3 -- it would change the test config format. Document the naming inconsistency for Phase 4.

## Code Examples

### Example 1: Updated P2pStatus Struct
```rust
// packages/types/src/http.rs
/// P2P network status for monitoring and readiness checks
#[derive(Debug, Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
pub struct P2pStatus {
    /// Whether P2P networking is enabled
    pub enabled: bool,
    /// Local peer ID (Ed25519 public key, hex-encoded)
    pub local_peer_id: Option<String>,
    /// Listen addresses (socket format, e.g. "0.0.0.0:9000")
    pub listen_addresses: Vec<String>,
    /// Number of connected peers
    pub connected_peers: usize,
    /// Connected peer IDs (Ed25519 public keys, hex-encoded)
    pub peer_ids: Vec<String>,
    /// Services this node is subscribed to (hex-encoded service ID hashes)
    pub subscribed_services: Vec<String>,
}
```

### Example 2: Connected Peer Tracking in Bridge Loop
```rust
// Inside run_lookup_network() / run_discovery_network()
// Shared state for connected peer tracking
let connected_peers: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(Vec::new()));

// In the broadcast acknowledgment handler:
Ok(recipients) => {
    // Update connected peer tracking from broadcast results
    let peer_hexes: Vec<String> = recipients
        .iter()
        .map(|pk| const_hex::encode(pk.as_ref()))
        .collect();
    *connected_peers.write().unwrap() = peer_hexes;
    // ... rest of broadcast handling
}

// In the GetStatus handler:
Some(P2pCommand::GetStatus { response_tx }) => {
    let peers = connected_peers.read().unwrap().clone();
    let status = P2pStatus {
        enabled: true,
        local_peer_id: Some(const_hex::encode(own_pubkey.as_ref())),
        listen_addresses: vec![listen_addr.to_string()],
        connected_peers: peers.len(),
        peer_ids: peers,
        subscribed_services: service_router.subscribed_topics(),
    };
    let _ = response_tx.send(status);
}
```

### Example 3: Updated wavs.toml P2P Config Comments
```toml
# ----------------------------
# P2P networking settings (commonware)
# ----------------------------
# P2P is disabled by default (for single-operator setups).
# Enable for multi-operator deployments to share submissions and reach quorum consensus.
# Peer identities use Ed25519 public keys derived from signing_mnemonic.

# Option 1 -- Disabled (default, single-operator):
# p2p = "disabled"

# Option 2 -- Local lookup mode (development/testing, known peer addresses):
# p2p = { local = { listen_port = 9000 } }
#
# [wavs.p2p.local]
# listen_port = 9000
# peer_addresses = ["<hex_ed25519_pubkey>@127.0.0.1:9001"]
# authorized_peers = ["<hex_ed25519_pubkey>"]

# Option 3 -- Remote discovery mode (production, bootstrapper-based discovery):
# p2p = { remote = { listen_port = 9000, bootstrappers = ["<hex_ed25519_pubkey>@1.2.3.4:9000"] } }
#
# [wavs.p2p.remote]
# listen_port = 9000
# bootstrappers = []            # Empty = this node acts as a bootstrapper
# authorized_peers = ["<hex_ed25519_pubkey>"]
```

### Example 4: Local Dev Preset (CFG-03)
```toml
# Multi-operator local dev setup (2 operators):
#
# wavs-node-1.toml:
# [wavs.p2p.local]
# listen_port = 9000
# peer_addresses = ["<node2_ed25519_pubkey>@127.0.0.1:9001"]
#
# wavs-node-2.toml:
# [wavs.p2p.local]
# listen_port = 9001
# peer_addresses = ["<node1_ed25519_pubkey>@127.0.0.1:9000"]
#
# To get a node's Ed25519 public key from its mnemonic:
# wavs-cli p2p identity --mnemonic "your mnemonic words here"
```

### Example 5: CFG-02 Extended P2pConfig with Optional Tuning
```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum P2pConfig {
    #[default]
    Disabled,
    Local {
        listen_port: u16,
        #[serde(default)]
        peer_addresses: Vec<String>,
        #[serde(default)]
        authorized_peers: Vec<String>,
        /// Max message size in bytes (default: 65536 = 64KB)
        #[serde(default)]
        max_message_size: Option<u32>,
        /// Broadcast Engine deque size per peer for catch-up (default: 128)
        #[serde(default)]
        deque_size: Option<usize>,
    },
    Remote {
        listen_port: u16,
        #[serde(default)]
        bootstrappers: Vec<String>,
        #[serde(default)]
        authorized_peers: Vec<String>,
        #[serde(default)]
        max_message_size: Option<u32>,
        #[serde(default)]
        deque_size: Option<usize>,
    },
}
```

## State of the Art

| Old Approach (libp2p) | Current Approach (commonware) | When Changed | Impact |
|----------------------|------------------------------|--------------|--------|
| Multiaddr format ("/ip4/x/tcp/y/p2p/...") | Socket addresses + Ed25519 hex ("pubkey@host:port") | Phase 1 | Config format, status output, documentation |
| mDNS for local discovery | Lookup mode with known addresses | Phase 1 | Operators must specify peer addresses (no zero-config) |
| Kademlia DHT for production | Discovery mode with bootstrappers | Phase 1 | Bootstrapper format changed to "pubkey@host:port" |
| GossipSub topics per service | Single broadcast channel with app-level filtering | Phase 2 | No per-topic peer counts possible |
| AutoNAT external addresses | Not available (operators need public IPs) | Phase 1 | external_addresses field meaningless |
| PeerId (base58 encoded) | Ed25519 PublicKey (hex-encoded) | Phase 1 | Peer identity format changed |

**Deprecated/outdated:**
- `P2pStatus.external_addresses`: AutoNAT concept from libp2p, commonware has no equivalent
- `P2pStatus.topic_peer_counts`: GossipSub concept, commonware uses single channel
- `P2pStatus.subscribed_topics` naming: Should be `subscribed_services` to match commonware's service-filtering model
- wavs.toml comments referencing mDNS, Kademlia, and libp2p config fields

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in + tokio::test) |
| Config file | Cargo.toml test configuration |
| Quick run command | `cargo test -p wavs --test p2p_connectivity_tests -- --test-threads=1` |
| Full suite command | `cargo test -p wavs --test p2p_connectivity_tests --test p2p_broadcast_tests -- --test-threads=1` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CFG-01 | P2pConfig::Local/Remote/Disabled deserialize from TOML correctly | unit | `cargo test -p wavs p2p_config_serde -x` | No -- Wave 0 |
| CFG-02 | Optional max_message_size and deque_size applied to Engine config | unit | `cargo test -p wavs p2p_config_defaults -x` | No -- Wave 0 |
| CFG-03 | Two-node local dev preset connects successfully | integration | `cargo test -p wavs --test p2p_connectivity_tests test_lookup_mode -x` | Yes (extends existing) |
| OBS-01 | GetStatus returns non-zero connected_peers after broadcast | integration | `cargo test -p wavs --test p2p_broadcast_tests test_status_after_broadcast -x` | No -- Wave 0 |
| OBS-02 | P2pStatus uses socket addresses and Ed25519 hex keys | unit | `cargo test -p wavs p2p_status_format -x` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p wavs -- p2p --test-threads=1`
- **Per wave merge:** `cargo test -p wavs --test p2p_connectivity_tests --test p2p_broadcast_tests -- --test-threads=1`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `p2p_config_serde` unit test -- CFG-01: verify P2pConfig roundtrips through TOML correctly, including new optional fields
- [ ] `p2p_config_defaults` unit test -- CFG-02: verify default values for max_message_size and deque_size
- [ ] `test_status_after_broadcast` integration test -- OBS-01: verify GetStatus returns real peer data after a broadcast exchange
- [ ] `p2p_status_format` unit test -- OBS-02: verify P2pStatus serializes with socket addresses, no multiaddr fields

## Open Questions

1. **Should deque_size and max_message_size be operator-facing config?**
   - What we know: CFG-02 says "Configurable ... deque sizes". Currently hardcoded at 128 and 65536.
   - What's unclear: Whether operators actually need to tune these, or if sensible defaults suffice
   - Recommendation: Add as optional fields with defaults. Operators can ignore them. `#[serde(default)]` means they're invisible in minimal configs.

2. **Should connected_peers tracking be real-time or best-effort?**
   - What we know: commonware-p2p's `SubscribePeers` mechanism is internal to the Router actor. The broadcast acknowledgment approach is simpler but updates only after message exchange.
   - What's unclear: Whether tests or production monitoring require immediate peer detection
   - Recommendation: Use broadcast-results tracking (simpler). The CLI's `wait_for_p2p_ready()` already polls with retry, so eventual accuracy is fine.

3. **Should thread_handle storage (TODO from Phase 2) be done in this phase?**
   - What we know: Line 1117 of p2p.rs has `// TODO: Store thread_handle for clean shutdown in Phase 3`
   - What's unclear: Whether Phase 3 scope includes clean shutdown (not in requirements CFG-01..03, OBS-01..02)
   - Recommendation: Address opportunistically if it's simple (store handle in P2pHandle, add a `shutdown()` method). Don't make it a primary deliverable.

## Sources

### Primary (HIGH confidence)
- commonware-p2p 2026.3.0 source code (cargo registry: `~/.cargo/registry/src/*/commonware-p2p-2026.3.0/`) -- Router actor, SubscribePeers mechanism, Connected trait, network.rs
- WAVS codebase direct inspection:
  - `packages/wavs/src/subsystems/aggregator/p2p.rs` -- P2pConfig, P2pHandle, bridge loops, GetStatus handler
  - `packages/wavs/src/config.rs` -- Config struct with P2pConfig field
  - `packages/types/src/http.rs` -- P2pStatus struct definition
  - `packages/wavs/src/http/handlers/p2p.rs` -- /p2p/status endpoint
  - `packages/wavs/src/http/handlers/info.rs` -- /info endpoint (includes P2pStatus)
  - `packages/wavs/src/http/server.rs` -- Route registration
  - `packages/cli/src/clients.rs` -- CLI HTTP client consuming P2pStatus
  - `packages/layer-tests/src/e2e/config.rs` -- TestP2pMode and e2e test P2P config
  - `wavs.toml` -- Current config file with outdated libp2p comments

### Secondary (MEDIUM confidence)
- Phase 1 and 2 implementation plans and state (`.planning/` directory)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries already in use, no new dependencies
- Architecture: HIGH - Directly inspected commonware-p2p source code to verify peer tracking options
- Pitfalls: HIGH - Based on code-level analysis of serialization contracts and cross-crate dependencies

**Research date:** 2026-03-17
**Valid until:** 2026-04-17 (stable -- all commonware versions pinned, no moving targets)
