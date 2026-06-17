# ADR-008: P2P Networking

**Status:** Accepted

**Context:** Validators need to discover each other, gossip transactions, and propagate blocks.

**Decision:** libp2p (rust-libp2p).

- Gossipsub for block and tx propagation
- Kademlia for peer discovery
- Identify protocol for peer metadata
- mDNS for localnet development

**Consequences:**

- Industry standard (Polkadot, Filecoin, Eth2)
- Avoids building discovery, NAT traversal, multiplexing from scratch
- Handles connection management, relay, and protocol negotiation
- Adds a C++ dependency (via libp2p's crypto deps) but well-maintained
