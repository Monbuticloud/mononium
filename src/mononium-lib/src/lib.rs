//! Mononium core blockchain library.
//!
//! All blockchain logic: types, state machine, consensus engine, crypto,
//! storage, P2P networking, RPC, governance, and configuration.

pub mod config;
pub mod consensus;
pub mod constants;
pub mod core;
pub mod crypto;
pub mod error;
pub mod governance;
pub mod mempool;
pub mod network;
pub mod rpc;
pub mod storage;
