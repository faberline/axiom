#![allow(improper_ctypes_definitions)]

//! Mamba FFI for [`pg::orm::blocking::Session`].
//!
//! Exposes the ORM Session / unit-of-work / identity-map surface as
//! native-call symbols mounted under `mambalibs.pg`.
//!
//! # Surface
//!
//! | Symbol                          | Mamba call                                            |
//! |---------------------------------|-------------------------------------------------------|
//! | `mb_pg_session_new`             | `Session(conn)`                                       |
//! | `mb_pg_session_begin`           | `session_begin(session)`                              |
//! | `mb_pg_session_commit`          | `session_commit(session)`                             |
//! | `mb_pg_session_rollback`        | `session_rollback(session)`                           |
//! | `mb_pg_session_flush`           | `session_flush(session)`                              |
//! | `mb_pg_session_add`             | `session_add(session, table, dict) -> slot`          |
//! | `mb_pg_session_slot_read`       | `session_slot_read(slot) -> int`                      |
//! | `mb_pg_session_delete`          | `session_delete(session, table, pk)`                  |
//! | `mb_pg_session_touch`           | `session_touch(session, table, dict_with_id)`         |
//! | `mb_pg_session_get`             | `session_get(session, table, pk) -> dict?`            |
//! | `mb_pg_session_query_all`       | `session_query_all(session, table, filter) -> list`   |
//! | `mb_pg_session_query_first`     | `session_query_first(session, table, filter) -> dict?`|
//! | `mb_pg_session_close`           | `session_close(session)`                              |
//!
//! Slot handle: `session_add` stages an INSERT and returns an opaque
//! handle to the `Arc<Mutex<i64>>` pk slot that the UoW will populate
//! during `flush`/`commit`. Read the assigned pk back with
//! `session_slot_read` after the next `commit`/`flush`.

// HANDWRITE-BEGIN reason: mamba-FFI generator codegen gap; the spec
//   describes an MbValue ABI that no `section_type` today emits. Closes
//   when score grows a `mamba-binding` section type.
//! @spec
//!   .aw/tech-design/projects/pg/specs/pg-orm-session-uow.md#changes

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mamba_registry::convert::mb_wrap_native;
use mamba_registry::MbValue;

use pg::driver::ExtractedValue;

use crate::types::{MbPgConnection, MbPgSession};

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

fn wrap_str(s: String) -> MbValue {
    mamba_registry::test_ops::init();
    mamba_registry::rc::wrap_obj_str(s)
}

#[inline]
unsafe fn handle<'a, T>(v: MbValue) -> Option<&'a T> {
    v.as_ptr()
        .filter(|a| *a != 0)
        .map(|a| unsafe { &*(a as *const T) })
}

/// Insert slot — opaque handle around the `Arc<Mutex<i64>>` the UoW
/// populates after flush.
pub struct MbPgInsertSlot {
    pub inner: Arc<Mutex<i64>>,
}

// ── ExtractedValue ↔ MbValue ──────────────────────────────────────────────────

/// Convert a mamba value into the driver's `ExtractedValue`. The
/// session dyn surface is the only consumer, so we cover the subset of
/// types the test plan exercises (None / bool / int / float / str).
/// Other variants (Uuid, Date, Json, …) fall through to `Null` —
/// callers that need them should bind via the typed surface.
fn mb_to_extracted(v: MbValue) -> ExtractedValue {
    if v.is_none() {
        ExtractedValue::Null
    } else if let Some(b) = v.as_bool() {
        ExtractedValue::Bool(b)
    } else if let Some(i) = v.as_int() {
        ExtractedValue::BigInt(i)
    } else if let Some(f) = v.as_float() {
        ExtractedValue::Double(f)
    } else if let Some(s) = read_str(v) {
        ExtractedValue::String(s)
    } else {
        ExtractedValue::Null
    }
}

/// Convert a driver `ExtractedValue` back into a mamba value. Mirrors
/// `mb_to_extracted`'s coverage; types outside the MVP subset are
/// stringified so the round-trip is observable (instead of silently
/// becoming `None`).
fn extracted_to_mb(ev: &ExtractedValue) -> MbValue {
    match ev {
        ExtractedValue::Null => MbValue::none(),
        ExtractedValue::Bool(b) => MbValue::from_bool(*b),
        ExtractedValue::SmallInt(v) => MbValue::from_int(*v as i64),
        ExtractedValue::Int(v) => MbValue::from_int(*v as i64),
        ExtractedValue::BigInt(v) => MbValue::from_int(*v),
        ExtractedValue::Float(v) => MbValue::from_float(*v as f64),
        ExtractedValue::Double(v) => MbValue::from_float(*v),
        ExtractedValue::String(s) => wrap_str(s.clone()),
        other => wrap_str(format!("{other:?}")),
    }
}

/// Decode a mamba dict (`HashMap<str, MbValue>`) into the
/// `(name, ExtractedValue)` rows the session dyn surface expects.
fn dict_to_values(v: MbValue) -> Vec<(String, ExtractedValue)> {
    mamba_registry::test_ops::init();
    let ops = mamba_registry::ops();
    match (ops.dict_iter_str_items)(v) {
        Some(items) => items
            .into_iter()
            .map(|(k, val)| (k, mb_to_extracted(val)))
            .collect(),
        None => Vec::new(),
    }
}

/// Encode a `HashMap<String, ExtractedValue>` row as a mamba dict.
fn values_to_dict(row: HashMap<String, ExtractedValue>) -> MbValue {
    mamba_registry::test_ops::init();
    let ops = mamba_registry::ops();
    let dict = (ops.dict_new)();
    for (k, v) in row {
        (ops.dict_insert_str)(dict, &k, extracted_to_mb(&v));
    }
    dict
}

/// Wrap a list-of-rows as a mamba list of dicts.
fn rows_to_list(rows: Vec<HashMap<String, ExtractedValue>>) -> MbValue {
    let dicts: Vec<MbValue> = rows.into_iter().map(values_to_dict).collect();
    let boxed: Box<Vec<MbValue>> = Box::new(dicts);
    MbValue::from_ptr(Box::into_raw(boxed) as usize)
}

// ── Session verbs ────────────────────────────────────────────────────────────

/// `Session(conn) -> Session`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_new(args: *const MbValue, nargs: usize) -> MbValue {
    let conn = match unsafe { handle::<MbPgConnection>(arg(args, nargs, 0)) } {
        Some(c) => c,
        None => return MbValue::none(),
    };
    mb_wrap_native(MbPgSession::new(conn.inner.clone()))
}

/// `session_begin(session) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_begin(args: *const MbValue, nargs: usize) -> MbValue {
    let sess = match unsafe { handle::<MbPgSession>(arg(args, nargs, 0)) } {
        Some(s) => s,
        None => return MbValue::none(),
    };
    let mut guard = match sess.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    if let Some(owned) = guard.as_mut() {
        let _ = owned.session().begin();
    }
    MbValue::none()
}

/// `session_commit(session) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_commit(args: *const MbValue, nargs: usize) -> MbValue {
    let sess = match unsafe { handle::<MbPgSession>(arg(args, nargs, 0)) } {
        Some(s) => s,
        None => return MbValue::none(),
    };
    let mut guard = match sess.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    if let Some(owned) = guard.as_mut() {
        let _ = owned.session().commit();
    }
    MbValue::none()
}

/// `session_rollback(session) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_rollback(args: *const MbValue, nargs: usize) -> MbValue {
    let sess = match unsafe { handle::<MbPgSession>(arg(args, nargs, 0)) } {
        Some(s) => s,
        None => return MbValue::none(),
    };
    let mut guard = match sess.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    if let Some(owned) = guard.as_mut() {
        let _ = owned.session().rollback();
    }
    MbValue::none()
}

/// `session_flush(session) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_flush(args: *const MbValue, nargs: usize) -> MbValue {
    let sess = match unsafe { handle::<MbPgSession>(arg(args, nargs, 0)) } {
        Some(s) => s,
        None => return MbValue::none(),
    };
    let mut guard = match sess.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    if let Some(owned) = guard.as_mut() {
        let _ = owned.session().flush();
    }
    MbValue::none()
}

/// `session_add(session, table, dict) -> slot`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_add(args: *const MbValue, nargs: usize) -> MbValue {
    let sess = match unsafe { handle::<MbPgSession>(arg(args, nargs, 0)) } {
        Some(s) => s,
        None => return MbValue::none(),
    };
    let table = match read_str(unsafe { arg(args, nargs, 1) }) {
        Some(s) if !s.is_empty() => s,
        _ => return MbValue::none(),
    };
    let values = dict_to_values(unsafe { arg(args, nargs, 2) });

    let mut guard = match sess.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    let owned = match guard.as_mut() {
        Some(o) => o,
        None => return MbValue::none(),
    };
    let slot = owned.session().add_dyn(&table, values);
    mb_wrap_native(MbPgInsertSlot { inner: slot })
}

/// `session_slot_read(slot) -> int`
///
/// Reads the pk assigned by the most recent `flush` / `commit`.
/// Returns `0` if the slot has not yet been populated.
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_slot_read(args: *const MbValue, nargs: usize) -> MbValue {
    let slot = match unsafe { handle::<MbPgInsertSlot>(arg(args, nargs, 0)) } {
        Some(s) => s,
        None => return MbValue::none(),
    };
    let pk = slot.inner.lock().map(|g| *g).unwrap_or(0);
    MbValue::from_int(pk)
}

/// `session_delete(session, table, pk) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_delete(args: *const MbValue, nargs: usize) -> MbValue {
    let sess = match unsafe { handle::<MbPgSession>(arg(args, nargs, 0)) } {
        Some(s) => s,
        None => return MbValue::none(),
    };
    let table = match read_str(unsafe { arg(args, nargs, 1) }) {
        Some(s) if !s.is_empty() => s,
        _ => return MbValue::none(),
    };
    let pk = match unsafe { arg(args, nargs, 2) }.as_int() {
        Some(v) => v,
        None => return MbValue::none(),
    };

    let mut guard = match sess.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    if let Some(owned) = guard.as_mut() {
        owned.session().delete_dyn(&table, pk);
    }
    MbValue::none()
}

/// `session_touch(session, table, dict_with_id) -> None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_touch(args: *const MbValue, nargs: usize) -> MbValue {
    let sess = match unsafe { handle::<MbPgSession>(arg(args, nargs, 0)) } {
        Some(s) => s,
        None => return MbValue::none(),
    };
    let table = match read_str(unsafe { arg(args, nargs, 1) }) {
        Some(s) if !s.is_empty() => s,
        _ => return MbValue::none(),
    };
    let mut values = dict_to_values(unsafe { arg(args, nargs, 2) });

    let pk_idx = values.iter().position(|(k, _)| k == "id");
    let pk = match pk_idx {
        Some(idx) => match &values[idx].1 {
            ExtractedValue::BigInt(v) => *v,
            ExtractedValue::Int(v) => *v as i64,
            ExtractedValue::SmallInt(v) => *v as i64,
            _ => return MbValue::none(),
        },
        None => return MbValue::none(),
    };
    // SessionModel::to_values omits the pk column; mirror that for the
    // dyn variant so the staged UPDATE doesn't try to rewrite `id`.
    values.remove(pk_idx.unwrap());

    let mut guard = match sess.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    if let Some(owned) = guard.as_mut() {
        owned.session().touch_dyn(&table, pk, values);
    }
    MbValue::none()
}

/// `session_get(session, table, pk) -> dict | None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_get(args: *const MbValue, nargs: usize) -> MbValue {
    let sess = match unsafe { handle::<MbPgSession>(arg(args, nargs, 0)) } {
        Some(s) => s,
        None => return MbValue::none(),
    };
    let table = match read_str(unsafe { arg(args, nargs, 1) }) {
        Some(s) if !s.is_empty() => s,
        _ => return MbValue::none(),
    };
    let pk = match unsafe { arg(args, nargs, 2) }.as_int() {
        Some(v) => v,
        None => return MbValue::none(),
    };

    let mut guard = match sess.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    let owned = match guard.as_mut() {
        Some(o) => o,
        None => return MbValue::none(),
    };
    match owned.session().get_dyn(&table, pk) {
        Ok(Some(row)) => values_to_dict(row),
        _ => MbValue::none(),
    }
}

/// `session_query_all(session, table, filter_dict) -> list[dict]`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_query_all(args: *const MbValue, nargs: usize) -> MbValue {
    let sess = match unsafe { handle::<MbPgSession>(arg(args, nargs, 0)) } {
        Some(s) => s,
        None => return MbValue::none(),
    };
    let table = match read_str(unsafe { arg(args, nargs, 1) }) {
        Some(s) if !s.is_empty() => s,
        _ => return MbValue::none(),
    };
    let filter = dict_to_values(unsafe { arg(args, nargs, 2) });

    let mut guard = match sess.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    let owned = match guard.as_mut() {
        Some(o) => o,
        None => return MbValue::none(),
    };
    match owned.session().query_all_dyn(&table, &filter) {
        Ok(rows) => rows_to_list(rows),
        Err(_) => MbValue::none(),
    }
}

/// `session_query_first(session, table, filter_dict) -> dict | None`
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_query_first(args: *const MbValue, nargs: usize) -> MbValue {
    let sess = match unsafe { handle::<MbPgSession>(arg(args, nargs, 0)) } {
        Some(s) => s,
        None => return MbValue::none(),
    };
    let table = match read_str(unsafe { arg(args, nargs, 1) }) {
        Some(s) if !s.is_empty() => s,
        _ => return MbValue::none(),
    };
    let filter = dict_to_values(unsafe { arg(args, nargs, 2) });

    let mut guard = match sess.inner.lock() {
        Ok(g) => g,
        Err(_) => return MbValue::none(),
    };
    let owned = match guard.as_mut() {
        Some(o) => o,
        None => return MbValue::none(),
    };
    match owned.session().query_first_dyn(&table, &filter) {
        Ok(Some(row)) => values_to_dict(row),
        _ => MbValue::none(),
    }
}

/// `session_close(session) -> None`
///
/// Drops the inner `OwnedSession` (and its outer transaction, if
/// any), releasing the underlying connection back to the pool.
#[no_mangle]
pub unsafe extern "C" fn mb_pg_session_close(args: *const MbValue, nargs: usize) -> MbValue {
    let sess = match unsafe { handle::<MbPgSession>(arg(args, nargs, 0)) } {
        Some(s) => s,
        None => return MbValue::none(),
    };
    if let Ok(mut guard) = sess.inner.lock() {
        let _ = guard.take();
    }
    MbValue::none()
}

// HANDWRITE-END
