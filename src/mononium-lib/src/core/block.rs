//! Block types: Block, BlockHeader, BlockBody, CommitVote.
//!
//! All types support both SCALE (wire) and JSON (RPC) encoding.
//!
//! **Votes** are not included in the block body. They are gossipped and
//! stored independently in `block_votes`.
