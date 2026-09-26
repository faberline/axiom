// Integration tests for mcp-mamba: covers all 6 mb_mcp_* functions.
// Requirements: R1, R2, R4, R5, R6
#![allow(improper_ctypes_definitions)]

use mamba_registry::MbValue;
use mcp_mamba::methods::{
    mb_mcp_server_name, mb_mcp_server_new, mb_mcp_server_register_tool, mb_mcp_server_run_stdio,
    mb_mcp_server_streamable_http_app, mb_mcp_server_tool_count,
};
use mcp_mamba::types::{MbMcpApp, MbMcpServer};

// ── Shared helpers ─────────────────────────────────────────────────────────────

fn make_str_val(s: &str) -> MbValue {
    mamba_registry::test_ops::init();
    mamba_registry::rc::wrap_obj_str(s.to_string())
}

unsafe fn read_str_val(v: MbValue) -> String {
    mamba_registry::test_ops::init();
    unsafe { mamba_registry::rc::read_obj_str(v) }.expect("expected a Str MbObject")
}

// ── mb_mcp_server_new ─────────────────────────────────────────────────────────

#[test]
fn server_new_happy() {
    let args = [make_str_val("Conductor")];
    let server_val = unsafe { mb_mcp_server_new(args.as_ptr(), 1) };
    assert!(server_val.is_ptr(), "server should be a ptr");
    let addr = server_val.as_ptr().unwrap();
    let server = unsafe { &*(addr as *const MbMcpServer) };
    assert_eq!(server.name, "Conductor");
    assert_eq!(server.tools.len(), 0, "new server should have no tools");
}

#[test]
fn server_new_default() {
    // No args → name defaults to "mcp"
    let args: [MbValue; 0] = [];
    let server_val = unsafe { mb_mcp_server_new(args.as_ptr(), 0) };
    assert!(server_val.is_ptr());
    let addr = server_val.as_ptr().unwrap();
    let server = unsafe { &*(addr as *const MbMcpServer) };
    assert_eq!(server.name, "mcp", "default server name should be 'mcp'");
}

// ── mb_mcp_server_register_tool ───────────────────────────────────────────────

#[test]
fn register_tool_happy() {
    let server_val = unsafe { mb_mcp_server_new([make_str_val("TestServer")].as_ptr(), 1) };
    let fn_ptr = MbValue::from_func(0xDEAD);
    let args = [
        server_val,
        make_str_val("list_projects"),
        make_str_val("List all projects."),
        fn_ptr,
    ];
    let result = unsafe { mb_mcp_server_register_tool(args.as_ptr(), 4) };
    assert!(result.is_none());

    let addr = server_val.as_ptr().unwrap();
    let server = unsafe { &*(addr as *const MbMcpServer) };
    assert_eq!(server.tools.len(), 1);
}

#[test]
fn register_tool_fields() {
    let server_val = unsafe { mb_mcp_server_new([make_str_val("MyServer")].as_ptr(), 1) };
    let fn_ptr_addr: usize = 0xCAFE_BABE;
    let fn_ptr = MbValue::from_func(fn_ptr_addr);
    let args = [
        server_val,
        make_str_val("my_tool"),
        make_str_val("A useful tool."),
        fn_ptr,
    ];
    unsafe { mb_mcp_server_register_tool(args.as_ptr(), 4) };

    let addr = server_val.as_ptr().unwrap();
    let server = unsafe { &*(addr as *const MbMcpServer) };
    assert_eq!(server.tools[0].0, "my_tool", "tool name should match");
    assert_eq!(server.tools[0].1, "A useful tool.", "tool doc should match");
    assert_eq!(server.tools[0].2, fn_ptr_addr, "tool func_ptr should match");
}

#[test]
fn register_tool_null_server() {
    let fn_ptr = MbValue::from_func(0);
    let args = [
        MbValue::none(),
        make_str_val("tool"),
        make_str_val("doc"),
        fn_ptr,
    ];
    let result = unsafe { mb_mcp_server_register_tool(args.as_ptr(), 4) };
    assert!(
        result.is_none(),
        "register_tool with null server should return none()"
    );
}

// ── mb_mcp_server_tool_count ──────────────────────────────────────────────────

#[test]
fn tool_count_zero() {
    let server_val = unsafe { mb_mcp_server_new([make_str_val("Empty")].as_ptr(), 1) };
    let count = unsafe { mb_mcp_server_tool_count([server_val].as_ptr(), 1) };
    assert_eq!(
        count.as_int(),
        Some(0),
        "empty server should have tool count 0"
    );
}

#[test]
fn tool_count_multiple() {
    let server_val = unsafe { mb_mcp_server_new([make_str_val("Multi")].as_ptr(), 1) };
    for i in 0..3u8 {
        let tool_name = make_str_val(&format!("tool_{i}"));
        let doc = make_str_val("doc");
        let fn_ptr = MbValue::from_func(i as usize);
        let args = [server_val, tool_name, doc, fn_ptr];
        unsafe { mb_mcp_server_register_tool(args.as_ptr(), 4) };
    }
    let count = unsafe { mb_mcp_server_tool_count([server_val].as_ptr(), 1) };
    assert_eq!(count.as_int(), Some(3), "server should report 3 tools");
}

#[test]
fn tool_count_null() {
    let count = unsafe { mb_mcp_server_tool_count([MbValue::none()].as_ptr(), 1) };
    assert_eq!(
        count.as_int(),
        Some(0),
        "null server should return tool count 0"
    );
}

// ── mb_mcp_server_run_stdio ───────────────────────────────────────────────────

#[test]
fn run_stdio_returns_none() {
    let server_val = unsafe { mb_mcp_server_new([make_str_val("StdioServer")].as_ptr(), 1) };
    let result = unsafe { mb_mcp_server_run_stdio([server_val].as_ptr(), 1) };
    assert!(result.is_none(), "run_stdio should return none()");
}

#[test]
fn run_stdio_null() {
    let result = unsafe { mb_mcp_server_run_stdio([MbValue::none()].as_ptr(), 1) };
    assert!(
        result.is_none(),
        "run_stdio with null server should return none()"
    );
}

// ── mb_mcp_server_streamable_http_app ─────────────────────────────────────────

#[test]
fn streamable_http_app_happy() {
    let server_val = unsafe { mb_mcp_server_new([make_str_val("Conductor")].as_ptr(), 1) };
    let app_val = unsafe { mb_mcp_server_streamable_http_app([server_val].as_ptr(), 1) };
    assert!(app_val.is_ptr(), "streamable_http_app should return a ptr");
    let addr = app_val.as_ptr().unwrap();
    let app = unsafe { &*(addr as *const MbMcpApp) };
    assert_eq!(app.server_name, "Conductor");
}

#[test]
fn streamable_http_app_null() {
    // Null server → server_name defaults to "mcp"
    let app_val = unsafe { mb_mcp_server_streamable_http_app([MbValue::none()].as_ptr(), 1) };
    assert!(
        app_val.is_ptr(),
        "streamable_http_app with null should still return a ptr"
    );
    let addr = app_val.as_ptr().unwrap();
    let app = unsafe { &*(addr as *const MbMcpApp) };
    assert_eq!(
        app.server_name, "mcp",
        "null server app should have default server_name 'mcp'"
    );
}

// ── mb_mcp_server_name ────────────────────────────────────────────────────────

#[test]
fn server_name_happy() {
    let server_val = unsafe { mb_mcp_server_new([make_str_val("Conductor")].as_ptr(), 1) };
    let name_val = unsafe { mb_mcp_server_name([server_val].as_ptr(), 1) };
    assert!(name_val.is_ptr());
    let name = unsafe { read_str_val(name_val) };
    assert_eq!(name, "Conductor");
}

#[test]
fn server_name_null() {
    let name_val = unsafe { mb_mcp_server_name([MbValue::none()].as_ptr(), 1) };
    assert!(
        name_val.is_ptr(),
        "server_name with null ptr should return ptr (empty string)"
    );
    let name = unsafe { read_str_val(name_val) };
    assert!(
        name.is_empty(),
        "server_name for null ptr should be empty string"
    );
}
