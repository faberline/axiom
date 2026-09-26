use mamba_registry::{
    convert::{mb_unwrap_native_ref, mb_wrap_native_typed},
    rt_sym, FromMbValue, IntoMbValue, MbValue, ModuleRegistrar,
};
use mambalibs_http::request_response::{Cookie, Request, Response};
use std::collections::HashMap;

unsafe fn read(args: *const MbValue, nargs: usize, index: usize) -> MbValue {
    if index < nargs {
        *args.add(index)
    } else {
        MbValue::none()
    }
}

/// @spec .score/tech_design/projects/httpkit/request-response.md#x-mamba-binding
#[no_mangle]
pub unsafe extern "C" fn cookie_new(args: *const MbValue, nargs: usize) -> MbValue {
    let name = String::from_mb_value(read(args, nargs, 0)).unwrap_or_default();
    let value = String::from_mb_value(read(args, nargs, 1)).unwrap_or_default();
    let path = String::from_mb_value(read(args, nargs, 2)).ok();
    let domain = String::from_mb_value(read(args, nargs, 3)).ok();
    let secure = bool::from_mb_value(read(args, nargs, 4)).unwrap_or(false);
    let http_only = bool::from_mb_value(read(args, nargs, 5)).unwrap_or(false);
    let max_age = i64::from_mb_value(read(args, nargs, 6)).ok();

    match Cookie::new(name, value, path, domain, secure, http_only, max_age) {
        Ok(value) => mb_wrap_native_typed("Cookie", value),
        Err(msg) => {
            if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
                (o.raise)("ValueError", &msg);
            }
            MbValue::none()
        }
    }
}

/// @spec .score/tech_design/projects/httpkit/request-response.md#x-mamba-binding
#[no_mangle]
pub unsafe extern "C" fn request_new(args: *const MbValue, nargs: usize) -> MbValue {
    let method = String::from_mb_value(read(args, nargs, 0)).unwrap_or_default();
    let path = String::from_mb_value(read(args, nargs, 1)).unwrap_or_default();
    let query_params =
        HashMap::<String, String>::from_mb_value(read(args, nargs, 2)).unwrap_or_default();
    let headers =
        HashMap::<String, String>::from_mb_value(read(args, nargs, 3)).unwrap_or_default();
    let path_params =
        HashMap::<String, String>::from_mb_value(read(args, nargs, 6)).unwrap_or_default();

    match Request::new(
        method,
        path,
        query_params,
        headers,
        Vec::new(),
        Vec::new(),
        path_params,
    ) {
        Ok(value) => mb_wrap_native_typed("Request", value),
        Err(msg) => {
            if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
                (o.raise)("ValueError", &msg);
            }
            MbValue::none()
        }
    }
}

/// @spec .score/tech_design/projects/httpkit/request-response.md#x-mamba-binding
#[no_mangle]
pub unsafe extern "C" fn response_new(args: *const MbValue, nargs: usize) -> MbValue {
    let status_code = i64::from_mb_value(read(args, nargs, 0))
        .ok()
        .map(|v| v as u16)
        .unwrap_or(200);
    let headers =
        HashMap::<String, String>::from_mb_value(read(args, nargs, 2)).unwrap_or_default();
    let media_type = String::from_mb_value(read(args, nargs, 4))
        .unwrap_or_else(|_| "application/json".to_string());

    match Response::new(status_code, Vec::new(), headers, Vec::new(), media_type) {
        Ok(value) => mb_wrap_native_typed("Response", value),
        Err(msg) => {
            if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
                (o.raise)("ValueError", &msg);
            }
            MbValue::none()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn cookie_get_name(args: *const MbValue, _nargs: usize) -> MbValue {
    let self_: &Cookie = match unsafe { mb_unwrap_native_ref(*args) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    self_.name.clone().into_mb_value()
}

#[no_mangle]
pub unsafe extern "C" fn request_get_method(args: *const MbValue, _nargs: usize) -> MbValue {
    let self_: &Request = match unsafe { mb_unwrap_native_ref(*args) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    self_.method.clone().into_mb_value()
}

#[no_mangle]
pub unsafe extern "C" fn request_get_path(args: *const MbValue, _nargs: usize) -> MbValue {
    let self_: &Request = match unsafe { mb_unwrap_native_ref(*args) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    self_.path.clone().into_mb_value()
}

#[no_mangle]
pub unsafe extern "C" fn response_get_status_code(args: *const MbValue, _nargs: usize) -> MbValue {
    let self_: &Response = match unsafe { mb_unwrap_native_ref(*args) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    (self_.status_code as i64).into_mb_value()
}

#[no_mangle]
pub unsafe extern "C" fn response_get_media_type(args: *const MbValue, _nargs: usize) -> MbValue {
    let self_: &Response = match unsafe { mb_unwrap_native_ref(*args) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    self_.media_type.clone().into_mb_value()
}

pub fn register(r: &mut ModuleRegistrar) {
    r.add_symbol(rt_sym!(
        "Cookie",
        cookie_new,
        "Cookie(name: str, value: str, path: str | None = None, domain: str | None = None, secure: bool = False, http_only: bool = False, max_age: int | None = None) -> Cookie"
    ));
    r.add_symbol(rt_sym!(
        "Request",
        request_new,
        "Request(method: str, path: str) -> Request"
    ));
    r.add_symbol(rt_sym!(
        "Response",
        response_new,
        "Response(status_code: int = 200, media_type: str = 'application/json') -> Response"
    ));
    if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
        (o.register_getter)("Cookie", "name", cookie_get_name);
        (o.register_getter)("Request", "method", request_get_method);
        (o.register_getter)("Request", "path", request_get_path);
        (o.register_getter)("Response", "status_code", response_get_status_code);
        (o.register_getter)("Response", "media_type", response_get_media_type);
    }
}
