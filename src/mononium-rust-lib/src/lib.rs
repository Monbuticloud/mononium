//! Mononium core blockchain library.
//!
//! All blockchain logic: types, state machine, consensus engine, crypto,
//! storage, P2P networking, RPC, governance, and configuration.

pub mod core;
pub mod crypto;
pub mod consensus;
pub mod config;
pub mod mempool;
pub mod storage;
pub mod network;
pub mod governance;
pub mod rpc;
