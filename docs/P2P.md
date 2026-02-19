┌─────────────────────────────────────────────────────────────────────┐
│                        WAVS Operator Node                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────┐  │
│  │   Trigger    │───▶│   Engine     │───▶│   Submission Manager │  │
│  │   Manager    │    │  (WASM exec) │    │                      │  │
│  └──────────────┘    └──────────────┘    └──────────┬───────────┘  │
│                                                      │              │
│                                          ┌───────────▼───────────┐  │
│                                          │     Aggregator        │  │
│                                          │                       │  │
│                                          │  ┌─────────────────┐  │  │
│                                          │  │  Quorum Queue   │  │  │
│                                          │  │  (per eventId)  │  │  │
│                                          │  └────────┬────────┘  │  │
│                                          │           │           │  │
│                                          │  ┌────────▼────────┐  │  │
│  ┌──────────────────────────────────┐    │  │   P2P Network   │  │  │
│  │      Other WAVS Nodes            │◀───┼──│  mDNS/Kademlia  │  │  │
│  │  (receive/send signatures)       │───▶│  │   + GossipSub   │  │  │
│  └──────────────────────────────────┘    │  └────────┬────────┘  │  │
│                                          │           │           │  │
│                                          │  ┌────────▼────────┐  │  │
│                                          │  │    Submit       │  │  │
│                                          │  │  (on quorum)    │  │  │
│                                          │  └────────┬────────┘  │  │
│                                          └───────────┼───────────┘  │
│                                                      │              │
└──────────────────────────────────────────────────────┼──────────────┘
                                                       │
                                                       ▼
                                              ┌────────────────┐
                                              │   Blockchain   │
                                              └────────────────┘

---

## Overview

P2P networking is an optional feature of the Aggregator subsystem. When enabled, operators broadcast their signed submissions to peers, and the Aggregator collects signatures from those peers to reach the quorum threshold before posting on-chain.

Each registered service gets its own GossipSub topic. Operators subscribe to a service's topic on registration and unsubscribe when the service is removed. This keeps message routing scoped to relevant operators.

---

## Configuration

P2P is disabled by default. To enable it, add a `[wavs.p2p]` block to `wavs.toml`.

### Disabled (default)

Omit the `[wavs.p2p]` block entirely. The node operates in single-operator mode: no P2P networking, no gossip, no bootstrap.

### Local — mDNS (development / testing)

```toml
[wavs.p2p]
local = { listen_port = 9000 }
```

Uses multicast DNS for automatic peer discovery on the local network. Suitable for multi-operator dev/test setups on the same LAN.

### Remote — Kademlia DHT (production)

```toml
[wavs.p2p]
remote = { listen_port = 9000, bootstrap_nodes = [
    "/ip4/1.2.3.4/tcp/9000/p2p/12D3Koo...",
] }
```

Uses Kademlia DHT for peer discovery and GossipSub for message dissemination. Suitable for multi-operator production deployments across the internet.

If `bootstrap_nodes` is an empty list, this node acts as the bootstrap server for other peers to connect to.

---

## Status Endpoint

`GET /p2p/status` returns the current P2P state of the node, including:

- Whether P2P is enabled
- The local peer ID
- Listen addresses and discovered external addresses (useful for NAT traversal)
- Connected peer IDs and count
- Subscribed GossipSub topics and peer counts per topic
