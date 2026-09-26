// MbValue is a newtype around u64; the JIT passes it by value as a 64-bit word.
#![allow(improper_ctypes_definitions)]

//! FFI functions exposed by `mambalibs.http` to Mamba scripts.
//!
//! All functions follow the Mamba native-call ABI:
//! ```text
//! extern "C" fn name(args: *const MbValue, nargs: usize) -> MbValue
//! ```
//!
//! # Exposed API
//!
//! | Symbol                       | Mamba call                                       |
//! |------------------------------|--------------------------------------------------|
//! | `mb_fetch_client_new`        | `Client(base_url, timeout?) -> client`           |
//! | `mb_fetch_client_get`        | `client.get(path) -> response`                   |
//! | `mb_fetch_client_post`       | `client.post(path, body?) -> response`           |
//! | `mb_fetch_client_put`        | `client.put(path, body?) -> response`            |
//! | `mb_fetch_client_delete`     | `client.delete(path) -> response`                |
//! | `mb_fetch_response_status`   | `response.status -> int`                         |
//! | `mb_fetch_response_text`     | `response.text() -> str`                         |
//! | `mb_fetch_response_json`     | `response.json() -> str`                         |

use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

use mamba_registry::convert::mb_wrap_native;
use mamba_registry::MbValue;
use mambalibs_http::http::Request;

use super::types::{MbHttpClient, MbHttpResponse};

// ── Global Tokio runtime ──────────────────────────────────────────────────────

static TOKIO_RT: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("mambalibs.http: failed to create Tokio runtime")
});

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

// ── mb_fetch_client_new ───────────────────────────────────────────────────────

/// Create a new HTTP client handle.
///
/// # ABI
/// ```text
/// args[0] = base_url  (MbValue::Ptr → heap String)
/// args[1] = timeout   (MbValue::Float, optional, default 30.0)
/// ```
/// Returns an opaque PTR to [`MbHttpClient`].
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_client_new(args: *const MbValue, nargs: usize) -> MbValue {
    let url_val = unsafe { arg(args, nargs, 0) };
    let timeout_val = unsafe { arg(args, nargs, 1) };

    let base_url = read_str(url_val).unwrap_or_default();
    let timeout_secs = timeout_val
        .as_float()
        .or_else(|| timeout_val.as_int().map(|i| i as f64))
        .unwrap_or(30.0);

    mb_wrap_native(MbHttpClient::new(base_url, timeout_secs))
}

// ── mb_fetch_client_get ───────────────────────────────────────────────────────

/// Perform an HTTP GET request.
///
/// # ABI
/// ```text
/// args[0] = client  (MbValue::Ptr → MbHttpClient)
/// args[1] = path    (MbValue::Ptr → heap String)
/// ```
/// Returns an opaque PTR to [`MbHttpResponse`].
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_client_get(args: *const MbValue, nargs: usize) -> MbValue {
    let client_val = unsafe { arg(args, nargs, 0) };
    let path_val = unsafe { arg(args, nargs, 1) };

    let Some(client) = read_client(client_val) else {
        return mb_wrap_native(MbHttpResponse::error());
    };
    let path = read_str(path_val).unwrap_or_default();
    let http = client.client.clone();

    let response = to_mb_response(TOKIO_RT.block_on(async move { http.get(&path).await }));
    mb_wrap_native(response)
}

// ── mb_fetch_client_post ──────────────────────────────────────────────────────

/// Perform an HTTP POST request.
///
/// # ABI
/// ```text
/// args[0] = client  (MbValue::Ptr → MbHttpClient)
/// args[1] = path    (MbValue::Ptr → heap String)
/// args[2] = body    (MbValue::Ptr → heap String, optional)
/// ```
/// Returns an opaque PTR to [`MbHttpResponse`].
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_client_post(args: *const MbValue, nargs: usize) -> MbValue {
    let client_val = unsafe { arg(args, nargs, 0) };
    let path_val = unsafe { arg(args, nargs, 1) };
    let body_val = unsafe { arg(args, nargs, 2) };

    let Some(client) = read_client(client_val) else {
        return mb_wrap_native(MbHttpResponse::error());
    };
    let path = read_str(path_val).unwrap_or_default();
    let body = read_str(body_val).unwrap_or_default();
    let http = client.client.clone();

    let response = to_mb_response(
        TOKIO_RT.block_on(async move { http.send(Request::post(path).text(body)).await }),
    );
    mb_wrap_native(response)
}

// ── mb_fetch_client_put ───────────────────────────────────────────────────────

/// Perform an HTTP PUT request.
///
/// # ABI
/// ```text
/// args[0] = client  (MbValue::Ptr → MbHttpClient)
/// args[1] = path    (MbValue::Ptr → heap String)
/// args[2] = body    (MbValue::Ptr → heap String, optional)
/// ```
/// Returns an opaque PTR to [`MbHttpResponse`].
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_client_put(args: *const MbValue, nargs: usize) -> MbValue {
    let client_val = unsafe { arg(args, nargs, 0) };
    let path_val = unsafe { arg(args, nargs, 1) };
    let body_val = unsafe { arg(args, nargs, 2) };

    let Some(client) = read_client(client_val) else {
        return mb_wrap_native(MbHttpResponse::error());
    };
    let path = read_str(path_val).unwrap_or_default();
    let body = read_str(body_val).unwrap_or_default();
    let http = client.client.clone();

    let response = to_mb_response(
        TOKIO_RT.block_on(async move { http.send(Request::put(path).text(body)).await }),
    );
    mb_wrap_native(response)
}

// ── mb_fetch_client_delete ────────────────────────────────────────────────────

/// Perform an HTTP DELETE request.
///
/// # ABI
/// ```text
/// args[0] = client  (MbValue::Ptr → MbHttpClient)
/// args[1] = path    (MbValue::Ptr → heap String)
/// ```
/// Returns an opaque PTR to [`MbHttpResponse`].
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_client_delete(args: *const MbValue, nargs: usize) -> MbValue {
    let client_val = unsafe { arg(args, nargs, 0) };
    let path_val = unsafe { arg(args, nargs, 1) };

    let Some(client) = read_client(client_val) else {
        return mb_wrap_native(MbHttpResponse::error());
    };
    let path = read_str(path_val).unwrap_or_default();
    let http = client.client.clone();

    let response = to_mb_response(TOKIO_RT.block_on(async move { http.delete(&path).await }));
    mb_wrap_native(response)
}

// ── mb_fetch_response_status ──────────────────────────────────────────────────

/// Get the HTTP status code from a response.
///
/// # ABI
/// ```text
/// args[0] = response  (MbValue::Ptr → MbHttpResponse)
/// ```
/// Returns `MbValue::Int(status)`.
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_response_status(args: *const MbValue, nargs: usize) -> MbValue {
    let resp_val = unsafe { arg(args, nargs, 0) };

    let addr = match resp_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return MbValue::from_int(0),
    };
    let resp = unsafe { &*(addr as *const MbHttpResponse) };
    MbValue::from_int(resp.status as i64)
}

// ── mb_fetch_response_text ────────────────────────────────────────────────────

/// Get the response body as text.
///
/// # ABI
/// ```text
/// args[0] = response  (MbValue::Ptr → MbHttpResponse)
/// ```
/// Returns `MbValue::Ptr → heap String`.
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_response_text(args: *const MbValue, nargs: usize) -> MbValue {
    let resp_val = unsafe { arg(args, nargs, 0) };

    let addr = match resp_val.as_ptr() {
        Some(a) if a != 0 => a,
        _ => return wrap_str(String::new()),
    };
    let resp = unsafe { &*(addr as *const MbHttpResponse) };
    wrap_str(resp.body.clone())
}

// ── mb_fetch_response_json ────────────────────────────────────────────────────

/// Get the response body as JSON (same as text for now).
///
/// # ABI
/// ```text
/// args[0] = response  (MbValue::Ptr → MbHttpResponse)
/// ```
/// Returns `MbValue::Ptr → heap String`.
#[no_mangle]
pub unsafe extern "C" fn mb_fetch_response_json(args: *const MbValue, nargs: usize) -> MbValue {
    // Same as response_text for now — body is already raw text/JSON.
    unsafe { mb_fetch_response_text(args, nargs) }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Extract the core-backed HTTP client from an MbHttpClient pointer.
fn read_client(v: MbValue) -> Option<&'static MbHttpClient> {
    v.as_ptr().and_then(|addr| {
        if addr == 0 {
            return None;
        }
        let client = unsafe { &*(addr as *const MbHttpClient) };
        Some(client)
    })
}

fn to_mb_response(
    result: mambalibs_http::client::HttpResult<mambalibs_http::http::Response>,
) -> MbHttpResponse {
    match result {
        Ok(response) => {
            let status = response.status_code;
            let body = response
                .text()
                .unwrap_or_else(|_| String::from_utf8_lossy(response.bytes()).into_owned());
            MbHttpResponse::ok(status, body)
        }
        Err(_) => MbHttpResponse::error(),
    }
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
    fn test_client_new() {
        let url_val = make_str_val("https://api.example.com");
        let timeout_val = MbValue::from_float(10.0);
        let args = [url_val, timeout_val];
        let client_val = unsafe { mb_fetch_client_new(args.as_ptr(), 2) };
        assert!(client_val.is_ptr(), "client should be a ptr");

        let addr = client_val.as_ptr().unwrap();
        let client = unsafe { &*(addr as *const MbHttpClient) };
        assert_eq!(client.base_url, "https://api.example.com");
        assert!((client.timeout_secs - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_client_new_default_timeout() {
        let url_val = make_str_val("https://example.com");
        let args = [url_val];
        let client_val = unsafe { mb_fetch_client_new(args.as_ptr(), 1) };
        let addr = client_val.as_ptr().unwrap();
        let client = unsafe { &*(addr as *const MbHttpClient) };
        assert!((client.timeout_secs - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_response_status() {
        let resp = MbHttpResponse::ok(200, "hello");
        let resp_val = mb_wrap_native(resp);

        let args = [resp_val];
        let status_val = unsafe { mb_fetch_response_status(args.as_ptr(), 1) };
        assert!(status_val.is_int());
        assert_eq!(status_val.as_int(), Some(200));
    }

    #[test]
    fn test_response_text() {
        let resp = MbHttpResponse::ok(200, "body content");
        let resp_val = mb_wrap_native(resp);

        let args = [resp_val];
        let text_val = unsafe { mb_fetch_response_text(args.as_ptr(), 1) };
        assert!(text_val.is_ptr());
        let text = unsafe { text_val.as_obj_str() }.unwrap();
        assert_eq!(text, "body content");
    }

    #[test]
    fn test_response_error() {
        let resp = MbHttpResponse::error();
        assert_eq!(resp.status, 0);
        assert_eq!(resp.body, "error");
    }
}
