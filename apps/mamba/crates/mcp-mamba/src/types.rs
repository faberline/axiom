//! Opaque types for the `mcp-mamba` FFI layer.

/// An MCP server that accumulates tool registrations.
///
/// Tools are registered with `@server.tool()` decorator syntax backed by
/// `mb_mcp_server_register_tool`.  Call `mb_mcp_server_run_stdio` to start
/// the stdio transport, or `mb_mcp_server_streamable_http_app` to get an
/// ASGI-compatible app handle for HTTP mounting.
#[derive(Debug, Clone)]
pub struct MbMcpServer {
    /// The server name as used in the MCP `initialize` response.
    pub name: String,
    /// Registered tools: (tool_name, docstring, func_ptr).
    pub tools: Vec<(String, String, usize)>,
}

impl MbMcpServer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tools: Vec::new(),
        }
    }

    pub fn register_tool(
        &mut self,
        name: impl Into<String>,
        doc: impl Into<String>,
        func_ptr: usize,
    ) {
        self.tools.push((name.into(), doc.into(), func_ptr));
    }
}

/// An ASGI-compatible MCP app handle returned by `streamable_http_app()`.
///
/// Used by Conductor as `mcp_app = mcp.streamable_http_app()` and mounted
/// into the main ASGI application.
#[derive(Debug, Clone)]
pub struct MbMcpApp {
    /// The originating server name.
    pub server_name: String,
}

impl MbMcpApp {
    pub fn new(server_name: impl Into<String>) -> Self {
        Self {
            server_name: server_name.into(),
        }
    }
}
