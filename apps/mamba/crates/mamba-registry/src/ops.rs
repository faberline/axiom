//! `ObjectOps` — callback table installed by the mamba runtime at boot.
//!
//! Binding crates call through [`ops()`] to allocate / inspect real mamba
//! objects and to raise exceptions, without depending on mamba internals
//! or mirroring its `ObjData` enum layout. That mirror has drifted from
//! mamba's real layout in the past; routing through function pointers
//! prevents a whole class of UB.
//!
//! # Lifecycle
//!
//! 1. The mamba binary (or test harness) calls [`set_object_ops`] exactly
//!    once at startup, passing a `&'static ObjectOps` whose function
//!    pointers hit real mamba runtime helpers.
//! 2. Binding crates call [`ops()`] at any time after that to get a
//!    reference to the installed table.
//! 3. Calling [`ops()`] before [`set_object_ops`] panics with a clear
//!    message — this is a programmer error, not a runtime condition.
//!
//! # Stability
//!
//! [`ObjectOps`] is `#[non_exhaustive]`; new fields may be appended in
//! future PRs without requiring binding crates to change construction
//! sites (there's only one — mamba's `registry_bridge`). Removing or
//! changing the signature of an existing field is a breaking change.

use super::MbValue;
use std::sync::OnceLock;

/// Callbacks supplied by the mamba runtime.
///
/// # Stability policy
///
/// Only mamba constructs this (via `runtime::registry_bridge`); binding
/// crates only read through [`ops()`]. Therefore:
///
/// - **Adding a field is non-breaking** for binding crates (they never
///   construct `ObjectOps`); it only requires a matching update in
///   mamba's `registry_bridge::REAL_OPS`.
/// - **Removing a field or changing a signature is breaking** for
///   binding crates — any caller of the removed/changed method fails to
///   compile. Such changes must bump `mamba-registry`'s minor
///   version so downstream binding crates notice via their `Cargo.toml`.
///
/// The struct is deliberately *not* `#[non_exhaustive]` — that attribute
/// prevents mamba itself (an external crate) from constructing the
/// literal `ObjectOps { ... }`, which is needed for the `static REAL_OPS`
/// pattern. The stability guarantee above is maintained by discipline +
/// code review, not by the type system.
pub struct ObjectOps {
    // ── dict (str keys for MVP) ─────────────────────────────────────────
    /// Allocate a new empty mamba `dict` object.
    pub dict_new: fn() -> MbValue,

    /// Read a value by string key. `None` if key missing or `dict` is
    /// not actually a dict.
    pub dict_get_str: fn(MbValue, &str) -> Option<MbValue>,

    /// Insert `(key, value)`. Retains `value` refcount per mamba semantics.
    /// Panics if `dict` is not a dict.
    pub dict_insert_str: fn(MbValue, &str, MbValue),

    /// Iterate `(String, MbValue)` pairs, skipping non-string keys.
    /// Returns `None` if the value is not a dict. The returned
    /// `Vec` is a snapshot — holds no lock after return.
    pub dict_iter_str_items: fn(MbValue) -> Option<Vec<(String, MbValue)>>,

    // ── list ────────────────────────────────────────────────────────────
    /// Allocate a new mamba `list` from the given elements.
    pub list_new: fn(Vec<MbValue>) -> MbValue,

    /// Length of a mamba `list`. `None` if not a list.
    pub list_len: fn(MbValue) -> Option<usize>,

    /// Indexed access. `None` on out-of-bounds or non-list.
    pub list_get: fn(MbValue, usize) -> Option<MbValue>,

    // ── str ─────────────────────────────────────────────────────────────
    /// Allocate a new mamba `str` object containing `s`. Returns a PTR-tagged
    /// `MbValue` with refcount 1.
    pub str_new: fn(s: &str) -> MbValue,

    /// Read the string content from a `str`-shaped `MbValue`. Returns
    /// `None` if the value is not a pointer to a mamba `str`. Allocates a
    /// new `String` — callers that need the borrowed slice must own the
    /// returned `String` and borrow from it locally.
    ///
    /// Routing through the ops table (instead of a layout-mirror read)
    /// decouples binding crates from mamba's internal `ObjData` enum,
    /// which has gained variants since the mirror was last synced.
    pub str_read: fn(v: MbValue) -> Option<String>,

    // ── exception ───────────────────────────────────────────────────────
    /// Set the thread-local current exception with the given type name
    /// and message. Mamba's eval loop picks this up after the FFI shim
    /// returns.
    pub raise: fn(exc_type: &str, msg: &str),

    /// Raise a pre-constructed exception instance (e.g. one produced by
    /// `mb_wrap_native` holding a user-defined exception type). Preserves
    /// all fields on the instance for handler inspection.
    pub raise_instance: fn(exc: MbValue),

    // ── attribute access (consumed by PR-5) ─────────────────────────────
    /// Register a getter for `{type_name}.{attr}` on a `mb_wrap_native`-
    /// wrapped value. PR-1 stores the registration; PR-5 wires up the
    /// `getattr` fallback that dispatches to it.
    pub register_getter: fn(
        type_name: &str,
        attr: &str,
        getter: unsafe extern "C" fn(*const MbValue, usize) -> MbValue,
    ),

    // ── callable dispatch ───────────────────────────────────────────────
    /// Invoke a zero-argument callable value. Returns `None` when the value
    /// is not callable by the installed runtime.
    ///
    /// Binding crates use this for callback-style APIs, such as HTTP route
    /// handlers, without depending on mamba's internal function/closure layout.
    pub call0: fn(MbValue) -> Option<MbValue>,
}

/// The globally installed ops table. `None` until [`set_object_ops`] is
/// called by the mamba runtime at boot.
pub static OBJECT_OPS: OnceLock<&'static ObjectOps> = OnceLock::new();

/// Install the global `ObjectOps` table. Called once by mamba at startup.
///
/// Subsequent calls are silently ignored — the first installation wins.
/// This makes the function idempotent across test harnesses that may
/// initialize multiple times.
pub fn set_object_ops(ops: &'static ObjectOps) {
    let _ = OBJECT_OPS.set(ops);
}

/// Access the installed ops table.
///
/// # Panics
///
/// Panics if [`set_object_ops`] has not been called. This indicates a
/// programmer error: a binding crate's code ran before the mamba runtime
/// finished bootstrapping. In normal use (via the mamba binary) this
/// cannot happen because `main` installs ops before any Python code is
/// compiled or executed.
pub fn ops() -> &'static ObjectOps {
    OBJECT_OPS.get().copied().expect(
        "mamba_registry::ObjectOps not initialized — \
         the mamba runtime must call set_object_ops() at startup \
         before any binding code runs.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_ops;

    /// Verify the ops table can be installed, is idempotent, and that
    /// `ops()` returns the first-installed table even when a second
    /// table is passed to `set_object_ops` later.
    ///
    /// All tests in this crate share the same process and therefore the
    /// same `OBJECT_OPS` `OnceLock`. They all install via
    /// `test_ops::init()` so later calls here observe the installed
    /// test table, not this test's local stubs.
    #[test]
    fn test_ops_install_and_idempotent() {
        // Install the crate-wide test ops (no-op if a sibling test got here first).
        test_ops::init();

        // `ops()` must not panic once installed.
        let first = ops();
        // `list_new` on an empty vec should give back a PTR-shaped MbValue
        // — proves the fn pointer goes somewhere real, not a NaN-poisoned slot.
        let empty_list = (first.list_new)(vec![]);
        assert!(empty_list.is_ptr(), "list_new should return a PTR MbValue");

        // OnceLock semantics: a second install with a different table is
        // silently ignored. Construct a table whose dict_new returns a
        // sentinel int and verify `ops().dict_new()` still behaves like the
        // real (already-installed) table, NOT the new one.
        fn sentinel_dict_new() -> MbValue {
            MbValue::from_int(777)
        }
        fn stub_dict_get(_: MbValue, _: &str) -> Option<MbValue> {
            None
        }
        fn stub_dict_insert(_: MbValue, _: &str, _: MbValue) {}
        fn stub_dict_iter(_: MbValue) -> Option<Vec<(String, MbValue)>> {
            None
        }
        fn stub_list_new(_: Vec<MbValue>) -> MbValue {
            MbValue::none()
        }
        fn stub_list_len(_: MbValue) -> Option<usize> {
            None
        }
        fn stub_list_get(_: MbValue, _: usize) -> Option<MbValue> {
            None
        }
        fn stub_str_new(_: &str) -> MbValue {
            MbValue::none()
        }
        fn stub_str_read(_: MbValue) -> Option<String> {
            None
        }
        fn stub_raise(_: &str, _: &str) {}
        fn stub_raise_instance(_: MbValue) {}
        fn stub_call0(_: MbValue) -> Option<MbValue> {
            None
        }
        fn stub_register_getter(
            _: &str,
            _: &str,
            _: unsafe extern "C" fn(*const MbValue, usize) -> MbValue,
        ) {
        }
        static OTHER_OPS: ObjectOps = ObjectOps {
            dict_new: sentinel_dict_new,
            dict_get_str: stub_dict_get,
            dict_insert_str: stub_dict_insert,
            dict_iter_str_items: stub_dict_iter,
            list_new: stub_list_new,
            list_len: stub_list_len,
            list_get: stub_list_get,
            str_new: stub_str_new,
            str_read: stub_str_read,
            raise: stub_raise,
            raise_instance: stub_raise_instance,
            register_getter: stub_register_getter,
            call0: stub_call0,
        };
        set_object_ops(&OTHER_OPS);
        assert_ne!(
            (ops().dict_new)().as_int(),
            Some(777),
            "OnceLock must ignore subsequent set_object_ops calls",
        );
    }

    /// `str_new` / `str_read` round-trip through the test ops, with a
    /// `None` on a non-str value. Pins the PR-6 contract that replaced
    /// the `ObjData` layout mirror.
    #[test]
    fn test_str_new_read_roundtrip() {
        test_ops::init();
        let s = (ops().str_new)("hello");
        assert!(s.is_ptr(), "str_new should return a PTR MbValue");
        assert_eq!((ops().str_read)(s), Some("hello".to_string()));

        // Non-str values return None.
        assert_eq!((ops().str_read)(MbValue::from_int(7)), None);
        assert_eq!((ops().str_read)(MbValue::none()), None);
    }
}
