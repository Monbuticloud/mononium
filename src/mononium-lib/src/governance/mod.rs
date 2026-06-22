//! Governance module: stake-weighted proposals, voting, parameter changes.
//!
//! ## Module structure
//!
//! - `types` — core data types (Proposal, Vote, GovernanceAction, …)
//! - `constants` — protocol constants and param bounds
//! - `engine` — GovernanceEngine (submit, vote, cancel, tally, execute)

pub mod constants;
pub mod engine;
pub mod types;
