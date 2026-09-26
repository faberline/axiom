//! String interop helpers that route through the installed [`ObjectOps`]
//! table. Formerly a layout mirror of `mamba::runtime::rc::{MbObject,
//! ObjData, ObjKind, MbObjectHeader}`; the mirror was deleted in PR-6
//! because it had already drifted (mamba grew an `ObjData::CodeObject`
//! variant that was never added here, reshaping the enum discriminant).
//!
//! # Migration note
//!
//! Existing binding crates called `rc::wrap_obj_str(s)` and
//! `unsafe { rc::read_obj_str(v) }` — those names are preserved here
//! as thin wrappers over `ops().str_new` / `ops().str_read`. The
//! `unsafe` on `read_obj_str` is retained for source compatibility
//! across the ~18 call sites; the body no longer dereferences raw
//! pointers itself, so new callers should prefer
//! [`crate::ops()`]`().str_read` directly.

use super::ops;
use super::MbValue;

/// Wrap a Rust `String` as a mamba `str`-shaped `MbValue`. Refcount is 1
/// (the caller holds the only reference).
///
/// Implemented via [`ObjectOps::str_new`]; the real mamba runtime
/// allocates a canonical `MbObject { data: ObjData::Str(_), … }` so
/// later JIT code, stdlib, etc. see a fully-formed mamba string.
pub fn wrap_obj_str(s: String) -> MbValue {
    (ops().str_new)(&s)
}

/// Read the contents of a mamba `str`-shaped `MbValue`. Returns `None`
/// if the value is not a pointer to a mamba string.
///
/// # Safety
///
/// Preserved for source compatibility with existing binding crates that
/// wrap calls in `unsafe { … }`. The implementation no longer performs
/// unchecked pointer derefs itself — it delegates to
/// [`ObjectOps::str_read`], which is the mamba runtime's own
/// well-typed string read. New code should call that directly.
pub unsafe fn read_obj_str(v: MbValue) -> Option<String> {
    (ops().str_read)(v)
}
