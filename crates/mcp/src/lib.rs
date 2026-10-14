//! MCP stdio server over the engine command catalog.
//!
//! Thin by design: `server` speaks the protocol, `tools` declares the tool shapes,
//! `backend` maps tool calls onto engine commands. No format or editing logic lives here.
//! See `docs/mcp.md`.
//!
//! LAYER: L5

pub mod backend;
pub mod server;
pub mod tools;
