// MbValue is a newtype around u64; the JIT passes it by value as a 64-bit word.
#![allow(improper_ctypes_definitions)]

//! FFI functions exposed by `mcp-mamba` to Mamba scripts.
//!
//! All functions follow the Mamba native-call ABI:
//! ```text
//! extern "C" fn name(args: *const MbValue, nargs: usize) -> MbValue
//! ```
//!
//! # Exposed API
//!
//! | Symbol                           | Mamba call                                          |
//! |----------------------------------|-----------------------------------------------------|
//! | `mb_mcp_server_new`              | `MCPServer(name) -> server`                         |
//! | `mb_mcp_server_register_tool`    | `server.tool()(fn) — @server.tool() decorator`      |
//! | `mb_mcp_server_tool_count`       | `len(server.tools) -> int`                          |
//! | `mb_mcp_server_run_stdio`        | `server.run() — MCP stdio transport`                |
//! | `mb_mcp_server_streamable_http_app` | `server.streamable_http_app() -> app`            |
//! | `mb_mcp_server_name`             | `server.name -> str`                                |

use mamba_registry::convert::mb_wrap_native;
use mamba_registry::MbValue;

use crate::types::{MbMcpApp, MbMcpServer};

// ── Helpers ───────────────────────────────────────────────────────────────────

#[inline]
unsafe fn arg(args: *const MbValue, nargs: usize, idx: usize) -> MbValue {
    if idx < nargs {
        unsafe { *args.add(idx) }
    } else {
        MbValue::none()
    }
}

fn read_str(v: MbValue) -> Option<String> {
    mamba_registry::test_ops::init();
    unsafe { mamba_registry::rc::read_obj_str(v) }
}

fn wrap_str(s: String) -> MbValue {
    mamba_registry::test_ops::init();
    mamba_registry::rc::wrap_obj_str(s)
}

// ── mb_mcp_server_new ─────────────────────────────────────────────────────────

/// Create a new MCP server with the given name.
///
/// # ABI
/// ```text
/// args[0] = name  (MbValue::Ptr → heap String)
/// ```
/// Returns an opaque PTR to [`MbMcpServer`].
#[no_mangle]
pub unsafe extern "C" fn mb_mcp_server_new(args: *const MbValue, nargs: usize) -> MbValue {
    let name_val = unsafe { arg(args, nargs, 0) };
    let name = read_str(name_val).unwrap_or_else(|| "mcp".to_string());
    mb_wrap_native(MbMcpServer::new(name))
}

// ── mb_mcp_server_register_tool ───────────────────────────────────────────────

/// Register a tool on the MCP server.
///
/// This backs the `@server.tool()` decorator pattern:
/// ```python
/// @mcp.tool()
/// async def my_tool(x: int) -> dict:
///     ...
/// ```
///
/// # ABI
/// ```text
/// args[0] = server    (MbValue::Ptr → MbMcpServer)
/// args[1] = name      (MbValue::Ptr → heap String — tool name)
/// args[2] = doc       (MbValue::Ptr → heap String — docstring)
/// args[3] = func      (MbValue::Func — Mamba function pointer)
/// ```
/// Returns `MbValue::none()`.
#[no_mangle]
pub unsafe extern "C" fn mb_mcp_server_register_tool(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let server_val = unsafe { arg(args, nargs, 0) };
    let name_val = unsafe { arg(args, nargs, 1) };
    let doc_val = unsafe { arg(args, nargs, 2) };
    let func_val = unsafe { arg(args, nargs, 3) };

    let addr = match server_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return MbValue::none(),
    };
    let server = unsafe { &mut *(addr as *mut MbMcpServer) };

    let name = read_str(name_val).unwrap_or_default();
    let doc = read_str(doc_val).unwrap_or_default();
    let func_ptr = func_val.as_func().unwrap_or(0);

    server.register_tool(name, doc, func_ptr);
    MbValue::none()
}

// ── mb_mcp_server_tool_count ──────────────────────────────────────────────────

/// Return the number of tools registered on the server.
///
/// # ABI
/// ```text
/// args[0] = server  (MbValue::Ptr → MbMcpServer)
/// ```
/// Returns `MbValue::Int(count)`.
#[no_mangle]
pub unsafe extern "C" fn mb_mcp_server_tool_count(args: *const MbValue, nargs: usize) -> MbValue {
    let server_val = unsafe { arg(args, nargs, 0) };

    let addr = match server_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return MbValue::from_int(0),
    };
    let server = unsafe { &*(addr as *const MbMcpServer) };
    MbValue::from_int(server.tools.len() as i64)
}

// ── mb_mcp_server_run_stdio ───────────────────────────────────────────────────

/// Start the MCP server in stdio transport mode.
///
/// This is the MCP stdio transport entry point.  In this prototype it prints
/// a minimal JSON-RPC 2.0 initialize response to stdout and returns.
/// Full bidirectional stdio MCP dispatch will be wired in a follow-up.
///
/// # ABI
/// ```text
/// args[0] = server  (MbValue::Ptr → MbMcpServer)
/// ```
/// Returns `MbValue::none()`.
#[no_mangle]
pub unsafe extern "C" fn mb_mcp_server_run_stdio(args: *const MbValue, nargs: usize) -> MbValue {
    let server_val = unsafe { arg(args, nargs, 0) };

    let server_name = server_val
        .as_ptr()
        .map(|addr| {
            if addr == 0 {
                "mcp".to_string()
            } else {
                unsafe { &*(addr as *const MbMcpServer) }.name.clone()
            }
        })
        .unwrap_or_else(|| "mcp".to_string());

    // Emit a minimal MCP initialize response stub.
    println!(
        r#"{{"jsonrpc":"2.0","id":1,"result":{{"protocolVersion":"2024-11-05","serverInfo":{{"name":"{server_name}","version":"0.1.0"}},"capabilities":{{"tools":{{}}}}}}}}"#
    );

    MbValue::none()
}

// ── mb_mcp_server_streamable_http_app ─────────────────────────────────────────

/// Return an ASGI app handle for HTTP mounting.
///
/// Used as `mcp_app = mcp.streamable_http_app()` in Conductor's `server.py`.
/// The returned [`MbMcpApp`] handle carries the server name and can be passed
/// to the HTTP framework's mount function.
///
/// # ABI
/// ```text
/// args[0] = server  (MbValue::Ptr → MbMcpServer)
/// ```
/// Returns an opaque PTR to [`MbMcpApp`].
#[no_mangle]
pub unsafe extern "C" fn mb_mcp_server_streamable_http_app(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let server_val = unsafe { arg(args, nargs, 0) };

    let server_name = server_val
        .as_ptr()
        .map(|addr| {
            if addr == 0 {
                "mcp".to_string()
            } else {
                unsafe { &*(addr as *const MbMcpServer) }.name.clone()
            }
        })
        .unwrap_or_else(|| "mcp".to_string());

    mb_wrap_native(MbMcpApp::new(server_name))
}

// ── mb_mcp_server_name ────────────────────────────────────────────────────────

/// Get the name of the MCP server.
///
/// # ABI
/// ```text
/// args[0] = server  (MbValue::Ptr → MbMcpServer)
/// ```
/// Returns `MbValue::Ptr → heap String`.
#[no_mangle]
pub unsafe extern "C" fn mb_mcp_server_name(args: *const MbValue, nargs: usize) -> MbValue {
    let server_val = unsafe { arg(args, nargs, 0) };

    let addr = match server_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return wrap_str(String::new()),
    };
    let server = unsafe { &*(addr as *const MbMcpServer) };
    wrap_str(server.name.clone())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_str_val(s: &str) -> MbValue {
        mamba_registry::test_ops::init();
        mamba_registry::rc::wrap_obj_str(s.to_string())
    }

    #[test]
    fn test_server_new() {
        let name_val = make_str_val("Conductor");
        let args = [name_val];
        let server_val = unsafe { mb_mcp_server_new(args.as_ptr(), 1) };
        assert!(server_val.is_ptr(), "server should be a ptr");

        let addr = server_val.as_ptr().unwrap();
        let server = unsafe { &*(addr as *const MbMcpServer) };
        assert_eq!(server.name, "Conductor");
        assert!(server.tools.is_empty());
    }

    #[test]
    fn test_register_tool() {
        let name_val = make_str_val("TestServer");
        let args = [name_val];
        let server_val = unsafe { mb_mcp_server_new(args.as_ptr(), 1) };

        let tool_name_val = make_str_val("list_projects");
        let doc_val = make_str_val("List all projects.");
        let func_val = MbValue::from_func(0xDEAD);
        let reg_args = [server_val, tool_name_val, doc_val, func_val];
        let result = unsafe { mb_mcp_server_register_tool(reg_args.as_ptr(), 4) };
        assert!(result.is_none());

        let addr = server_val.as_ptr().unwrap();
        let server = unsafe { &*(addr as *const MbMcpServer) };
        assert_eq!(server.tools.len(), 1);
        assert_eq!(server.tools[0].0, "list_projects");
        assert_eq!(server.tools[0].1, "List all projects.");
        assert_eq!(server.tools[0].2, 0xDEAD);
    }

    #[test]
    fn test_tool_count() {
        let name_val = make_str_val("CountServer");
        let args = [name_val];
        let server_val = unsafe { mb_mcp_server_new(args.as_ptr(), 1) };

        // Initially 0
        let count_val = unsafe { mb_mcp_server_tool_count([server_val].as_ptr(), 1) };
        assert_eq!(count_val.as_int(), Some(0));

        // Register two tools
        for i in 0..2u8 {
            let tool_name_val = make_str_val(&format!("tool_{i}"));
            let doc_val = make_str_val("doc");
            let func_val = MbValue::from_func(i as usize);
            let reg_args = [server_val, tool_name_val, doc_val, func_val];
            unsafe { mb_mcp_server_register_tool(reg_args.as_ptr(), 4) };
        }

        let count_val = unsafe { mb_mcp_server_tool_count([server_val].as_ptr(), 1) };
        assert_eq!(count_val.as_int(), Some(2));
    }

    #[test]
    fn test_streamable_http_app() {
        let name_val = make_str_val("Conductor");
        let args = [name_val];
        let server_val = unsafe { mb_mcp_server_new(args.as_ptr(), 1) };

        let app_val = unsafe { mb_mcp_server_streamable_http_app([server_val].as_ptr(), 1) };
        assert!(app_val.is_ptr(), "streamable_http_app should return a ptr");

        let addr = app_val.as_ptr().unwrap();
        let app = unsafe { &*(addr as *const MbMcpApp) };
        assert_eq!(app.server_name, "Conductor");
    }

    #[test]
    fn test_server_name() {
        let name_val = make_str_val("MyServer");
        let args = [name_val];
        let server_val = unsafe { mb_mcp_server_new(args.as_ptr(), 1) };

        let name_result = unsafe { mb_mcp_server_name([server_val].as_ptr(), 1) };
        assert!(name_result.is_ptr());
        let s = unsafe { name_result.as_obj_str() }.unwrap();
        assert_eq!(s, "MyServer");
    }
}
