//! Exception-raising helpers for binding crates.
//!
//! Thin convenience wrappers over [`crate::ops()`] — generator output calls
//! into these instead of constructing the `(exc_type, msg)` string pair and
//! invoking `ops().raise` by hand at every raise site.
//!
//! # Return-value contract
//!
//! Every helper returns [`MbValue::none()`]. Mamba's eval loop reads the
//! thread-local exception state set by `ops().raise{_instance}` **before**
//! looking at the return value — the `MbValue::none()` is purely a
//! sentinel so generator-emitted Rust code can write:
//!
//! ```ignore
//! if cond { return raise_value_error(&format!("bad input: {x}")); }
//! ```
//!
//! without a phantom `unreachable!()` or `unsafe { std::mem::zeroed() }`.
//!
//! # Exception class names
//!
//! The four helpers below hard-code the CPython builtin names that the
//! mamba runtime already recognizes. For custom classes use
//! [`raise_instance`] with a pre-constructed `mb_wrap_native`-wrapped value.

use crate::MbValue;

/// Raise a `ValueError` with the given message.
///
/// Matches CPython: used when a function receives an argument of the right
/// type but an inappropriate value (e.g. out-of-range HTTP status codes).
pub fn raise_value_error(msg: &str) -> MbValue {
    (crate::ops().raise)("ValueError", msg);
    MbValue::none()
}

/// Raise a `TypeError` with the given message.
///
/// Matches CPython: used when an argument has the wrong type (e.g. passing
/// a string where an int is expected).
pub fn raise_type_error(msg: &str) -> MbValue {
    (crate::ops().raise)("TypeError", msg);
    MbValue::none()
}

/// Raise a `RuntimeError` with the given message.
///
/// Matches CPython: used as a generic catch-all when no more specific
/// exception class fits. Generators emit this from unrecoverable failure
/// paths that don't map to `ValueError` / `TypeError` / `KeyError`.
pub fn raise_runtime_error(msg: &str) -> MbValue {
    (crate::ops().raise)("RuntimeError", msg);
    MbValue::none()
}

/// Raise a `KeyError` with the given message.
///
/// Matches CPython: used when a mapping lookup fails. Typical caller is a
/// generator emitting a `dict`-backed attribute access that missed.
pub fn raise_key_error(msg: &str) -> MbValue {
    (crate::ops().raise)("KeyError", msg);
    MbValue::none()
}

/// Raise a pre-constructed exception instance.
///
/// `exc` is typically a [`crate::convert::mb_wrap_native`] result holding a
/// user-defined exception type (e.g. `HTTPException`). Mamba preserves
/// every field on the instance so the handler can inspect
/// `exc.status_code`, `exc.detail`, `exc.headers`, etc.
///
/// This is the mechanism Q2(a) of the SDD-codegen ↔ mamba discussion
/// identified as the "higher priority" path: generated raise sites build a
/// fully-typed wrapper and hand it to the runtime, rather than encoding
/// fields into a string message.
pub fn raise_instance(exc: MbValue) -> MbValue {
    (crate::ops().raise_instance)(exc);
    MbValue::none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_ops;

    #[test]
    fn raise_value_error_routes_to_ops() {
        test_ops::init();
        test_ops::clear_raises();
        let ret = raise_value_error("bad input: -1");
        assert!(
            ret.is_none(),
            "raise_* must return MbValue::none() sentinel"
        );
        let raises = test_ops::take_raises();
        assert_eq!(
            raises,
            [("ValueError".to_string(), "bad input: -1".to_string())]
        );
    }

    #[test]
    fn four_builtin_raisers_produce_distinct_class_names() {
        test_ops::init();
        test_ops::clear_raises();
        raise_value_error("v");
        raise_type_error("t");
        raise_runtime_error("r");
        raise_key_error("k");
        let raises = test_ops::take_raises();
        let names: Vec<&str> = raises.iter().map(|(c, _)| c.as_str()).collect();
        assert_eq!(
            names,
            ["ValueError", "TypeError", "RuntimeError", "KeyError"]
        );
    }

    #[test]
    fn raise_instance_forwards_exact_mbvalue() {
        test_ops::init();
        test_ops::clear_raises();
        // Pretend this is a mb_wrap_native result — we just need a distinctive
        // MbValue. Int slot is fine; the ops stub records it verbatim.
        let fake_exc = MbValue::from_int(0xC0FFEE);
        let ret = raise_instance(fake_exc);
        assert!(ret.is_none());
        assert_eq!(test_ops::take_raised_instance(), Some(fake_exc));
    }
}
