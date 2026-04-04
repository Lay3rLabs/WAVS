# Requirements: WAVS v1.3

**Defined:** 2026-04-03
**Core Value:** Multi-operator signature aggregation over P2P must work reliably -- operators broadcast signed submissions, reach quorum, and submit on-chain

## v1.3 Requirements

Requirements for per-service P2P targeting. Each maps to roadmap phases.

### Subscription Tracking

- [x] **SUB-01**: Node maintains a per-service peer subscription map (`service_id → Set<PeerPubkey>`) updated from announcement messages
- [x] **SUB-02**: Node maintains a reverse index (`PeerPubkey → Set<service_id>`) for efficient cleanup on peer disconnect
- [x] **SUB-03**: When a peer disconnects, all its subscription entries are removed from both maps

### Announcement Protocol

- [x] **ANN-01**: When an operator adds a service, a subscription announcement is broadcast to all connected peers
- [x] **ANN-02**: When an operator removes a service, an unsubscribe announcement is broadcast to all connected peers
- [x] **ANN-03**: Subscription state is piggybacked on periodic heartbeats for self-healing eventual consistency
- [x] **ANN-04**: When a new peer connects, the node sends its full subscription set as a hello message
- [x] **ANN-05**: Subscription announcements use a sentinel service_id to multiplex on the existing direct channel (no new channels required)

### Targeted Send

- [x] **TGT-01**: Submissions on the direct channel (channel 1) use `Recipients::Some(service_peers)` instead of `Recipients::All`
- [x] **TGT-02**: When the subscriber set for a service is empty or a peer hasn't announced yet, the node falls back to `Recipients::All`
- [x] **TGT-03**: The broadcast Engine channel (channel 0) continues using `Recipients::All` for catch-up reliability
- [x] **TGT-04**: Retry queue messages re-resolve recipients at drain time (not cached from original send)

### Observability

- [x] **OBS-01**: `/p2p/status` endpoint includes per-service peer counts (how many peers subscribe to each service)

### Compatibility

- [x] **COMPAT-01**: Existing secp256k1 e2e tests pass unchanged
- [x] **COMPAT-02**: Existing BLS e2e tests pass unchanged
- [x] **COMPAT-03**: Nodes without v1.3 targeting are treated as subscribed-to-all (backward compatible during rolling updates)

## Future Requirements

Deferred to v1.4+. Tracked but not in current roadmap.

### Catch-Up Scoping

- **CATCH-01**: Per-service catch-up filtering -- Engine only replays cached messages for services the reconnecting peer subscribes to
- **CATCH-02**: Per-service Engine instances with independent deque caches

### Scale Optimization

- **SCALE-01**: Gossip relay mode -- send to K random peers who relay, for services with 50+ operators
- **SCALE-02**: Aggregator hub mode -- operators send to designated aggregator nodes only, for 500+ operator services
- **SCALE-03**: Differential subscription updates -- only send changes vs full state on heartbeat

## Out of Scope

| Feature | Reason |
|---------|--------|
| Per-service catch-up scoping | Engine has no service awareness; requires commonware fork or separate caching layer -- defer to v1.4 |
| Gossip relay / aggregator hub | Not needed at current operator scale (3-50); foundation (Recipients::Some) enables later |
| Multiple Network instances per service | Connection/identity overhead not justified at current scale |
| Dynamic channel registration | commonware-p2p requires channels before network.start(); architectural constraint |
| Signed subscription announcements | Authenticated peer set via Oracle makes spoofing impractical |
| Tauri app changes | Backend-only milestone; P2P status already displays in existing UI |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| SUB-01 | Phase 14 | Complete |
| SUB-02 | Phase 14 | Complete |
| SUB-03 | Phase 18 | Complete |
| ANN-01 | Phase 15 | Complete |
| ANN-02 | Phase 15 | Complete |
| ANN-03 | Phase 15 | Complete |
| ANN-04 | Phase 15 | Complete |
| ANN-05 | Phase 14 | Complete |
| TGT-01 | Phase 16 | Complete |
| TGT-02 | Phase 16 | Complete |
| TGT-03 | Phase 16 | Complete |
| TGT-04 | Phase 16 | Complete |
| OBS-01 | Phase 17 | Complete |
| COMPAT-01 | Phase 16 | Complete |
| COMPAT-02 | Phase 16 | Complete |
| COMPAT-03 | Phase 18 | Complete |

**Coverage:**
- v1.3 requirements: 16 total
- Mapped to phases: 16
- Unmapped: 0

---
*Requirements defined: 2026-04-03*
*Last updated: 2026-04-04 after gap closure phase 18 added*
