//! Bridge between mamba's internal runtime and `mamba-registry`'s
//! `ObjectOps` callback table.
//!
//! # Lifecycle
//!
//! [`install`] is called once from `main.rs` at binary startup, before
//! any Python source is compiled or any FFI symbol fires. It populates
//! `mamba_registry::OBJECT_OPS` with function pointers that route
//! to real mamba runtime helpers.
//!
//! # MbValue transmute
//!
//! `mamba_registry::MbValue` is `#[repr(transparent)]` over `u64`;
//! mamba's internal `crate::runtime::value::MbValue` is also a `u64`
//! wrapper. They are bit-compatible. We go through `to_bits` /
//! `from_bits` on both sides rather than unsafe transmute to keep the
//! conversion explicit and grep-able.

use mamba_registry as registry;
use registry::ObjectOps;

use super::dict_ops::DictKey;
use super::rc::{MbObject, ObjData};
use super::value::MbValue as InternalMb;

// ── Bit-level conversion helpers ────────────────────────────────────────

#[inline]
fn to_internal(v: registry::MbValue) -> InternalMb {
    InternalMb::from_bits(v.to_bits())
}

#[inline]
fn to_registry(v: InternalMb) -> registry::MbValue {
    registry::MbValue::from_bits(v.to_bits())
}

// ── Dict ops ────────────────────────────────────────────────────────────

fn dict_new() -> registry::MbValue {
    to_registry(super::dict_ops::mb_dict_new())
}

fn dict_get_str(dict: registry::MbValue, key: &str) -> Option<registry::MbValue> {
    let d = to_internal(dict);
    let ptr = d.as_ptr()?;
    unsafe {
        if let ObjData::Dict(ref lock) = (*ptr).data {
            let guard = lock.read().unwrap();
            // DictKey::Str equivalent lookup; avoid allocating a DictKey.
            let dk = DictKey::Str(key.to_string());
            let val = guard.get(&dk).copied()?;
            super::rc::retain_if_ptr(val);
            return Some(to_registry(val));
        }
    }
    None
}

fn dict_insert_str(dict: registry::MbValue, key: &str, value: registry::MbValue) {
    let d = to_internal(dict);
    let v = to_internal(value);
    let k_mb = InternalMb::from_ptr(MbObject::new_str(key.to_string()));
    super::dict_ops::mb_dict_setitem(d, k_mb, v);
}

fn dict_iter_str_items(dict: registry::MbValue) -> Option<Vec<(String, registry::MbValue)>> {
    let d = to_internal(dict);
    let ptr = d.as_ptr()?;
    unsafe {
        if let ObjData::Dict(ref lock) = (*ptr).data {
            let guard = lock.read().unwrap();
            let mut out = Vec::with_capacity(guard.len());
            for (k, &v) in guard.iter() {
                if let DictKey::Str(ref s) = k {
                    super::rc::retain_if_ptr(v);
                    out.push((s.clone(), to_registry(v)));
                }
                // non-string keys silently skipped — matches trait contract
            }
            return Some(out);
        }
    }
    None
}

// ── List ops ────────────────────────────────────────────────────────────

fn list_new(elements: Vec<registry::MbValue>) -> registry::MbValue {
    let internal: Vec<InternalMb> = elements.into_iter().map(to_internal).collect();
    to_registry(super::list_ops::mb_list_from(internal))
}

fn list_len(list: registry::MbValue) -> Option<usize> {
    let l = to_internal(list);
    let ptr = l.as_ptr()?;
    unsafe {
        if let ObjData::List(ref lock) = (*ptr).data {
            return Some(lock.read().unwrap().len());
        }
    }
    None
}

fn list_get(list: registry::MbValue, idx: usize) -> Option<registry::MbValue> {
    let l = to_internal(list);
    let ptr = l.as_ptr()?;
    unsafe {
        if let ObjData::List(ref lock) = (*ptr).data {
            let guard = lock.read().unwrap();
            let v = guard.get(idx)?;
            super::rc::retain_if_ptr(v);
            return Some(to_registry(v));
        }
    }
    None
}

// ── Str ops ─────────────────────────────────────────────────────────────

fn str_new(s: &str) -> registry::MbValue {
    let ptr = MbObject::new_str(s.to_string());
    to_registry(InternalMb::from_ptr(ptr))
}

fn str_read(v: registry::MbValue) -> Option<String> {
    let internal = to_internal(v);
    let ptr = internal.as_ptr()?;
    unsafe {
        if let ObjData::Str(ref s) = (*ptr).data {
            return Some(s.clone());
        }
    }
    None
}

// ── Exception ops ───────────────────────────────────────────────────────

fn raise(exc_type: &str, msg: &str) {
    let t = InternalMb::from_ptr(MbObject::new_str(exc_type.to_string()));
    let m = InternalMb::from_ptr(MbObject::new_str(msg.to_string()));
    super::exception::mb_raise(t, m);
}

fn raise_instance(exc: registry::MbValue) {
    super::class::mb_raise_instance(to_internal(exc));
}

// ── Attribute getter registry (consumed by PR-5) ────────────────────────

use std::collections::HashMap;
use std::sync::RwLock;

type GetterFn = unsafe extern "C" fn(*const registry::MbValue, usize) -> registry::MbValue;

/// Registered attribute getters, keyed by `(type_name, attr)`. PR-5 will
/// read from this when `mb_obj_getattr` misses mamba's own field lookups
/// on a `mb_wrap_native`-produced pointer.
pub static ATTRIBUTE_GETTERS: RwLock<Option<HashMap<(String, String), GetterFn>>> =
    RwLock::new(None);

fn register_getter(type_name: &str, attr: &str, getter: GetterFn) {
    let mut guard = ATTRIBUTE_GETTERS.write().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert((type_name.to_string(), attr.to_string()), getter);
}

fn call0(callable: registry::MbValue) -> Option<registry::MbValue> {
    let value = to_internal(callable);
    let looks_callable =
        value.as_func().is_some() || value.as_int().is_some() || value.as_ptr().is_some();
    if !looks_callable {
        return None;
    }
    Some(to_registry(super::class::mb_call0(value)))
}

/// Look up a previously registered getter. Used by PR-5's getattr path.
#[allow(dead_code)]
pub fn lookup_getter(type_name: &str, attr: &str) -> Option<GetterFn> {
    let guard = ATTRIBUTE_GETTERS.read().unwrap();
    guard
        .as_ref()?
        .get(&(type_name.to_string(), attr.to_string()))
        .copied()
}

// ── Installation ────────────────────────────────────────────────────────

static REAL_OPS: ObjectOps = ObjectOps {
    dict_new,
    dict_get_str,
    dict_insert_str,
    dict_iter_str_items,
    list_new,
    list_len,
    list_get,
    str_new,
    str_read,
    raise,
    raise_instance,
    register_getter,
    call0,
};

/// Install the real `ObjectOps` table. Called once from `main.rs` before
/// any runtime work. Idempotent — subsequent calls are no-ops.
pub fn install() {
    registry::set_object_ops(&REAL_OPS);
}
