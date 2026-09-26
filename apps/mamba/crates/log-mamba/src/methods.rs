// MbValue is a newtype around u64; the JIT passes it by value as a 64-bit word.
#![allow(improper_ctypes_definitions)]

//! FFI functions exposed by `log-mamba` to Mamba scripts.
//!
//! All functions follow the Mamba native-call ABI:
//! ```text
//! extern "C" fn name(args: *const MbValue, nargs: usize) -> MbValue
//! ```
//!
//! # Exposed API
//!
//! | Symbol               | Mamba call                              |
//! |----------------------|-----------------------------------------|
//! | `mb_log_get_logger`  | `get_logger(name: str) -> Logger`       |
//! | `mb_log_info`        | `logger.info(msg: str) -> None`         |
//! | `mb_log_error`       | `logger.error(msg: str) -> None`        |
//! | `mb_log_debug`       | `logger.debug(msg: str) -> None`        |
//! | `mb_log_warning`     | `logger.warning(msg: str) -> None`      |

use mamba_registry::convert::mb_wrap_native;
use mamba_registry::MbValue;

use crate::types::MbLogger;

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

// ── mb_log_get_logger ─────────────────────────────────────────────────────────

/// Create a logger handle with the given name.
///
/// # ABI
/// ```text
/// args[0] = name  (MbValue::Ptr → heap String)
/// ```
/// Returns an opaque PTR to [`MbLogger`].
#[no_mangle]
pub unsafe extern "C" fn mb_log_get_logger(args: *const MbValue, nargs: usize) -> MbValue {
    let name_val = unsafe { arg(args, nargs, 0) };
    let name = read_str(name_val).unwrap_or_else(|| "root".to_string());
    mb_wrap_native(MbLogger::new(name))
}

// ── mb_log_info ───────────────────────────────────────────────────────────────

/// Log a message at INFO level.
///
/// # ABI
/// ```text
/// args[0] = logger  (MbValue::Ptr → MbLogger)
/// args[1] = msg     (MbValue::Ptr → heap String)
/// ```
/// Returns `MbValue::none()`.
#[no_mangle]
pub unsafe extern "C" fn mb_log_info(args: *const MbValue, nargs: usize) -> MbValue {
    let logger_val = unsafe { arg(args, nargs, 0) };
    let msg_val = unsafe { arg(args, nargs, 1) };

    let name = logger_val
        .as_ptr()
        .map(|addr| {
            if addr == 0 {
                "root".to_string()
            } else {
                unsafe { &*(addr as *const MbLogger) }.name.clone()
            }
        })
        .unwrap_or_else(|| "root".to_string());

    let msg = read_str(msg_val).unwrap_or_default();
    eprintln!("{{\"level\":\"info\",\"logger\":\"{name}\",\"msg\":\"{msg}\"}}");
    MbValue::none()
}

// ── mb_log_error ──────────────────────────────────────────────────────────────

/// Log a message at ERROR level.
///
/// # ABI
/// ```text
/// args[0] = logger  (MbValue::Ptr → MbLogger)
/// args[1] = msg     (MbValue::Ptr → heap String)
/// ```
/// Returns `MbValue::none()`.
#[no_mangle]
pub unsafe extern "C" fn mb_log_error(args: *const MbValue, nargs: usize) -> MbValue {
    let logger_val = unsafe { arg(args, nargs, 0) };
    let msg_val = unsafe { arg(args, nargs, 1) };

    let name = logger_val
        .as_ptr()
        .map(|addr| {
            if addr == 0 {
                "root".to_string()
            } else {
                unsafe { &*(addr as *const MbLogger) }.name.clone()
            }
        })
        .unwrap_or_else(|| "root".to_string());

    let msg = read_str(msg_val).unwrap_or_default();
    eprintln!("{{\"level\":\"error\",\"logger\":\"{name}\",\"msg\":\"{msg}\"}}");
    MbValue::none()
}

// ── mb_log_debug ──────────────────────────────────────────────────────────────

/// Log a message at DEBUG level.
///
/// # ABI
/// ```text
/// args[0] = logger  (MbValue::Ptr → MbLogger)
/// args[1] = msg     (MbValue::Ptr → heap String)
/// ```
/// Returns `MbValue::none()`.
#[no_mangle]
pub unsafe extern "C" fn mb_log_debug(args: *const MbValue, nargs: usize) -> MbValue {
    let logger_val = unsafe { arg(args, nargs, 0) };
    let msg_val = unsafe { arg(args, nargs, 1) };

    let name = logger_val
        .as_ptr()
        .map(|addr| {
            if addr == 0 {
                "root".to_string()
            } else {
                unsafe { &*(addr as *const MbLogger) }.name.clone()
            }
        })
        .unwrap_or_else(|| "root".to_string());

    let msg = read_str(msg_val).unwrap_or_default();
    eprintln!("{{\"level\":\"debug\",\"logger\":\"{name}\",\"msg\":\"{msg}\"}}");
    MbValue::none()
}

// ── mb_log_warning ────────────────────────────────────────────────────────────

/// Log a message at WARNING level.
///
/// # ABI
/// ```text
/// args[0] = logger  (MbValue::Ptr → MbLogger)
/// args[1] = msg     (MbValue::Ptr → heap String)
/// ```
/// Returns `MbValue::none()`.
#[no_mangle]
pub unsafe extern "C" fn mb_log_warning(args: *const MbValue, nargs: usize) -> MbValue {
    let logger_val = unsafe { arg(args, nargs, 0) };
    let msg_val = unsafe { arg(args, nargs, 1) };

    let name = logger_val
        .as_ptr()
        .map(|addr| {
            if addr == 0 {
                "root".to_string()
            } else {
                unsafe { &*(addr as *const MbLogger) }.name.clone()
            }
        })
        .unwrap_or_else(|| "root".to_string());

    let msg = read_str(msg_val).unwrap_or_default();
    eprintln!("{{\"level\":\"warning\",\"logger\":\"{name}\",\"msg\":\"{msg}\"}}");
    MbValue::none()
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
    fn test_get_logger() {
        let name_val = make_str_val("myapp");
        let args = [name_val];
        let logger_val = unsafe { mb_log_get_logger(args.as_ptr(), 1) };
        assert!(logger_val.is_ptr(), "get_logger should return a ptr");

        let addr = logger_val.as_ptr().unwrap();
        let logger = unsafe { &*(addr as *const MbLogger) };
        assert_eq!(logger.name, "myapp");
    }

    #[test]
    fn test_info() {
        let name_val = make_str_val("test_logger");
        let args = [name_val];
        let logger_val = unsafe { mb_log_get_logger(args.as_ptr(), 1) };

        let msg_val = make_str_val("hello world");
        let log_args = [logger_val, msg_val];
        let result = unsafe { mb_log_info(log_args.as_ptr(), 2) };
        assert!(result.is_none(), "info returns None");
    }

    #[test]
    fn test_error() {
        let name_val = make_str_val("err_logger");
        let args = [name_val];
        let logger_val = unsafe { mb_log_get_logger(args.as_ptr(), 1) };

        let msg_val = make_str_val("something went wrong");
        let log_args = [logger_val, msg_val];
        let result = unsafe { mb_log_error(log_args.as_ptr(), 2) };
        assert!(result.is_none(), "error returns None");
    }

    #[test]
    fn test_debug() {
        let name_val = make_str_val("dbg_logger");
        let args = [name_val];
        let logger_val = unsafe { mb_log_get_logger(args.as_ptr(), 1) };

        let msg_val = make_str_val("debugging info");
        let log_args = [logger_val, msg_val];
        let result = unsafe { mb_log_debug(log_args.as_ptr(), 2) };
        assert!(result.is_none(), "debug returns None");
    }

    #[test]
    fn test_warning() {
        let name_val = make_str_val("warn_logger");
        let args = [name_val];
        let logger_val = unsafe { mb_log_get_logger(args.as_ptr(), 1) };

        let msg_val = make_str_val("a warning message");
        let log_args = [logger_val, msg_val];
        let result = unsafe { mb_log_warning(log_args.as_ptr(), 2) };
        assert!(result.is_none(), "warning returns None");
    }
}
