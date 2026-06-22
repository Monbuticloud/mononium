//! JSON-RPC and REST server types.
//!
//! ## Module structure
//!
//! - `config` — RPC configuration and error codes
//! - `state` — shared application state
//! - `server` — JSON-RPC WebSocket server

pub mod config;
pub mod server;
pub mod state;
