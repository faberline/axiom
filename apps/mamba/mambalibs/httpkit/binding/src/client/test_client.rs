//! HTTP test client helper for Mamba tests.
//!
//! `MbTestClient` dispatches in-process against the native `mambalibs.http`
//! `App` registry. It is intentionally not an ASGI/WSGI client; it gives
//! FastAPI-style test ergonomics over the Mamba-native host contract.

#![allow(improper_ctypes_definitions)]

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use mamba_registry::convert::{mb_unwrap_native_ref, mb_wrap_native_typed};
use mamba_registry::{ops, MbValue};
use serde_json::Value as JsonValue;

use crate::app::{app_dispatch_handler_json, app_preflight_json};

type NativeFn = unsafe extern "C" fn(*const MbValue, usize) -> MbValue;

thread_local! {
    static ACTIVE_TEST_CLIENT: RefCell<Option<MbValue>> = const { RefCell::new(None) };
    static ACTIVE_TEST_RESPONSE: RefCell<Option<MbValue>> = const { RefCell::new(None) };
}

// ── MbTestClient ──────────────────────────────────────────────────────────────

/// In-process test client bound to a native `mambalibs.http` App handle.
pub struct MbTestClient {
    /// Native App handle.
    pub app: MbValue,
    /// Optional DI Container, RequestScope, or dependency dict.
    pub provider: MbValue,
    /// Stable pseudo-base URL for compatibility with existing helpers.
    pub base_url: String,
    /// Shutdown signal for future native-host attachment.
    shutdown: Arc<AtomicBool>,
}

impl MbTestClient {
    fn new(app: MbValue, provider: MbValue) -> Self {
        Self {
            app,
            provider,
            base_url: "http://testserver".to_string(),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Resolve the pseudo-base URL for compatibility with URL-based tests.
    pub fn get_url(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        let path = path.trim_start_matches('/');
        format!("{base}/{path}")
    }
}

impl Drop for MbTestClient {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

/// Response from the in-process test dispatch.
pub struct MbTestResponse {
    pub status: u16,
    pub body: String,
}

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
    (ops().str_read)(v)
}

fn wrap_str(s: String) -> MbValue {
    (ops().str_new)(&s)
}

fn native_func(func: NativeFn) -> MbValue {
    MbValue::from_func(func as usize)
}

fn bind_test_client(receiver: MbValue) {
    ACTIVE_TEST_CLIENT.with(|slot| {
        *slot.borrow_mut() = Some(receiver);
    });
}

fn active_test_client() -> Option<MbValue> {
    ACTIVE_TEST_CLIENT.with(|slot| *slot.borrow())
}

fn bind_test_response(receiver: MbValue) {
    ACTIVE_TEST_RESPONSE.with(|slot| {
        *slot.borrow_mut() = Some(receiver);
    });
}

fn active_test_response() -> Option<MbValue> {
    ACTIVE_TEST_RESPONSE.with(|slot| *slot.borrow())
}

unsafe fn read_client(value: MbValue) -> Option<&'static MbTestClient> {
    unsafe { mb_unwrap_native_ref::<MbTestClient>(value) }
}

unsafe fn read_response(value: MbValue) -> Option<&'static MbTestResponse> {
    unsafe { mb_unwrap_native_ref::<MbTestResponse>(value) }
}

fn empty_dict() -> MbValue {
    (ops().dict_new)()
}

fn dispatch_preflight(
    client: &MbTestClient,
    method: &str,
    path: MbValue,
    body: MbValue,
) -> MbValue {
    let method_value = wrap_str(method.to_string());
    let body_value = if body.is_none() { empty_dict() } else { body };
    let args = [client.app, method_value, path, body_value, client.provider];
    unsafe { app_preflight_json(args.as_ptr(), args.len()) }
}

fn dispatch_request(client: &MbTestClient, method: &str, path: MbValue, body: MbValue) -> MbValue {
    let report_value = dispatch_preflight(client, method, path, body);
    let report_body = read_str(report_value).unwrap_or_else(|| {
        r#"{"matched":false,"status_code":500,"errors":["invalid preflight report"]}"#.to_string()
    });
    let status = status_from_report(&report_body);
    if status >= 400 {
        return wrap_response_from_report(wrap_str(report_body));
    }

    let Some(path_text) = read_str(path) else {
        return wrap_response_from_report(wrap_str(report_body));
    };
    if let Some((status, body)) = app_dispatch_handler_json(client.app, method, &path_text, status)
    {
        return mb_wrap_native_typed("TestResponse", MbTestResponse { status, body });
    }

    wrap_response_from_report(wrap_str(report_body))
}

fn status_from_report(report: &str) -> u16 {
    serde_json::from_str::<JsonValue>(report)
        .ok()
        .and_then(|doc| doc.get("status_code").and_then(JsonValue::as_u64))
        .and_then(|status| u16::try_from(status).ok())
        .unwrap_or(0)
}

fn json_to_mb_value(value: &JsonValue) -> MbValue {
    match value {
        JsonValue::Null => MbValue::none(),
        JsonValue::Bool(value) => MbValue::from_bool(*value),
        JsonValue::Number(value) => value
            .as_i64()
            .map(MbValue::from_int)
            .or_else(|| value.as_f64().map(MbValue::from_float))
            .unwrap_or_else(MbValue::none),
        JsonValue::String(value) => wrap_str(value.clone()),
        JsonValue::Array(items) => {
            let values = items.iter().map(json_to_mb_value).collect();
            (ops().list_new)(values)
        }
        JsonValue::Object(fields) => {
            let dict = (ops().dict_new)();
            for (key, value) in fields {
                (ops().dict_insert_str)(dict, key, json_to_mb_value(value));
            }
            dict
        }
    }
}

fn wrap_response_from_report(report_value: MbValue) -> MbValue {
    let body = read_str(report_value).unwrap_or_else(|| {
        r#"{"matched":false,"status_code":500,"errors":["invalid preflight report"]}"#.to_string()
    });
    let status = status_from_report(&body);
    mb_wrap_native_typed("TestResponse", MbTestResponse { status, body })
}

fn wrap_response(result: Result<(u16, String), String>) -> MbValue {
    let (status, body) = match result {
        Ok(pair) => pair,
        Err(e) => (0, format!("error: {e}")),
    };
    mb_wrap_native_typed("TestResponse", MbTestResponse { status, body })
}

// ── mb_fetch_test_client_new ─────────────────────────────────────────────────

/// Return a test client bound to the native Mamba application.
///
/// # ABI
/// ```text
/// args[0] = app_handle
/// args[1] = provider handle, optional
/// ```
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_test_client_new(args: *const MbValue, nargs: usize) -> MbValue {
    let app = unsafe { arg(args, nargs, 0) };
    let provider = unsafe { arg(args, nargs, 1) };
    mb_wrap_native_typed("TestClient", MbTestClient::new(app, provider))
}

// ── mb_fetch_test_client_close ────────────────────────────────────────────────

/// Signal the test client to shut down.
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_test_client_close(args: *const MbValue, nargs: usize) -> MbValue {
    let client_val = unsafe { arg(args, nargs, 0) };
    if let Some(client) = unsafe { read_client(client_val) } {
        client.shutdown.store(true, Ordering::Relaxed);
    }
    MbValue::none()
}

// ── mb_fetch_test_client_get ──────────────────────────────────────────────────

/// Perform an in-process HTTP GET request.
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_test_client_get(args: *const MbValue, nargs: usize) -> MbValue {
    let client_val = unsafe { arg(args, nargs, 0) };
    let path_val = unsafe { arg(args, nargs, 1) };

    let Some(client) = (unsafe { read_client(client_val) }) else {
        return wrap_response(Err("invalid test client".to_string()));
    };

    dispatch_request(client, "GET", path_val, empty_dict())
}

// ── mb_fetch_test_client_post ─────────────────────────────────────────────────

/// Perform an in-process HTTP POST request.
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_test_client_post(args: *const MbValue, nargs: usize) -> MbValue {
    let client_val = unsafe { arg(args, nargs, 0) };
    let path_val = unsafe { arg(args, nargs, 1) };
    let body_val = unsafe { arg(args, nargs, 2) };

    let Some(client) = (unsafe { read_client(client_val) }) else {
        return wrap_response(Err("invalid test client".to_string()));
    };

    dispatch_request(client, "POST", path_val, body_val)
}

// ── Bound TestClient methods ─────────────────────────────────────────────────

#[no_mangle]
pub unsafe extern "C" fn mb_fetch_test_client_get_bound(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let Some(client) = active_test_client() else {
        return wrap_response(Err("unbound test client".to_string()));
    };
    let path = unsafe { arg(args, nargs, 0) };
    let forwarded = [client, path];
    unsafe { mb_fetch_test_client_get(forwarded.as_ptr(), forwarded.len()) }
}

#[no_mangle]
pub unsafe extern "C" fn mb_fetch_test_client_post_bound(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let Some(client) = active_test_client() else {
        return wrap_response(Err("unbound test client".to_string()));
    };
    let path = unsafe { arg(args, nargs, 0) };
    let body = unsafe { arg(args, nargs, 1) };
    let forwarded = [client, path, body];
    unsafe { mb_fetch_test_client_post(forwarded.as_ptr(), forwarded.len()) }
}

#[no_mangle]
pub unsafe extern "C" fn mb_fetch_test_client_close_bound(
    _args: *const MbValue,
    _nargs: usize,
) -> MbValue {
    let Some(client) = active_test_client() else {
        return MbValue::none();
    };
    let forwarded = [client];
    unsafe { mb_fetch_test_client_close(forwarded.as_ptr(), forwarded.len()) }
}

pub unsafe extern "C" fn get_test_client_get(args: *const MbValue, nargs: usize) -> MbValue {
    bind_test_client(unsafe { arg(args, nargs, 0) });
    native_func(mb_fetch_test_client_get_bound)
}

pub unsafe extern "C" fn get_test_client_post(args: *const MbValue, nargs: usize) -> MbValue {
    bind_test_client(unsafe { arg(args, nargs, 0) });
    native_func(mb_fetch_test_client_post_bound)
}

pub unsafe extern "C" fn get_test_client_close(args: *const MbValue, nargs: usize) -> MbValue {
    bind_test_client(unsafe { arg(args, nargs, 0) });
    native_func(mb_fetch_test_client_close_bound)
}

// ── Response accessors ────────────────────────────────────────────────────────

/// Get the HTTP status code from a test response.
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_test_client_status(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let resp_val = unsafe { arg(args, nargs, 0) };
    match unsafe { read_response(resp_val) } {
        Some(resp) => MbValue::from_int(resp.status as i64),
        None => MbValue::from_int(0),
    }
}

/// Get the response body as text.
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_test_client_text(args: *const MbValue, nargs: usize) -> MbValue {
    let resp_val = unsafe { arg(args, nargs, 0) };
    match unsafe { read_response(resp_val) } {
        Some(resp) => wrap_str(resp.body.clone()),
        None => wrap_str(String::new()),
    }
}

/// Get the response body as a Mamba JSON value.
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_test_client_json(args: *const MbValue, nargs: usize) -> MbValue {
    let resp_val = unsafe { arg(args, nargs, 0) };
    let Some(resp) = (unsafe { read_response(resp_val) }) else {
        return MbValue::none();
    };
    serde_json::from_str::<JsonValue>(&resp.body)
        .map(|value| json_to_mb_value(&value))
        .unwrap_or_else(|_| wrap_str(resp.body.clone()))
}

#[no_mangle]
pub unsafe extern "C" fn mb_fetch_test_client_text_bound(
    _args: *const MbValue,
    _nargs: usize,
) -> MbValue {
    let Some(response) = active_test_response() else {
        return wrap_str(String::new());
    };
    let forwarded = [response];
    unsafe { mb_fetch_test_client_text(forwarded.as_ptr(), forwarded.len()) }
}

#[no_mangle]
pub unsafe extern "C" fn mb_fetch_test_client_json_bound(
    _args: *const MbValue,
    _nargs: usize,
) -> MbValue {
    let Some(response) = active_test_response() else {
        return MbValue::none();
    };
    let forwarded = [response];
    unsafe { mb_fetch_test_client_json(forwarded.as_ptr(), forwarded.len()) }
}

pub unsafe extern "C" fn get_test_response_status(args: *const MbValue, nargs: usize) -> MbValue {
    unsafe { mb_fetch_test_client_status(args, nargs) }
}

pub unsafe extern "C" fn get_test_response_text(args: *const MbValue, nargs: usize) -> MbValue {
    bind_test_response(unsafe { arg(args, nargs, 0) });
    native_func(mb_fetch_test_client_text_bound)
}

pub unsafe extern "C" fn get_test_response_json(args: *const MbValue, nargs: usize) -> MbValue {
    bind_test_response(unsafe { arg(args, nargs, 0) });
    native_func(mb_fetch_test_client_json_bound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mamba_registry::convert::native_type_name;

    #[test]
    fn test_client_new_returns_typed_ptr() {
        mamba_registry::test_ops::init();
        let args: [MbValue; 0] = [];
        let client_val = unsafe { mb_fetch_test_client_new(args.as_ptr(), 0) };
        assert!(client_val.is_ptr());
        assert_eq!(native_type_name(client_val), Some("TestClient"));

        let client = unsafe { read_client(client_val).expect("typed test client") };
        assert_eq!(client.base_url, "http://testserver");
    }

    #[test]
    fn test_client_close_noop_on_null() {
        mamba_registry::test_ops::init();
        let args = [MbValue::none()];
        let result = unsafe { mb_fetch_test_client_close(args.as_ptr(), 1) };
        assert!(result.is_none());
    }

    #[test]
    fn test_response_status() {
        mamba_registry::test_ops::init();
        let resp = MbTestResponse {
            status: 200,
            body: "ok".to_string(),
        };
        let resp_val = mb_wrap_native_typed("TestResponse", resp);
        let args = [resp_val];
        let status_val = unsafe { mb_fetch_test_client_status(args.as_ptr(), 1) };
        assert_eq!(status_val.as_int(), Some(200));
    }

    #[test]
    fn test_response_text() {
        mamba_registry::test_ops::init();
        let resp = MbTestResponse {
            status: 200,
            body: r#"{"ok":true}"#.to_string(),
        };
        let resp_val = mb_wrap_native_typed("TestResponse", resp);
        let args = [resp_val];
        let text_val = unsafe { mb_fetch_test_client_text(args.as_ptr(), 1) };
        assert!(text_val.is_ptr());
        let text = (ops().str_read)(text_val).unwrap();
        assert!(text.contains("ok"));
    }

    #[test]
    fn test_response_json_returns_dict() {
        mamba_registry::test_ops::init();
        let resp = MbTestResponse {
            status: 200,
            body: r#"{"ok":true}"#.to_string(),
        };
        let resp_val = mb_wrap_native_typed("TestResponse", resp);
        let args = [resp_val];
        let json_val = unsafe { mb_fetch_test_client_json(args.as_ptr(), 1) };
        assert_eq!(
            (ops().dict_get_str)(json_val, "ok").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_client_resolves_paths_against_pseudo_host() {
        let client = MbTestClient::new(MbValue::none(), MbValue::none());

        assert_eq!(
            client.get_url("/api/health"),
            "http://testserver/api/health"
        );
        assert_eq!(client.get_url("api/status"), "http://testserver/api/status");
    }

    #[test]
    #[ignore = "placeholder for sqlite in-memory DB test"]
    fn sqlite_fixture_in_memory() {
        let url = "sqlite://:memory:";
        assert!(!url.is_empty());
        assert!(url.contains("sqlite"));
    }
}
