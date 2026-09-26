//! Test-only `ObjectOps` implementation. Public so binding crates can
//! install it from their own unit tests (which don't link mamba's real
//! runtime) without reimplementing the stub. Call [`init`] at the top
//! of any test that exercises `ops()`-dependent code paths — it's
//! idempotent and safe to call from every test thanks to `Once`.
//!
//! Real production use is served by `mamba::runtime::registry_bridge`,
//! which installs before any binding code fires. `init` and
//! `set_object_ops` share the same `OnceLock`, so the first caller
//! wins — test harnesses installing the stub before mamba boot is not
//! a supported configuration.
//!
//! The stub intentionally leaks: `list_new` / `dict_new` / `str_new`
//! return a `MbValue::from_ptr` over a `Box::into_raw(Box::new(TestObj::_))`
//! that is never reclaimed. Acceptable for tests; not a production
//! shape.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, Once};

use crate::{MbValue, ObjectOps};

/// All state a list / dict / str MbValue points at under the test ops.
/// Mutable interior via `Mutex` so `dict_insert_str` can mutate through
/// a shared (`&`) pointer, matching the real ops signature.
enum TestObj {
    List(Mutex<Vec<MbValue>>),
    Dict(Mutex<HashMap<String, MbValue>>),
    Str(String),
}

static INIT: Once = Once::new();

/// Idempotently install the test ops table. Called by every test that
/// exercises `Vec<T>` / `HashMap<String, V>` conversion or `ops()`.
pub fn init() {
    INIT.call_once(|| {
        crate::ops::set_object_ops(&TEST_OPS);
    });
}

static TEST_OPS: ObjectOps = ObjectOps {
    dict_new: test_dict_new,
    dict_get_str: test_dict_get_str,
    dict_insert_str: test_dict_insert_str,
    dict_iter_str_items: test_dict_iter,
    list_new: test_list_new,
    list_len: test_list_len,
    list_get: test_list_get,
    str_new: test_str_new,
    str_read: test_str_read,
    raise: test_raise,
    raise_instance: test_raise_instance,
    register_getter: test_register_getter,
    call0: test_call0,
};

// ── helpers ─────────────────────────────────────────────────────────────

unsafe fn deref(v: MbValue) -> Option<&'static TestObj> {
    let addr = v.as_ptr()?;
    if addr == 0 {
        return None;
    }
    Some(&*(addr as *const TestObj))
}

// ── list ────────────────────────────────────────────────────────────────

fn test_list_new(items: Vec<MbValue>) -> MbValue {
    let raw = Box::into_raw(Box::new(TestObj::List(Mutex::new(items))));
    MbValue::from_ptr(raw as usize)
}

fn test_list_len(v: MbValue) -> Option<usize> {
    unsafe {
        match deref(v)? {
            TestObj::List(m) => Some(m.lock().unwrap().len()),
            _ => None,
        }
    }
}

fn test_list_get(v: MbValue, i: usize) -> Option<MbValue> {
    unsafe {
        match deref(v)? {
            TestObj::List(m) => m.lock().unwrap().get(i).copied(),
            _ => None,
        }
    }
}

// ── dict ────────────────────────────────────────────────────────────────

fn test_dict_new() -> MbValue {
    let raw = Box::into_raw(Box::new(TestObj::Dict(Mutex::new(HashMap::new()))));
    MbValue::from_ptr(raw as usize)
}

fn test_dict_get_str(v: MbValue, key: &str) -> Option<MbValue> {
    unsafe {
        match deref(v)? {
            TestObj::Dict(m) => m.lock().unwrap().get(key).copied(),
            _ => None,
        }
    }
}

fn test_dict_insert_str(v: MbValue, key: &str, value: MbValue) {
    unsafe {
        if let Some(TestObj::Dict(m)) = deref(v) {
            m.lock().unwrap().insert(key.to_string(), value);
        }
    }
}

// ── str ─────────────────────────────────────────────────────────────────

fn test_str_new(s: &str) -> MbValue {
    let raw = Box::into_raw(Box::new(TestObj::Str(s.to_string())));
    MbValue::from_ptr(raw as usize)
}

fn test_str_read(v: MbValue) -> Option<String> {
    unsafe {
        match deref(v)? {
            TestObj::Str(s) => Some(s.clone()),
            _ => None,
        }
    }
}

fn test_dict_iter(v: MbValue) -> Option<Vec<(String, MbValue)>> {
    unsafe {
        match deref(v)? {
            TestObj::Dict(m) => {
                let g = m.lock().unwrap();
                Some(g.iter().map(|(k, &v)| (k.clone(), v)).collect())
            }
            _ => None,
        }
    }
}

// ── exception recorders ─────────────────────────────────────────────────
//
// `exc` module tests verify that `raise_value_error` & friends route
// through `ops().raise{_instance}`. The stubs record calls into
// thread-locals so the tests can assert on what was observed without
// racing across threads.

thread_local! {
    static RAISES: std::cell::RefCell<Vec<(String, String)>>
        = const { std::cell::RefCell::new(Vec::new()) };
    static RAISED_INSTANCE: std::cell::Cell<Option<MbValue>>
        = const { std::cell::Cell::new(None) };
}

static TEST_CALL0: Mutex<Option<HashSet<usize>>> = Mutex::new(None);

fn test_raise(exc_type: &str, msg: &str) {
    RAISES.with(|r| r.borrow_mut().push((exc_type.to_string(), msg.to_string())));
}

fn test_raise_instance(exc: MbValue) {
    RAISED_INSTANCE.with(|r| r.set(Some(exc)));
}

/// Drain the recorded `raise` calls and return them in call order. Thread-local.
pub fn take_raises() -> Vec<(String, String)> {
    RAISES.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

/// Reset the recorder without consuming it — convenience for tests that
/// only care about post-`clear` calls.
pub fn clear_raises() {
    RAISES.with(|r| r.borrow_mut().clear());
    RAISED_INSTANCE.with(|r| r.set(None));
}

/// Fetch the last `raise_instance` argument (if any) and reset the slot.
pub fn take_raised_instance() -> Option<MbValue> {
    RAISED_INSTANCE.with(|r| r.take())
}

fn test_register_getter(
    _type_name: &str,
    _attr: &str,
    _getter: unsafe extern "C" fn(*const MbValue, usize) -> MbValue,
) {
}

fn test_call0(callable: MbValue) -> Option<MbValue> {
    let addr = callable.as_func()?;
    let guard = TEST_CALL0.lock().unwrap();
    if !guard
        .as_ref()
        .is_some_and(|allowed| allowed.contains(&addr))
    {
        return None;
    }
    let func: unsafe extern "C" fn(*const MbValue, usize) -> MbValue =
        unsafe { std::mem::transmute(addr) };
    Some(unsafe { func(std::ptr::null(), 0) })
}

/// Register a native test callback for `ops().call0` and return its MbValue.
pub fn register_call0(callback: unsafe extern "C" fn(*const MbValue, usize) -> MbValue) -> MbValue {
    init();
    let addr = callback as usize;
    let mut guard = TEST_CALL0.lock().unwrap();
    guard.get_or_insert_with(HashSet::new).insert(addr);
    MbValue::from_func(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn hello(_args: *const MbValue, _nargs: usize) -> MbValue {
        test_str_new("hello")
    }

    #[test]
    fn object_ops_call0_invokes_test_callback() {
        init();
        let handler = register_call0(hello);
        let result = (crate::ops().call0)(handler).expect("registered callback");
        assert_eq!(test_str_read(result).as_deref(), Some("hello"));
        assert!((crate::ops().call0)(MbValue::from_func(0x7777)).is_none());
    }
}
