# WAVS Migrates to commonware for P2P Networking

**Date:** 2026-03

## Summary

WAVS has replaced its libp2p-based P2P networking layer with commonware primitives.
This migration delivers simpler architecture, better performance, and tighter security
for multi-operator deployments.

## Why commonware?

The libp2p integration served WAVS well during early development, but its feature
surface far exceeded what WAVS actually needed. The configuration required 13 feature
flags (tcp, dns, noise, yamux, identify, ping, gossipsub, request-response, kad, mdns,
autonat, macros, secp256k1) to support a relatively narrow use case: broadcast
signed submissions between a known set of operators. GossipSub mesh tuning,
per-service topic management, and secp256k1 peer identity all added operational
complexity without matching benefits.

commonware takes a focused approach. Its broadcast Engine is purpose-built for
disseminating messages to a known set of peers with reliable catch-up on reconnection.
Peer authorization uses an Oracle — a deterministic set of allowed public keys —
rather than open-mesh peer discovery. This maps directly to WAVS's trust model:
only registered operators should communicate.

The identity model is another strong fit. WAVS operators already hold a signing
mnemonic (`WAVS_SIGNING_MNEMONIC`). commonware's Ed25519 identity is derived
deterministically from that mnemonic via ChaCha20Rng, so no separate key management
is required. The same mnemonic that signs on-chain submissions also identifies the
operator in the P2P network.

## What Changed

- **Identity:** secp256k1 keypair (libp2p PeerID like `12D3Koo...`) replaced by Ed25519 keypair derived from signing mnemonic. Peer IDs are now 64-character hex strings.
- **Address format:** multiaddr (`/ip4/1.2.3.4/tcp/9000/p2p/12D3Koo...`) replaced by socket format (`<hex_ed25519_pubkey>@<host>:<port>`).
- **Discovery:** mDNS (local LAN) and Kademlia DHT (production) replaced by Lookup mode (explicit peer list, local/dev) and Discovery mode (bootstrapper-based, production).
- **Message routing:** Per-service GossipSub topics replaced by a single commonware broadcast channel with application-level service filtering (ServiceRouter).
- **Catch-up:** Custom request/response protocol replaced by Broadcast Engine with per-peer message caching. On reconnection, the Engine replays cached messages to the reconnecting peer automatically.
- **Security:** Connection-level TLS encryption replaced by Oracle-based peer authorization. Only peers whose Ed25519 public keys appear in `authorized_peers` can connect. Built-in per-peer rate limiting is provided by commonware's default configuration.

## Impact on Operators

This is a breaking change. The commonware and libp2p backends are fundamentally
incompatible — they use different wire protocols, different identity schemes, and
different address formats. Operators running multi-node deployments must coordinate
a simultaneous upgrade across all nodes.

Peer IDs will change. All operators need to regenerate their peer IDs from their
signing mnemonic, exchange the new IDs out-of-band, and update their `wavs.toml`
configuration before restarting. Single-operator deployments (no P2P enabled) are
unaffected beyond the config format change if they migrate to the new `wavs.toml`
format.

See the [Operator Migration Guide](../OPERATOR_MIGRATION.md) for step-by-step
instructions covering identity regeneration, config format changes, and coordinated
upgrade sequencing.

## Technical Details

The migration was implemented in four phases. Phase 1 replaced the identity system,
establishing Ed25519 keypair derivation from the signing mnemonic via ChaCha20Rng.
Phase 2 replaced the network layer, implementing lookup and discovery modes on
top of commonware-p2p with Oracle-based authorization. Phase 3 added the broadcast
architecture — a two-channel design where Channel 0 feeds the Broadcast Engine for
message caching and catch-up, and Channel 1 provides real-time forwarding to the
Aggregator.

Zero libp2p code remains in the codebase. The networking stack uses
commonware-p2p for transport and peer management, commonware-broadcast for message
dissemination and catch-up, and commonware-cryptography for Ed25519 key operations.

## What's Next

Future improvements in the P2P layer include on-chain operator registry integration
(automatically deriving `authorized_peers` from registered operators rather than
manual configuration), simulated networking tests for deterministic multi-operator
testing, and improved NAT traversal guidance for production deployments. The current
architecture provides a stable foundation for these enhancements.
