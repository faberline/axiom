use mamba_registry::{
    convert::{mb_unwrap_native_ref, mb_wrap_native_typed},
    rt_sym, FromMbValue, IntoMbValue, MbValue, ModuleRegistrar,
};
use mambalibs_http::http_exception::HTTPException;
use std::collections::HashMap;

/// @spec .score/tech_design/projects/httpkit/http-exception.md#x-mamba-binding
#[no_mangle]
pub unsafe extern "C" fn http_exception_new(args: *const MbValue, nargs: usize) -> MbValue {
    let read = |i: usize| -> MbValue {
        if i < nargs {
            unsafe { *args.add(i) }
        } else {
            MbValue::none()
        }
    };
    let status_code = i64::from_mb_value(read(0))
        .ok()
        .map(|v| v as u16)
        .unwrap_or(500);
    let detail = String::from_mb_value(read(1)).ok();
    let headers = HashMap::<String, String>::from_mb_value(read(2)).unwrap_or_default();

    match HTTPException::new(status_code, detail, headers) {
        Ok(value) => mb_wrap_native_typed("HTTPException", value),
        Err(msg) => {
            if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
                (o.raise)("ValueError", &msg);
            }
            MbValue::none()
        }
    }
}

/// @spec .score/tech_design/projects/httpkit/http-exception.md#x-mamba-attributes.status_code
#[no_mangle]
pub unsafe extern "C" fn http_exception_get_status_code(
    args: *const MbValue,
    _nargs: usize,
) -> MbValue {
    let self_: &HTTPException = match unsafe { mb_unwrap_native_ref(*args) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    (self_.status_code as i64).into_mb_value()
}

/// @spec .score/tech_design/projects/httpkit/http-exception.md#x-mamba-attributes.detail
#[no_mangle]
pub unsafe extern "C" fn http_exception_get_detail(args: *const MbValue, _nargs: usize) -> MbValue {
    let self_: &HTTPException = match unsafe { mb_unwrap_native_ref(*args) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    self_.detail.clone().into_mb_value()
}

/// @spec .score/tech_design/projects/httpkit/http-exception.md#x-mamba-attributes.headers
#[no_mangle]
pub unsafe extern "C" fn http_exception_get_headers(
    args: *const MbValue,
    _nargs: usize,
) -> MbValue {
    let self_: &HTTPException = match unsafe { mb_unwrap_native_ref(*args) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    self_.headers.clone().into_mb_value()
}

pub fn register(r: &mut ModuleRegistrar) {
    r.add_symbol(rt_sym!(
        "HTTPException",
        http_exception_new,
        "HTTPException(status_code: int, detail: str | None = None, headers: dict | None = None) -> HTTPException"
    ));
    if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
        (o.register_getter)(
            "HTTPException",
            "status_code",
            http_exception_get_status_code,
        );
        (o.register_getter)("HTTPException", "detail", http_exception_get_detail);
        (o.register_getter)("HTTPException", "headers", http_exception_get_headers);
    }
}
