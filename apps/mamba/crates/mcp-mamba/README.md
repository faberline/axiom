# Cclab Mcp Mamba

## Brief

Cclab MCP Mamba is the Mamba native binding for MCP server primitives.

It registers the `cclab.mcp` module through the shared Mamba registry and
exposes native-call entrypoints for server creation, tool metadata registration,
tool counts, server-name lookup, a minimal stdio initialize response stub, and
streamable HTTP app handles. Full bidirectional MCP stdio dispatch is outside
the current binding surface.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Mamba MCP Server Binding | - | exposes MCP server handles, tool metadata registration, stdio initialize stub, and streamable HTTP app handles |

### Mamba MCP Server Binding

Cclab MCP Mamba exposes MCP server primitives to Mamba scripts through the
`cclab.mcp` native module, including server handles, tool registration
metadata, tool counts, server-name access, a minimal stdio initialize response
stub, and streamable HTTP app handles for mounting.

- Root WI: none; this capability predates the tracker.
- Surfaces: Mamba module: `cclab.mcp`; Native ABI: `mb_mcp_server_new`,
  `mb_mcp_server_register_tool`, `mb_mcp_server_tool_count`,
  `mb_mcp_server_run_stdio`, `mb_mcp_server_streamable_http_app`,
  `mb_mcp_server_name`; Rust module registrar: `McpMambaModule`
- Gate — behavior: `cargo test -p mcp-mamba`
- Gate: `cargo test -p mcp-mamba`
- Source: `apps/mamba/crates/mcp-mamba/src/lib.rs`,
  `apps/mamba/crates/mcp-mamba/src/methods.rs`,
  `apps/mamba/crates/mcp-mamba/src/types.rs`,
  `apps/mamba/crates/mcp-mamba/tests/methods_test.rs`
- Evidence: `cargo test -p mcp-mamba`; apps/mamba/crates/mcp-mamba/src/lib.rs;
  apps/mamba/crates/mcp-mamba/src/methods.rs; apps/mamba/crates/mcp-mamba/src/types.rs;
  apps/mamba/crates/mcp-mamba/tests/methods_test.rs
