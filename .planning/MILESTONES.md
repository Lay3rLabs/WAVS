# Milestones

## v1.0 Commonware P2P Migration (Shipped: 2026-03-18)

**Phases completed:** 4 phases, 11 plans

**Key accomplishments:**
1. Replaced libp2p secp256k1 identity with Ed25519 derived from BIP-39 mnemonic via ChaCha20Rng — deterministic peer IDs across node restarts (IDEN-01, IDEN-02)
2. Implemented commonware-p2p lookup and discovery modes with Oracle peer authorization, rate limiting, and BlockPeer support — all security requirements satisfied (NET-01 to NET-04, SEC-01 to SEC-03)
3. Implemented P2pMessage with Codec+Digestible traits and wired commonware-broadcast buffered Engine — broadcast, service filtering, dedup, retry, and catch-up all working (BCAST-01 to BCAST-05, CATCH-01, CATCH-02)
4. Rewrote P2P config to Disabled/Local/Remote format with configurable max_message_size and deque_size; status endpoint returns real connected peer tracking (CFG-01 to CFG-03, OBS-01, OBS-02)
5. Removed libp2p 0.56 from workspace entirely; renamed test harness enums (Local/Remote); all e2e tests pass (INT-01 to INT-03)
6. Complete documentation: P2P.md rewritten for commonware, announcement blog post, and 231-line operator migration guide covering all 4 breaking changes (DOC-01 to DOC-03)

**Git range:** `3e101d79` (feat: Ed25519 identity) → `08748210` (Tests pass)
**Archive:** [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md)

---
