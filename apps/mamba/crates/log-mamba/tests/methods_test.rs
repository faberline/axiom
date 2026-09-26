// Integration tests for log-mamba: covers all 5 mb_log_* functions.
// Requirements: R1, R2, R4, R5, R6
#![allow(improper_ctypes_definitions)]

use log_mamba::methods::{
    mb_log_debug, mb_log_error, mb_log_get_logger, mb_log_info, mb_log_warning,
};
use log_mamba::types::MbLogger;
use mamba_registry::MbValue;

// ── Shared helpers ─────────────────────────────────────────────────────────────

fn make_str_val(s: &str) -> MbValue {
    mamba_registry::test_ops::init();
    mamba_registry::rc::wrap_obj_str(s.to_string())
}

// ── mb_log_get_logger ─────────────────────────────────────────────────────────

#[test]
fn get_logger_named() {
    let args = [make_str_val("myapp")];
    let logger_val = unsafe { mb_log_get_logger(args.as_ptr(), 1) };
    assert!(logger_val.is_ptr(), "get_logger should return a ptr");
    let addr = logger_val.as_ptr().unwrap();
    let logger = unsafe { &*(addr as *const MbLogger) };
    assert_eq!(logger.name, "myapp");
}

#[test]
fn get_logger_default() {
    // No args → name defaults to "root"
    let args: [MbValue; 0] = [];
    let logger_val = unsafe { mb_log_get_logger(args.as_ptr(), 0) };
    assert!(logger_val.is_ptr());
    let addr = logger_val.as_ptr().unwrap();
    let logger = unsafe { &*(addr as *const MbLogger) };
    assert_eq!(logger.name, "root", "default logger name should be 'root'");
}

#[test]
fn get_logger_empty() {
    // Empty string name should be preserved
    let args = [make_str_val("")];
    let logger_val = unsafe { mb_log_get_logger(args.as_ptr(), 1) };
    assert!(logger_val.is_ptr());
    let addr = logger_val.as_ptr().unwrap();
    let logger = unsafe { &*(addr as *const MbLogger) };
    assert_eq!(logger.name, "", "empty name should be preserved");
}

// ── mb_log_info ───────────────────────────────────────────────────────────────

#[test]
fn log_info_returns_none() {
    let logger_val = unsafe { mb_log_get_logger([make_str_val("test")].as_ptr(), 1) };
    let args = [logger_val, make_str_val("application started")];
    let result = unsafe { mb_log_info(args.as_ptr(), 2) };
    assert!(result.is_none(), "mb_log_info should return none()");
}

#[test]
fn log_info_null_logger() {
    // Null logger ptr should not crash; uses "root" name fallback
    let args = [MbValue::none(), make_str_val("some message")];
    let result = unsafe { mb_log_info(args.as_ptr(), 2) };
    assert!(
        result.is_none(),
        "mb_log_info with null logger should return none() without crash"
    );
}

// ── mb_log_error ──────────────────────────────────────────────────────────────

#[test]
fn log_error_returns_none() {
    let logger_val = unsafe { mb_log_get_logger([make_str_val("err")].as_ptr(), 1) };
    let args = [logger_val, make_str_val("something went wrong")];
    let result = unsafe { mb_log_error(args.as_ptr(), 2) };
    assert!(result.is_none(), "mb_log_error should return none()");
}

#[test]
fn log_error_null_logger() {
    let args = [MbValue::none(), make_str_val("error msg")];
    let result = unsafe { mb_log_error(args.as_ptr(), 2) };
    assert!(
        result.is_none(),
        "mb_log_error with null logger should return none()"
    );
}

// ── mb_log_debug ──────────────────────────────────────────────────────────────

#[test]
fn log_debug_returns_none() {
    let logger_val = unsafe { mb_log_get_logger([make_str_val("dbg")].as_ptr(), 1) };
    let args = [logger_val, make_str_val("debug info here")];
    let result = unsafe { mb_log_debug(args.as_ptr(), 2) };
    assert!(result.is_none(), "mb_log_debug should return none()");
}

#[test]
fn log_debug_null_logger() {
    let args = [MbValue::none(), make_str_val("debug msg")];
    let result = unsafe { mb_log_debug(args.as_ptr(), 2) };
    assert!(
        result.is_none(),
        "mb_log_debug with null logger should return none()"
    );
}

// ── mb_log_warning ────────────────────────────────────────────────────────────

#[test]
fn log_warning_returns_none() {
    let logger_val = unsafe { mb_log_get_logger([make_str_val("warn")].as_ptr(), 1) };
    let args = [logger_val, make_str_val("low disk space")];
    let result = unsafe { mb_log_warning(args.as_ptr(), 2) };
    assert!(result.is_none(), "mb_log_warning should return none()");
}

#[test]
fn log_warning_null_logger() {
    let args = [MbValue::none(), make_str_val("warning msg")];
    let result = unsafe { mb_log_warning(args.as_ptr(), 2) };
    assert!(
        result.is_none(),
        "mb_log_warning with null logger should return none()"
    );
}

// ── All log levels on same logger (sequence test) ─────────────────────────────

#[test]
fn log_all_levels_sequence() {
    let logger_val = unsafe { mb_log_get_logger([make_str_val("multi")].as_ptr(), 1) };
    let msg = make_str_val("sequence test");

    let r1 = unsafe { mb_log_info([logger_val, msg].as_ptr(), 2) };
    let r2 = unsafe { mb_log_error([logger_val, msg].as_ptr(), 2) };
    let r3 = unsafe { mb_log_debug([logger_val, msg].as_ptr(), 2) };
    let r4 = unsafe { mb_log_warning([logger_val, msg].as_ptr(), 2) };

    assert!(r1.is_none(), "info in sequence should return none()");
    assert!(r2.is_none(), "error in sequence should return none()");
    assert!(r3.is_none(), "debug in sequence should return none()");
    assert!(r4.is_none(), "warning in sequence should return none()");
}
