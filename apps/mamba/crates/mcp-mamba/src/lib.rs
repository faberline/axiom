//! Mamba binding for the Model Context Protocol (MCP).
//!
//! Exposes MCP server primitives to Mamba scripts via the
//! `mamba-registry` infrastructure.
//!
//! # Module name
//!
//! Import in Mamba as `cclab.mcp`:
//! ```python
//! from cclab.mcp import MCPServer
//! mcp = MCPServer("Conductor")
//!
//! @mcp.tool()
//! async def list_projects() -> dict:
//!     ...
//!
//! mcp_app = mcp.streamable_http_app()
//! ```

pub mod methods;
pub mod types;

use linkme::distributed_slice;
use mamba_registry::{rt_sym, MambaModule, ModuleRegistrar, MAMBA_MODULES};

// ── McpMambaModule ────────────────────────────────────────────────────────────

/// The `mcp-mamba` native module descriptor.
pub struct McpMambaModule;

impl MambaModule for McpMambaModule {
    fn name(&self) -> &'static str {
        "cclab.mcp"
    }

    fn doc(&self) -> &'static str {
        "Mamba bindings for MCP — Model Context Protocol server"
    }

    fn register(&self, r: &mut ModuleRegistrar) {
        use crate::methods::{
            mb_mcp_server_name, mb_mcp_server_new, mb_mcp_server_register_tool,
            mb_mcp_server_run_stdio, mb_mcp_server_streamable_http_app, mb_mcp_server_tool_count,
        };

        r.add_symbols([
            rt_sym!("Server", mb_mcp_server_new, "Server(name: str) -> server"),
            rt_sym!(
                "server_register_tool",
                mb_mcp_server_register_tool,
                "server_register_tool(server, name: str, doc: str, func) -> None"
            ),
            rt_sym!(
                "server_tool_count",
                mb_mcp_server_tool_count,
                "server_tool_count(server) -> int"
            ),
            rt_sym!(
                "server_run_stdio",
                mb_mcp_server_run_stdio,
                "server_run_stdio(server) -> None"
            ),
            rt_sym!(
                "server_streamable_http_app",
                mb_mcp_server_streamable_http_app,
                "server_streamable_http_app(server) -> app"
            ),
            rt_sym!(
                "server_name",
                mb_mcp_server_name,
                "server_name(server) -> str"
            ),
        ]);
    }
}

// ── Auto-registration ─────────────────────────────────────────────────────────

#[distributed_slice(MAMBA_MODULES)]
static MCP_MAMBA_MODULE: &dyn MambaModule = &McpMambaModule;
