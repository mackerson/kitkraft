//! MCP stdio server over the engine command catalog.
//!
//! Thin by design: `server` speaks the protocol (JSON-RPC 2.0, one message per line,
//! MCP 2026-07-28 with the handshake-era revisions for older clients), `tools` declares
//! the tool shapes from the catalog, `backend` maps tool calls onto `Engine::execute`.
//! No format or editing logic lives here. See `docs/mcp.md`.
//!
//! LAYER: L5

pub mod backend;
pub mod server;
pub mod tools;

pub use server::{Server, PROTOCOL_VERSION};

/// Serves MCP on this process's stdin/stdout until stdin closes.
pub fn serve_stdio(server: &mut Server) -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    server.serve(stdin.lock(), stdout.lock())
}
