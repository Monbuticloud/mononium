//! Peer scoring and ban management.
//!
//! Each peer has a score in [-100, 100] that is adjusted based on
//! behavior. Peers whose score falls below -20 are banned for
//! [`BAN_DURATION`] blocks (~1 hour).

use std::collections::HashMap;
use std::time::Instant;

use libp2p::PeerId;

/// Ban duration in blocks (~1 hour at 5s blocks).
pub const BAN_DURATION: u64 = 720;

/// Score threshold below which a peer is banned.
pub const BAN_THRESHOLD: i32 = -20;

/// Maximum possible peer score.
pub const MAX_SCORE: i32 = 100;

/// Minimum possible peer score.
pub const MIN_SCORE: i32 = -100;

// ---------------------------------------------------------------------------
// PeerScore
// ---------------------------------------------------------------------------

/// Score state for a single peer.
#[derive(Debug, Clone)]
pub struct PeerScore {
    /// Current score, clamped to [-100, 100].
    score: i32,
    /// Block height at which the ban was applied, if banned.
    banned_at_height: Option<u64>,
    /// Timestamp of the last positive interaction.
    last_positive: Instant,
}

impl PeerScore {
    /// Create a new neutral peer score.
    #[must_use]
    pub fn new() -> Self {
        Self {
            score: 0,
            banned_at_height: None,
            last_positive: Instant::now(),
        }
    }

    /// Get the current score.
    #[must_use]
    pub fn score(&self) -> i32 {
        self.score
    }

    /// Check whether the peer is currently banned.
    #[must_use]
    pub fn is_banned(&self, current_height: u64) -> bool {
        match self.banned_at_height {
            Some(h) => current_height < h + BAN_DURATION,
            None => false,
        }
    }

    /// Returns `true` if the score is below the ban threshold.
    #[must_use]
    pub fn should_ban(&self) -> bool {
        self.score < BAN_THRESHOLD
    }

    /// Apply a ban at the given current height.
    pub fn apply_ban(&mut self, current_height: u64) {
        self.banned_at_height = Some(current_height);
    }

    /// Adjust the score by `delta`, clamped to [-100, 100].
    pub fn adjust(&mut self, delta: i32) {
        self.score = (self.score + delta).clamp(MIN_SCORE, MAX_SCORE);
        if delta > 0 {
            self.last_positive = Instant::now();
        }
    }
}

impl Default for PeerScore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ScoreEvent — named adjustment constants
// ---------------------------------------------------------------------------

/// Predefined score adjustments for specific peer behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreEvent {
    ValidBlockPropagated,
    ValidVotePropagated,
    SuccessfulSyncBatch,
    SyncBatchHashMismatch,
    SyncBatchVerifyFail,
    EmptySyncResponse,
    SyncTimeout,
    InvalidBlockGossiped,
    InvalidVoteGossiped,
    ConnectDisconnectLoop,
    DuplicateBlockGossip,
}

impl ScoreEvent {
    /// The score delta for this event.
    #[must_use]
    pub fn delta(self) -> i32 {
        match self {
            Self::ValidBlockPropagated => 1,
            Self::ValidVotePropagated => 1,
            Self::SuccessfulSyncBatch => 2,
            Self::SyncBatchHashMismatch => -10,
            Self::SyncBatchVerifyFail => -20,
            Self::EmptySyncResponse => -2,
            Self::SyncTimeout => -4,
            Self::InvalidBlockGossiped => -10,
            Self::InvalidVoteGossiped => -10,
            Self::ConnectDisconnectLoop => -10,
            Self::DuplicateBlockGossip => -2,
        }
    }
}

// ---------------------------------------------------------------------------
// PeerScoreRepo
// ---------------------------------------------------------------------------

/// Thread-safe repository of peer scores.
#[derive(Debug, Clone)]
pub struct PeerScoreRepo {
    scores: HashMap<PeerId, PeerScore>,
}

impl PeerScoreRepo {
    /// Create an empty repo.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scores: HashMap::new(),
        }
    }

    /// Get the score for a peer, or create a default one if absent.
    pub fn get_or_create(&mut self, peer: &PeerId) -> &mut PeerScore {
        self.scores.entry(*peer).or_default()
    }

    /// Apply a score event to a peer.
    pub fn apply_event(&mut self, peer: &PeerId, event: ScoreEvent) {
        let score = self.get_or_create(peer);
        score.adjust(event.delta());
    }

    /// Get the raw score value for a peer.
    #[must_use]
    pub fn score(&self, peer: &PeerId) -> Option<i32> {
        self.scores.get(peer).map(PeerScore::score)
    }

    /// Check if a peer is banned.
    #[must_use]
    pub fn is_banned(&self, peer: &PeerId, current_height: u64) -> bool {
        self.scores
            .get(peer)
            .map(|s| s.is_banned(current_height))
            .unwrap_or(false)
    }

    /// Apply bans to all peers whose score is below threshold.
    pub fn auto_ban(&mut self, current_height: u64) {
        for score in self.scores.values_mut() {
            if score.should_ban() && score.banned_at_height.is_none() {
                score.apply_ban(current_height);
            }
        }
    }

    /// Iterate over all scores.
    pub fn iter(&self) -> impl Iterator<Item = (&PeerId, &PeerScore)> {
        self.scores.iter()
    }

    /// Number of tracked peers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.scores.len()
    }

    /// Whether the repo is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.scores.is_empty()
    }
}

impl Default for PeerScoreRepo {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_peer_id(n: u8) -> PeerId {
        PeerId::from_bytes(&[0, 1, n]).unwrap()
    }

    #[test]
    fn test_new_score_is_zero() {
        let ps = PeerScore::new();
        assert_eq!(ps.score(), 0);
    }

    #[test]
    fn test_adjust_clamps_correctly() {
        let mut ps = PeerScore::new();

        ps.adjust(50);
        assert_eq!(ps.score(), 50);

        ps.adjust(100);
        assert_eq!(ps.score(), MAX_SCORE); // 100

        ps.adjust(-250);
        assert_eq!(ps.score(), MIN_SCORE); // -100
    }

    #[test]
    fn test_ban_threshold() {
        let mut ps = PeerScore::new();
        assert!(!ps.should_ban());

        ps.adjust(-19);
        assert!(!ps.should_ban()); // score = -19, not below -20

        ps.adjust(-2);
        assert!(ps.should_ban()); // score = -21, below -20
    }

    #[test]
    fn test_ban_duration() {
        let mut ps = PeerScore::new();
        ps.apply_ban(50);

        assert!(ps.is_banned(50)); // 50 < 50 + 720
        assert!(ps.is_banned(769)); // 769 < 770
        assert!(!ps.is_banned(770)); // 770 >= 50 + 720
    }

    #[test]
    fn test_score_event_deltas() {
        assert_eq!(ScoreEvent::ValidBlockPropagated.delta(), 1);
        assert_eq!(ScoreEvent::ValidVotePropagated.delta(), 1);
        assert_eq!(ScoreEvent::SuccessfulSyncBatch.delta(), 2);
        assert_eq!(ScoreEvent::SyncBatchHashMismatch.delta(), -10);
        assert_eq!(ScoreEvent::SyncBatchVerifyFail.delta(), -20);
        assert_eq!(ScoreEvent::EmptySyncResponse.delta(), -2);
        assert_eq!(ScoreEvent::SyncTimeout.delta(), -4);
        assert_eq!(ScoreEvent::InvalidBlockGossiped.delta(), -10);
        assert_eq!(ScoreEvent::InvalidVoteGossiped.delta(), -10);
        assert_eq!(ScoreEvent::ConnectDisconnectLoop.delta(), -10);
        assert_eq!(ScoreEvent::DuplicateBlockGossip.delta(), -2);
    }

    #[test]
    fn test_repo_apply_event() {
        let mut repo = PeerScoreRepo::new();
        let peer = dummy_peer_id(1);

        repo.apply_event(&peer, ScoreEvent::ValidBlockPropagated);
        assert_eq!(repo.score(&peer), Some(1));

        repo.apply_event(&peer, ScoreEvent::InvalidBlockGossiped);
        assert_eq!(repo.score(&peer), Some(-9)); // 1 - 10 = -9
    }

    #[test]
    fn test_repo_len_and_is_empty() {
        let mut repo = PeerScoreRepo::new();
        assert!(repo.is_empty());
        assert_eq!(repo.len(), 0);

        let peer = dummy_peer_id(1);
        repo.apply_event(&peer, ScoreEvent::ValidBlockPropagated);
        assert!(!repo.is_empty());
        assert_eq!(repo.len(), 1);
    }

    #[test]
    fn test_repo_iter() {
        let mut repo = PeerScoreRepo::new();
        let p1 = dummy_peer_id(1);
        let p2 = dummy_peer_id(2);
        repo.apply_event(&p1, ScoreEvent::ValidBlockPropagated);
        repo.apply_event(&p2, ScoreEvent::InvalidBlockGossiped);

        let scores: Vec<(PeerId, i32)> = repo.iter().map(|(p, s)| (*p, s.score())).collect();
        assert_eq!(scores.len(), 2);
        assert!(scores.contains(&(p1, 1)));
        assert!(scores.contains(&(p2, -10)));
    }

    #[test]
    fn test_repo_default_is_empty() {
        let repo: PeerScoreRepo = Default::default();
        assert!(repo.is_empty());
    }

    #[test]
    fn test_peer_score_default_is_zero() {
        let ps: PeerScore = Default::default();
        assert_eq!(ps.score(), 0);
    }

    #[test]
    fn test_peer_score_is_banned_false_when_never_banned() {
        let ps = PeerScore::new();
        // Fresh PeerScore has banned_at_height = None → is_banned returns false
        assert!(!ps.is_banned(0));
        assert!(!ps.is_banned(100));
        assert!(!ps.is_banned(u64::MAX));
    }

    #[test]
    fn test_peer_score_is_banned_true_when_banned_and_still_active() {
        let mut ps = PeerScore::new();
        ps.apply_ban(100);
        // banned_at_height = 100, BAN_DURATION = 720
        assert!(ps.is_banned(100));   // 100 < 820
        assert!(ps.is_banned(819));   // 819 < 820
    }

    #[test]
    fn test_peer_score_is_banned_false_after_ban_expires() {
        let mut ps = PeerScore::new();
        ps.apply_ban(100);
        // banned_at_height = 100, BAN_DURATION = 720
        assert!(!ps.is_banned(820));  // 820 >= 820
        assert!(!ps.is_banned(900));  // well past expiry
    }

    #[test]
    fn test_peer_score_is_banned_transition() {
        let mut ps = PeerScore::new();
        // Not banned initially
        assert!(!ps.is_banned(50));
        // Apply ban at height 50
        ps.apply_ban(50);
        // Banned while within duration
        assert!(ps.is_banned(50));
        assert!(ps.is_banned(769)); // 50 + 720 - 1
        // Ban expires
        assert!(!ps.is_banned(770)); // 50 + 720
    }

    #[test]
    fn test_repo_score_none_for_unknown() {
        let repo = PeerScoreRepo::new();
        assert_eq!(repo.score(&dummy_peer_id(99)), None);
    }

    #[test]
    fn test_repo_is_banned_unknown_peer() {
        let repo = PeerScoreRepo::new();
        assert!(!repo.is_banned(&dummy_peer_id(99), 0));
    }

    #[test]
    fn test_repo_auto_ban() {
        let mut repo = PeerScoreRepo::new();
        let peer = dummy_peer_id(1);

        repo.apply_event(&peer, ScoreEvent::SyncBatchVerifyFail);
        repo.apply_event(&peer, ScoreEvent::SyncBatchVerifyFail);
        // score = -40

        repo.auto_ban(100);
        assert!(repo.is_banned(&peer, 100));
        assert!(!repo.is_banned(&peer, 821)); // 100 + 720 + 1
    }
}
