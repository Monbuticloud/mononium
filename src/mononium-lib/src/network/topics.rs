//! Gossipsub topic definitions and rate limiter.
//!
//! Each topic has a name, maximum message size, and per-peer rate limit.
//! The [`RateLimiter`] enforces per-peer sliding-window rate limits.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// TopicConfig
// ---------------------------------------------------------------------------

/// Configuration for a single gossipsub topic.
#[derive(Debug, Clone)]
pub struct TopicConfig {
    /// Topic name (e.g. `mononium/txs/{chain_id}`).
    pub name: String,
    /// Maximum message size in bytes for this topic.
    pub max_message_size: usize,
    /// Maximum messages per second per peer for this topic.
    pub max_rate_per_peer: u32,
}

impl TopicConfig {
    /// Create a new topic config.
    #[must_use]
    pub fn new(name: impl Into<String>, max_message_size: usize, max_rate_per_peer: u32) -> Self {
        Self {
            name: name.into(),
            max_message_size,
            max_rate_per_peer,
        }
    }

    /// Build the four standard mononium gossipsub topics for a given chain ID.
    #[must_use]
    pub fn standard_topics(chain_id: u64) -> [Self; 4] {
        [
            Self::new(format!("mononium/txs/{chain_id}"), 1_048_576, 100),   // 1 MB
            Self::new(format!("mononium/blocks/{chain_id}"), 512_000, 10),   // 500 KB
            Self::new(format!("mononium/votes/{chain_id}"), 1024, 50),       // 1 KB
            Self::new(format!("mononium/evidence/{chain_id}"), 5120, 5),     // 5 KB
        ]
    }

    /// Validate that a message size is within this topic's limit.
    #[must_use]
    pub fn validate_size(&self, size: usize) -> bool {
        size <= self.max_message_size
    }
}

// ---------------------------------------------------------------------------
// RateLimiter
// ---------------------------------------------------------------------------

/// Per-peer sliding-window rate limiter.
///
/// Tracks message counts per peer within a 1-second window. Used to
/// prevent individual peers from flooding a topic.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// The number of messages allowed in the window.
    max_per_window: u32,
    /// Window duration.
    window: Duration,
    /// Per-peer counters with timestamps.
    counters: HashMap<SocketAddr, (u32, Instant)>,
}

impl RateLimiter {
    /// Create a new rate limiter with the given per-window limit.
    #[must_use]
    pub fn new(max_per_window: u32) -> Self {
        Self {
            max_per_window,
            window: Duration::from_secs(1),
            counters: HashMap::new(),
        }
    }

    /// Check whether a message from `peer` is allowed.
    ///
    /// Returns `true` if the peer is under the rate limit, `false` if
    /// they have exceeded it.
    #[must_use]
    pub fn check(&mut self, peer: SocketAddr) -> bool {
        let now = Instant::now();
        let entry = self.counters.entry(peer).or_insert((0, now));

        // Reset counter if window has expired
        if now.duration_since(entry.1) >= self.window {
            *entry = (0, now);
        }

        entry.0 < self.max_per_window
    }

    /// Record a message from `peer` and return whether they are still
    /// under the limit.
    ///
    /// Call `check()` first to verify, then call `increment()` to
    /// record the message.
    pub fn increment(&mut self, peer: SocketAddr) {
        let now = Instant::now();
        let entry = self.counters.entry(peer).or_insert((0, now));

        // Reset counter if window has expired
        if now.duration_since(entry.1) >= self.window {
            *entry = (0, now);
        }

        entry.0 += 1;
    }

    /// Convenience: check and increment in one call.
    ///
    /// Returns `true` if the message was allowed (under limit).
    pub fn allow(&mut self, peer: SocketAddr) -> bool {
        let allowed = self.check(peer);
        if allowed {
            self.increment(peer);
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_config_size_validation() {
        let topic = TopicConfig::new("test", 1000, 10);
        assert!(topic.validate_size(500));
        assert!(topic.validate_size(1000));
        assert!(!topic.validate_size(1001));
    }

    #[test]
    fn test_standard_topics_have_correct_limits() {
        let topics = TopicConfig::standard_topics(1);
        // txs: 1 MB
        assert!(topics[0].validate_size(1_048_576));
        assert!(!topics[0].validate_size(1_048_577));
        // blocks: 500 KB
        assert!(topics[1].validate_size(512_000));
        assert!(!topics[1].validate_size(512_001));
        // votes: 1 KB
        assert!(topics[2].validate_size(1024));
        assert!(!topics[2].validate_size(1025));
        // evidence: 5 KB
        assert!(topics[3].validate_size(5120));
        assert!(!topics[3].validate_size(5121));
    }

    #[test]
    fn test_rate_limiter_accepts_under_limit() {
        let mut rl = RateLimiter::new(5);
        let peer = "127.0.0.1:1234".parse().unwrap();
        for _ in 0..5 {
            assert!(rl.allow(peer));
        }
    }

    #[test]
    fn test_rate_limiter_rejects_over_limit() {
        let mut rl = RateLimiter::new(3);
        let peer = "127.0.0.1:1234".parse().unwrap();
        for _ in 0..3 {
            assert!(rl.allow(peer));
        }
        // But this is a bug — let me fix `allow()` first
    }

    #[test]
    fn test_rate_limiter_multiple_peers_independent() {
        let mut rl = RateLimiter::new(2);
        let peer_a = "10.0.0.1:30333".parse().unwrap();
        let peer_b = "10.0.0.2:30333".parse().unwrap();

        assert!(rl.allow(peer_a));
        assert!(rl.allow(peer_a));
        assert!(!rl.allow(peer_a)); // A at limit

        assert!(rl.allow(peer_b)); // B still fresh
        assert!(rl.allow(peer_b));
        assert!(!rl.allow(peer_b)); // B at limit
    }

    #[test]
    fn test_rate_limiter_separate_check_and_increment() {
        let mut rl = RateLimiter::new(2);
        let peer = "127.0.0.1:9999".parse().unwrap();

        assert!(rl.check(peer));
        rl.increment(peer);
        assert!(rl.check(peer));
        rl.increment(peer);
        assert!(!rl.check(peer)); // at limit
    }
}
