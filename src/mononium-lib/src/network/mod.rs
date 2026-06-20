//! P2P networking layer built on libp2p.
//!
//! Implements gossipsub (4 topics), kademlia + mDNS discovery, peer
//! scoring with ban mechanics, snappy compression, and the sync protocol.

pub mod constants;
pub mod messages;
pub mod peer_score;
pub mod topics;
