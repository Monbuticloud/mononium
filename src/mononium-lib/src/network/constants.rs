//! Network protocol constants.

/// Default P2P port for libp2p.
pub const DEFAULT_P2P_PORT: u16 = 30333;

/// Agent version string sent during Identify.
pub const AGENT_VERSION: &str = "mononium/0.1.0";

/// Protocol version string.
pub const PROTOCOL_VERSION: &str = "mononium/1.0";

/// Maximum number of peer connections.
pub const MAX_PEERS: usize = 50;

/// Kademlia replication factor.
pub const KAD_REPLICATION_FACTOR: usize = 20;

/// Kademlia query timeout in seconds.
pub const KAD_QUERY_TIMEOUT_SECS: u64 = 10;

/// Maximum message size for gossipsub (1 MB default).
pub const GOSSIP_MAX_TRANSMIT_SIZE: usize = 1_048_576;
