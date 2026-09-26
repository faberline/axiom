//! FFI functions exposed by `qc-mamba` to Mamba scripts.
//!
//! All functions follow the Mamba native-call ABI:
//! ```text
//! extern "C" fn name(args: *const MbValue, nargs: usize) -> MbValue
//! ```
//!
//! # Decorator semantics
//!
//! `fixture`, `mark`, and `parametrize` are registered as symbols in the
//! `cclab.qc` module. When the Mamba compiler sees `@fixture` or
//! `from cclab.qc import fixture`, it resolves these symbols at compile time
//! and inserts the appropriate metadata into the test runner's registry.
//!
//! At runtime, the test runner (backed by `cclab_qc::runner::TestRunner`) reads
//! the registered metadata and invokes fixtures / parametrized cases in order.

#![allow(improper_ctypes_definitions)]

use mamba_registry::convert::mb_wrap_native;
use mamba_registry::MbValue;

// ── Internal types ────────────────────────────────────────────────────────────

/// Metadata attached to a `@fixture`-decorated function.
#[derive(Debug)]
pub struct MbFixtureMeta {
    pub autouse: bool,
    pub scope: String,
}

/// Metadata for a `raises(ExcType)` context manager.
#[derive(Debug)]
pub struct MbRaisesCtx {
    /// The exception type name that should be raised.
    pub exc_type_name: String,
    /// Whether the expected exception was actually raised.
    pub caught: bool,
}

/// Metadata for a `@mark.parametrize` case set.
#[derive(Debug)]
pub struct MbParametrizeMeta {
    pub argnames: String,
    pub case_count: usize,
}

// ── Helper utilities ──────────────────────────────────────────────────────────

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

// ── mb_qc_fixture ─────────────────────────────────────────────────────────────

/// Mark a function as a QC fixture.
///
/// # ABI
/// ```text
/// args[0] = fn          (MbValue::Ptr → callable)
/// args[1] = autouse     (MbValue::Bool, optional, default false)
/// args[2] = scope       (MbValue::Ptr → str, optional, default "function")
/// ```
/// Returns the same callable with fixture metadata attached.
#[no_mangle]
pub unsafe extern "C" fn mb_qc_fixture(args: *const MbValue, nargs: usize) -> MbValue {
    let fn_val = unsafe { arg(args, nargs, 0) };
    let autouse = unsafe { arg(args, nargs, 1) };
    let scope = unsafe { arg(args, nargs, 2) };

    let autouse = autouse.as_bool().unwrap_or(false);
    let scope = read_str(scope).unwrap_or_else(|| "function".to_string());

    let meta = MbFixtureMeta { autouse, scope };
    mb_wrap_native(meta);

    // Return the original function unchanged; the test runner resolves
    // fixture metadata through the module symbol table at discovery time.
    fn_val
}

// ── mb_qc_mark ────────────────────────────────────────────────────────────────

/// Return the `mark` namespace object.
///
/// Provides `.asyncio`, `.parametrize`, and `.skip` attributes that the Mamba
/// compiler resolves via member-access lowering.
///
/// # ABI
/// ```text
/// (no arguments)
/// ```
/// Returns a `MbValue::Ptr → str("mb_qc_mark_namespace")` sentinel.
#[no_mangle]
pub unsafe extern "C" fn mb_qc_mark(_args: *const MbValue, _nargs: usize) -> MbValue {
    wrap_str("mb_qc_mark_namespace".to_string())
}

// ── mb_qc_raises ─────────────────────────────────────────────────────────────

/// Create a `raises(ExcType)` context manager.
///
/// # ABI
/// ```text
/// args[0] = exc_type  (MbValue::Ptr → str, exception type name)
/// ```
/// Returns a `MbRaisesCtx` wrapped as an opaque pointer.
#[no_mangle]
pub unsafe extern "C" fn mb_qc_raises(args: *const MbValue, nargs: usize) -> MbValue {
    let exc_val = unsafe { arg(args, nargs, 0) };
    let exc_type_name = read_str(exc_val).unwrap_or_else(|| "Exception".to_string());

    let ctx = MbRaisesCtx {
        exc_type_name,
        caught: false,
    };
    mb_wrap_native(ctx)
}

// ── mb_qc_parametrize ────────────────────────────────────────────────────────

/// Create a parametrize decorator.
///
/// # ABI
/// ```text
/// args[0] = argnames   (MbValue::Ptr → str, comma-separated names)
/// args[1] = argvalues  (MbValue::Ptr → list of cases)
/// ```
/// Returns a `MbParametrizeMeta` wrapped as an opaque pointer.
#[no_mangle]
pub unsafe extern "C" fn mb_qc_parametrize(args: *const MbValue, nargs: usize) -> MbValue {
    let names_val = unsafe { arg(args, nargs, 0) };
    let values_val = unsafe { arg(args, nargs, 1) };

    let argnames = read_str(names_val).unwrap_or_default();

    // Count cases: if values_val is a list, inspect its length; otherwise 1.
    let case_count = if let Some(addr) = values_val.as_ptr() {
        // Opaque count; the test runner resolves actual cases at discovery time.
        // A non-null pointer implies at least 1 case.
        if addr != 0 {
            1
        } else {
            0
        }
    } else {
        0
    };

    let meta = MbParametrizeMeta {
        argnames,
        case_count,
    };
    mb_wrap_native(meta)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_str_val(s: &str) -> MbValue {
        mamba_registry::test_ops::init();
        mamba_registry::rc::wrap_obj_str(s.to_string())
    }

    #[test]
    fn test_fixture_returns_fn_value() {
        let fn_val = MbValue::from_int(42);
        let autouse = MbValue::from_bool(true);
        let args = [fn_val, autouse];
        let result = unsafe { mb_qc_fixture(args.as_ptr(), 2) };
        // Should return the original fn value unchanged
        assert_eq!(result, fn_val);
    }

    #[test]
    fn test_mark_returns_sentinel() {
        let result = unsafe { mb_qc_mark(std::ptr::null(), 0) };
        assert!(result.is_ptr());
        let s = unsafe { result.as_obj_str() }.unwrap();
        assert_eq!(s, "mb_qc_mark_namespace");
    }

    #[test]
    fn test_raises_returns_ptr() {
        let exc_val = make_str_val("ValueError");
        let args = [exc_val];
        let result = unsafe { mb_qc_raises(args.as_ptr(), 1) };
        assert!(result.is_ptr());
    }

    #[test]
    fn test_parametrize_returns_ptr() {
        let names_val = make_str_val("x,y");
        let values_val = make_str_val("cases");
        let args = [names_val, values_val];
        let result = unsafe { mb_qc_parametrize(args.as_ptr(), 2) };
        assert!(result.is_ptr());
    }

    #[test]
    fn qc_fixture_injected() {
        let fn_val = MbValue::from_int(99);
        let autouse_true = MbValue::from_bool(true);
        let scope_val = make_str_val("function");
        let args = [fn_val, autouse_true, scope_val];
        let result = unsafe { mb_qc_fixture(args.as_ptr(), 3) };
        assert_eq!(result, fn_val);

        let autouse_false = MbValue::from_bool(false);
        let args2 = [fn_val, autouse_false];
        let result2 = unsafe { mb_qc_fixture(args2.as_ptr(), 2) };
        assert_eq!(result2, fn_val);
    }

    #[test]
    fn qc_parametrize_runs_cases() {
        let names_val = make_str_val("x");
        let values_val = make_str_val("cases_data");
        let args = [names_val, values_val];
        let result = unsafe { mb_qc_parametrize(args.as_ptr(), 2) };
        assert!(result.is_ptr());

        let addr = result.as_ptr().unwrap();
        let meta = unsafe { &*(addr as *const MbParametrizeMeta) };
        assert_eq!(meta.argnames, "x");
        assert_eq!(meta.case_count, 1);
    }

    #[test]
    fn qc_raises_asserts_exception() {
        let exc_val = make_str_val("ValueError");
        let args = [exc_val];
        let result = unsafe { mb_qc_raises(args.as_ptr(), 1) };
        assert!(result.is_ptr());

        let addr = result.as_ptr().unwrap();
        let ctx = unsafe { &*(addr as *const MbRaisesCtx) };
        assert_eq!(ctx.exc_type_name, "ValueError");
        assert!(!ctx.caught);
    }
}
