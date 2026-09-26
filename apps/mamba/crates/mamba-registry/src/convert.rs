//! `MbValue` ↔ Rust type conversion traits and implementations.
//!
//! # Traits
//!
//! - [`FromMbValue`]: Convert a [`MbValue`] into a Rust type (may fail).
//! - [`IntoMbValue`]: Convert a Rust type into a [`MbValue`] (infallible).
//!
//! # Opaque handles
//!
//! Use [`mb_wrap_native`] / [`mb_unwrap_native`] for arbitrary Rust structs
//! that cannot be represented as a plain [`MbValue`] scalar.  The value is
//! heap-allocated and the pointer is stored in the NaN-boxed PTR slot.

use std::any::Any;
use std::collections::HashMap;
use std::sync::RwLock;
use thiserror::Error;

use super::MbValue;

// ── Error type ────────────────────────────────────────────────────────────────

/// Error produced by a [`FromMbValue`] conversion.
#[derive(Debug, Clone, Error)]
pub enum MbConvError {
    #[error("expected {expected}, got {actual}")]
    TypeMismatch {
        expected: &'static str,
        actual: &'static str,
    },

    #[error("integer {value} overflows target type")]
    IntOverflow { value: i64 },

    #[error("invalid UTF-8 in string value")]
    Utf8Error,

    #[error("null pointer in native handle")]
    NullPointer,
}

// ── Traits ────────────────────────────────────────────────────────────────────

/// Convert a [`MbValue`] into a Rust type.
pub trait FromMbValue: Sized {
    fn from_mb_value(v: MbValue) -> Result<Self, MbConvError>;
}

/// Convert a Rust type into a [`MbValue`].
pub trait IntoMbValue {
    fn into_mb_value(self) -> MbValue;
}

// Blanket: anything that implements IntoMbValue can go through MbValue.
impl<T: IntoMbValue> From<T> for MbValue {
    fn from(v: T) -> Self {
        v.into_mb_value()
    }
}

// ── Primitive implementations ─────────────────────────────────────────────────

impl IntoMbValue for i64 {
    fn into_mb_value(self) -> MbValue {
        MbValue::from_int(self)
    }
}
impl FromMbValue for i64 {
    fn from_mb_value(v: MbValue) -> Result<Self, MbConvError> {
        v.as_int().ok_or(MbConvError::TypeMismatch {
            expected: "int",
            actual: mb_type_name(v),
        })
    }
}

impl IntoMbValue for i32 {
    fn into_mb_value(self) -> MbValue {
        MbValue::from_int(self as i64)
    }
}
impl FromMbValue for i32 {
    fn from_mb_value(v: MbValue) -> Result<Self, MbConvError> {
        let i = i64::from_mb_value(v)?;
        i32::try_from(i).map_err(|_| MbConvError::IntOverflow { value: i })
    }
}

impl IntoMbValue for u32 {
    fn into_mb_value(self) -> MbValue {
        MbValue::from_int(self as i64)
    }
}
impl FromMbValue for u32 {
    fn from_mb_value(v: MbValue) -> Result<Self, MbConvError> {
        let i = i64::from_mb_value(v)?;
        u32::try_from(i).map_err(|_| MbConvError::IntOverflow { value: i })
    }
}

impl IntoMbValue for usize {
    fn into_mb_value(self) -> MbValue {
        MbValue::from_int(self as i64)
    }
}
impl FromMbValue for usize {
    fn from_mb_value(v: MbValue) -> Result<Self, MbConvError> {
        let i = i64::from_mb_value(v)?;
        if i < 0 {
            return Err(MbConvError::IntOverflow { value: i });
        }
        Ok(i as usize)
    }
}

impl IntoMbValue for bool {
    fn into_mb_value(self) -> MbValue {
        MbValue::from_bool(self)
    }
}
impl FromMbValue for bool {
    fn from_mb_value(v: MbValue) -> Result<Self, MbConvError> {
        v.as_bool().ok_or(MbConvError::TypeMismatch {
            expected: "bool",
            actual: mb_type_name(v),
        })
    }
}

impl IntoMbValue for f64 {
    fn into_mb_value(self) -> MbValue {
        MbValue::from_float(self)
    }
}
impl FromMbValue for f64 {
    fn from_mb_value(v: MbValue) -> Result<Self, MbConvError> {
        v.as_float().ok_or(MbConvError::TypeMismatch {
            expected: "float",
            actual: mb_type_name(v),
        })
    }
}

impl IntoMbValue for f32 {
    fn into_mb_value(self) -> MbValue {
        MbValue::from_float(self as f64)
    }
}
impl FromMbValue for f32 {
    fn from_mb_value(v: MbValue) -> Result<Self, MbConvError> {
        Ok(f64::from_mb_value(v)? as f32)
    }
}

// ── String ────────────────────────────────────────────────────────────────────

/// Strings are represented as heap-allocated `MbObject` with `ObjData::Str`.
/// The raw pointer to the `MbObject` is stored in the NaN-boxed PTR slot.
impl IntoMbValue for String {
    fn into_mb_value(self) -> MbValue {
        super::rc::wrap_obj_str(self)
    }
}

impl FromMbValue for String {
    fn from_mb_value(v: MbValue) -> Result<Self, MbConvError> {
        // SAFETY: The pointer was created by MbObject::new_str (or equivalent
        // in the JIT runtime). The caller ensures the value hasn't been freed.
        unsafe {
            super::rc::read_obj_str(v).ok_or(MbConvError::TypeMismatch {
                expected: "str",
                actual: mb_type_name(v),
            })
        }
    }
}

impl IntoMbValue for &str {
    fn into_mb_value(self) -> MbValue {
        self.to_string().into_mb_value()
    }
}

// ── Option<T> ─────────────────────────────────────────────────────────────────

impl<T: IntoMbValue> IntoMbValue for Option<T> {
    fn into_mb_value(self) -> MbValue {
        match self {
            Some(v) => v.into_mb_value(),
            None => MbValue::none(),
        }
    }
}

impl<T: FromMbValue> FromMbValue for Option<T> {
    fn from_mb_value(v: MbValue) -> Result<Self, MbConvError> {
        if v.is_none() {
            Ok(None)
        } else {
            T::from_mb_value(v).map(Some)
        }
    }
}

// ── Vec<T> ────────────────────────────────────────────────────────────────────

/// `Vec<T>` round-trips through a real mamba `list` object via the
/// installed [`crate::ops()`] callback table. Binding crates get a
/// value that mamba code can iterate, index, and pass back — not a
/// crate-private Rust box that only this impl knows how to decode.
impl<T: IntoMbValue + 'static> IntoMbValue for Vec<T> {
    fn into_mb_value(self) -> MbValue {
        let converted: Vec<MbValue> = self.into_iter().map(|x| x.into_mb_value()).collect();
        (super::ops().list_new)(converted)
    }
}

impl<T: FromMbValue + 'static> FromMbValue for Vec<T> {
    fn from_mb_value(v: MbValue) -> Result<Self, MbConvError> {
        let ops = super::ops();
        let len = (ops.list_len)(v).ok_or(MbConvError::TypeMismatch {
            expected: "list",
            actual: mb_type_name(v),
        })?;
        (0..len)
            .map(|i| {
                let elem = (ops.list_get)(v, i).ok_or(MbConvError::TypeMismatch {
                    expected: "list element",
                    actual: "out-of-bounds",
                })?;
                T::from_mb_value(elem)
            })
            .collect()
    }
}

// ── HashMap<String, V> ────────────────────────────────────────────────────────

/// `HashMap<String, V>` round-trips through a real mamba `dict` object
/// with string keys. Non-string keys on the mamba side are silently
/// dropped by [`crate::ops()::dict_iter_str_items`] — matches the
/// contract documented on the ops table.
impl<V: IntoMbValue + 'static> IntoMbValue for HashMap<String, V> {
    fn into_mb_value(self) -> MbValue {
        let ops = super::ops();
        let dict = (ops.dict_new)();
        for (k, v) in self {
            (ops.dict_insert_str)(dict, &k, v.into_mb_value());
        }
        dict
    }
}

impl<V: FromMbValue + 'static> FromMbValue for HashMap<String, V> {
    fn from_mb_value(v: MbValue) -> Result<Self, MbConvError> {
        let items = (super::ops().dict_iter_str_items)(v).ok_or(MbConvError::TypeMismatch {
            expected: "dict",
            actual: mb_type_name(v),
        })?;
        items
            .into_iter()
            .map(|(k, val)| V::from_mb_value(val).map(|v| (k, v)))
            .collect()
    }
}

// ── Opaque native handles ─────────────────────────────────────────────────────

/// Wrap an arbitrary Rust value as an opaque [`MbValue`] PTR.
///
/// The value is heap-allocated as `Box<T>` (a thin pointer) and stored in the
/// NaN-boxed PTR slot.  Use [`mb_unwrap_native`] to recover the original type.
///
/// # Safety
///
/// The returned [`MbValue`] owns the heap allocation.
/// The caller must ensure [`mb_unwrap_native::<T>`] is called exactly once to
/// drop the box, or accept a memory leak.
pub fn mb_wrap_native<T: Any + Send + Sync>(value: T) -> MbValue {
    let ptr = Box::into_raw(Box::new(value)) as usize;
    MbValue::from_ptr(ptr)
}

/// Wrap a Rust value and **tag the resulting pointer with a type name** so
/// that mamba's `getattr` can route attribute reads into a per-type getter
/// registered via [`crate::ops()::register_getter`].
///
/// Without the tag, `mb_wrap_native`-produced pointers are indistinguishable
/// from mamba's own `MbObject` pointers, and `getattr(wrapper, "foo")`
/// would blindly dereference random memory.
///
/// # Layout
///
/// The payload is a plain `Box<T>` (same allocation as `mb_wrap_native`).
/// The association between pointer address and `type_name` lives in the
/// private [`NATIVE_TYPE_NAMES`] map — no inline header, no layout hack.
/// `mb_unwrap_native*` still works unchanged because the pointer still
/// points directly at `T`.
///
/// # Leak note
///
/// The `NATIVE_TYPE_NAMES` entry is added here but **not removed** on
/// `mb_unwrap_native` — the map grows by `(usize, &'static str)` per
/// wrapper and never shrinks. Fine for the current pattern (one wrapper
/// per request/operation); PR-6 can add proper cleanup if it becomes an
/// issue.
pub fn mb_wrap_native_typed<T: Any + Send + Sync>(type_name: &'static str, value: T) -> MbValue {
    let ptr = Box::into_raw(Box::new(value)) as usize;
    let mut guard = NATIVE_TYPE_NAMES
        .write()
        .expect("NATIVE_TYPE_NAMES poisoned");
    guard
        .get_or_insert_with(HashMap::new)
        .insert(ptr, type_name);
    MbValue::from_ptr(ptr)
}

/// Look up the registered type name for a typed native wrapper.
///
/// Returns `None` for untyped `mb_wrap_native` pointers, non-PTR values,
/// mamba `MbObject` pointers, or freed wrappers. Used by mamba's
/// `mb_getattr` to decide whether to route to the registered getter
/// table instead of the default `MbObject` dispatch.
pub fn native_type_name(v: MbValue) -> Option<&'static str> {
    let addr = v.as_ptr()?;
    if addr == 0 {
        return None;
    }
    let guard = NATIVE_TYPE_NAMES
        .read()
        .expect("NATIVE_TYPE_NAMES poisoned");
    guard.as_ref()?.get(&addr).copied()
}

static NATIVE_TYPE_NAMES: RwLock<Option<HashMap<usize, &'static str>>> = RwLock::new(None);

/// Unwrap an opaque [`MbValue`] PTR back into a Rust value of type `T`.
///
/// Returns `None` if the value is not a PTR or the pointer is null.
///
/// # Safety
///
/// `v` must have been created by [`mb_wrap_native::<T>`] with the **exact same**
/// concrete type `T`. Calling with a mismatched `T` is undefined behaviour.
/// The `MbValue` must not be used again after this call.
pub unsafe fn mb_unwrap_native<T: Any>(v: MbValue) -> Option<T> {
    let addr = v.as_ptr()?;
    if addr == 0 {
        return None;
    }
    // SAFETY: addr was created by Box::into_raw(Box::new(value: T)).
    Some(*Box::from_raw(addr as *mut T))
}

/// Borrow an opaque [`MbValue`] PTR as `&T` without taking ownership.
///
/// Use this from FFI attribute getters that need to read fields of a
/// wrapped native value while leaving it alive for the caller.
///
/// # Safety
///
/// `v` must have been created by [`mb_wrap_native::<T>`] with the same
/// concrete type `T`. The returned borrow's `'static` lifetime is a
/// safety lie — the caller contract is that `v` (and therefore the
/// underlying `Box<T>`) outlives the returned reference. In practice for
/// FFI shims this holds trivially: the arg slot lives for the duration
/// of the call.
pub unsafe fn mb_unwrap_native_ref<T: Any>(v: MbValue) -> Option<&'static T> {
    let addr = v.as_ptr()?;
    if addr == 0 {
        return None;
    }
    Some(&*(addr as *const T))
}

/// Borrow an opaque [`MbValue`] PTR as `&mut T`. Mirror of
/// [`mb_unwrap_native_ref`] for setters.
///
/// # Safety
///
/// Same contract as [`mb_unwrap_native_ref`] plus exclusive access: no
/// other code may borrow the same wrapped value (shared or mutable) for
/// the lifetime of the returned reference.
pub unsafe fn mb_unwrap_native_mut<T: Any>(v: MbValue) -> Option<&'static mut T> {
    let addr = v.as_ptr()?;
    if addr == 0 {
        return None;
    }
    Some(&mut *(addr as *mut T))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn mb_type_name(v: MbValue) -> &'static str {
    if v.is_float() {
        "float"
    } else if v.is_int() {
        "int"
    } else if v.is_bool() {
        "bool"
    } else if v.is_none() {
        "None"
    } else if v.is_ptr() {
        "ptr"
    } else if v.is_func() {
        "func"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_ops;

    #[test]
    fn test_i64_roundtrip() {
        for i in [0i64, 1, -1, 1000, -1000, (1 << 47) - 1, -(1 << 47)] {
            let v = i.into_mb_value();
            assert_eq!(i64::from_mb_value(v).unwrap(), i);
        }
    }

    #[test]
    fn test_f64_roundtrip() {
        for f in [0.0f64, 1.0, -1.5, 3.14, f64::INFINITY] {
            let v = f.into_mb_value();
            assert!((f64::from_mb_value(v).unwrap() - f).abs() < f64::EPSILON || f.is_infinite());
        }
    }

    #[test]
    fn test_bool_roundtrip() {
        assert_eq!(bool::from_mb_value(true.into_mb_value()).unwrap(), true);
        assert_eq!(bool::from_mb_value(false.into_mb_value()).unwrap(), false);
    }

    #[test]
    fn test_option_none() {
        let v: Option<i64> = None;
        let mv = v.into_mb_value();
        assert!(mv.is_none());
        let back: Option<i64> = Option::from_mb_value(mv).unwrap();
        assert!(back.is_none());
    }

    #[test]
    fn test_option_some() {
        let v: Option<i64> = Some(42);
        let mv = v.into_mb_value();
        let back: Option<i64> = Option::from_mb_value(mv).unwrap();
        assert_eq!(back, Some(42));
    }

    #[test]
    fn test_vec_roundtrip() {
        test_ops::init();
        let v: Vec<i64> = vec![1, 2, 3];
        let mv = v.into_mb_value();
        let back: Vec<i64> = Vec::from_mb_value(mv).unwrap();
        assert_eq!(back, [1, 2, 3]);
    }

    #[test]
    fn test_hashmap_roundtrip() {
        test_ops::init();
        let mut map = HashMap::new();
        map.insert("a".to_string(), 1i64);
        map.insert("b".to_string(), 2i64);
        let mv = map.clone().into_mb_value();
        let back: HashMap<String, i64> = HashMap::from_mb_value(mv).unwrap();
        assert_eq!(back["a"], 1);
        assert_eq!(back["b"], 2);
    }

    #[test]
    fn test_option_hashmap_roundtrip() {
        test_ops::init();
        let v: HashMap<String, Option<i64>> = {
            let mut m = HashMap::new();
            m.insert("x".to_string(), Some(10i64));
            m.insert("y".to_string(), None);
            m
        };
        let mv = v.clone().into_mb_value();
        let back: HashMap<String, Option<i64>> = HashMap::from_mb_value(mv).unwrap();
        assert_eq!(back["x"], Some(10));
        assert_eq!(back["y"], None);
    }

    #[test]
    fn test_wrap_unwrap_native() {
        #[derive(Debug, PartialEq)]
        struct Point {
            x: i32,
            y: i32,
        }

        let v = mb_wrap_native(Point { x: 1, y: 2 });
        assert!(v.is_ptr());
        let back: Option<Point> = unsafe { mb_unwrap_native(v) };
        assert_eq!(back, Some(Point { x: 1, y: 2 }));
    }

    #[test]
    fn test_wrap_native_typed_stores_type_name() {
        #[derive(Debug)]
        struct HTTPException {
            status_code: u16,
        }
        let v = mb_wrap_native_typed("HTTPException", HTTPException { status_code: 418 });
        assert!(v.is_ptr());
        assert_eq!(native_type_name(v), Some("HTTPException"));
        // Payload readable through the existing borrow path.
        let exc: &HTTPException = unsafe { mb_unwrap_native_ref(v) }.unwrap();
        assert_eq!(exc.status_code, 418);
    }

    #[test]
    fn test_native_type_name_misses_untyped_wrapper() {
        // Plain `mb_wrap_native` doesn't register a type name — mamba's
        // getattr must fall through to the normal MbObject dispatch for
        // these pointers (not attempt a getter lookup).
        let v = mb_wrap_native(42i64);
        assert_eq!(native_type_name(v), None);
    }

    #[test]
    fn test_native_type_name_misses_non_ptr() {
        assert_eq!(native_type_name(MbValue::from_int(5)), None);
        assert_eq!(native_type_name(MbValue::from_float(1.5)), None);
        assert_eq!(native_type_name(MbValue::from_bool(true)), None);
        assert_eq!(native_type_name(MbValue::none()), None);
    }

    #[test]
    fn test_type_mismatch_error() {
        let v = MbValue::from_float(1.0);
        let result = i64::from_mb_value(v);
        assert!(matches!(
            result,
            Err(MbConvError::TypeMismatch {
                expected: "int",
                ..
            })
        ));
    }

    #[test]
    fn test_int_overflow() {
        let large: i64 = i32::MAX as i64 + 1;
        let v = large.into_mb_value();
        let result = i32::from_mb_value(v);
        assert!(matches!(result, Err(MbConvError::IntOverflow { .. })));
    }
}
