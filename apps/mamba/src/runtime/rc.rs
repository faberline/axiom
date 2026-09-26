use super::dict_ops::DictKey;
use indexmap::IndexMap;
use num_bigint::BigInt;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::sync::atomic::{AtomicU32, Ordering};
/// Reference-counted heap objects (#280) — thread-safe, no-GIL.
///
/// Every heap-allocated Mamba object starts with a MbObjectHeader containing
/// an atomic reference count and a type tag. The runtime uses `mb_retain` /
/// `mb_release` to manage lifetimes. When refcount drops to zero, the object
/// is freed.
///
/// Cycle collection is deferred — containers (list, dict) are tracked separately
/// and a mark-sweep collector runs periodically to break cycles.
///
/// # Ownership Audit (#1129)
///
/// Every `mb_*` function registered in `runtime_symbols()` that returns `MbValue`
/// is classified as NEW, BORROWED, or VOID. Borrowed-reference functions call
/// `retain_if_ptr(result)` before returning so callers always receive an owned
/// reference.
///
/// ## NEW (caller owns, rc=1 — no retain needed)
///
/// Constructors / allocators:
///   mb_list_new, mb_list_from, mb_list_from_iterable, mb_list_copy,
///   mb_list_concat, mb_list_repeat, mb_list_pop, mb_list_pop_at,
///   mb_list_to_tuple, mb_dict_new, mb_dict_from_pairs, mb_dict_copy,
///   mb_dict_keys, mb_dict_values, mb_dict_items, mb_dict_pop,
///   mb_set_new, mb_set_from_list, mb_set_from_iterable,
///   mb_tuple_new, mb_tuple_from, mb_tuple_from_iterable,
///   mb_str_concat, mb_str, mb_repr, mb_str_format, mb_str_join,
///   mb_str_split, mb_str_upper, mb_str_lower, mb_str_replace,
///   mb_str_strip, mb_str_lstrip, mb_str_rstrip, mb_str_encode,
///   mb_bytes_decode, mb_bytes_new, mb_bytes_concat,
///   mb_instance_new, mb_instance_new_with_init,
///   mb_exception_new, mb_exception_new_with_args,
///   mb_iter, mb_enumerate, mb_zip, mb_range,
///   mb_closure_new, mb_cell_new,
///   mb_generator_create, mb_frozenset_new,
///   mb_sorted, mb_reversed, mb_list_comprehension,
///   mb_dict_comprehension, mb_set_comprehension,
///   mb_box_int, mb_box_bool, mb_box_float
///
/// Arithmetic / comparison (return NaN-boxed or new objects):
///   mb_add, mb_sub, mb_mul, mb_truediv, mb_floordiv, mb_mod,
///   mb_pow, mb_neg, mb_pos, mb_invert, mb_lshift, mb_rshift,
///   mb_bitand, mb_bitor, mb_bitxor, mb_matmul,
///   mb_eq, mb_ne, mb_lt, mb_le, mb_gt, mb_ge, mb_not
///
/// ## BORROWED (container/global still owns — return_owned added)
///
///   mb_list_getitem, mb_dict_getitem, mb_tuple_getitem,
///   mb_seq_getitem, mb_getattr, mb_getattr_default,
///   mb_global_get, mb_global_get_id, mb_cell_get,
///   mb_closure_get_capture, mb_module_getattr, mb_import_from,
///   mb_next, mb_next_raise, mb_generator_yield_value,
///   mb_coroutine_get_local, mb_property_get, mb_super_getattr,
///   mb_dict_get, mb_dict_setdefault,
///   mb_catch_exception, mb_catch_exception_instance
///
/// ## VOID (no MbValue return — not in scope)
///
///   mb_list_append, mb_list_extend, mb_list_insert,
///   mb_list_remove, mb_list_clear, mb_list_reverse, mb_list_sort,
///   mb_dict_setitem, mb_dict_update, mb_dict_clear,
///   mb_set_add, mb_set_discard, mb_set_remove, mb_set_clear,
///   mb_setattr, mb_print, mb_gc_collect
///
/// ## NON-POINTER (returns NaN-boxed i64/f64/bool — no heap refcount)
///
///   mb_len, mb_is_truthy, mb_hash, mb_id, mb_bool
#[inline]
fn is_typed_native_wrapper(val: super::value::MbValue) -> bool {
    let registry_value = mamba_registry::MbValue::from_bits(val.to_bits());
    mamba_registry::convert::native_type_name(registry_value).is_some()
}

/// Backing buffer for `ObjData::List` and `ObjData::Set` (#2517).
///
/// `SmallVec<[MbValue; 8]>` is inline-storage-up-to-8-elements: a
/// `[a,b,c,d]`-style list literal stores its elements directly inside
/// the `MbObject` heap allocation, eliminating the inner `Vec` heap
/// alloc that previously dominated literal construction cost. Beyond
/// 8 elements the SmallVec spills to a heap buffer (identical to
/// `Vec`'s representation, no extra cost).
///
/// `SmallVec` is API-compatible with `Vec` for `.push() / .pop() /
/// .iter() / .iter_mut() / .len() / .is_empty() / .clear() /
/// .extend() / .extend_from_slice() / .truncate() / .swap_remove() /
/// .insert() / .remove() / .resize() / .as_slice() / .as_mut_slice() /
/// .drain() / .retain() / .sort_by() / .with_capacity() / .capacity()`
/// — the ~387 `ObjData::List(ref lock)` pattern-match arms across the
/// runtime compile unchanged.
///
/// Inline capacity 8 covers the dominant literal arity (`mb_list_new_1`
/// through `mb_list_new_8` fixed-arity JIT shims).
pub type MbList = SmallVec<[super::value::MbValue; 8]>;

/// Backing storage representation for `ObjData::List`.
///
/// Stage 1 (#1071-4 / WI #1075) introduces unboxed contiguous scalar buffers
/// (`Int` for `i64` and `Float` for `f64`) alongside `Generic` (`MbValue`).
/// Boundary rule: crossing into an `Any` context boxes lazily via `ensure_generic`.
#[derive(Clone, Debug)]
pub enum MbListBuffer {
    Generic(MbList),
    Int(SmallVec<[i64; 8]>),
    Float(SmallVec<[f64; 8]>),
}

impl Default for MbListBuffer {
    #[inline]
    fn default() -> Self {
        MbListBuffer::Generic(SmallVec::new())
    }
}

impl std::ops::Deref for MbListBuffer {
    type Target = [super::value::MbValue];

    #[inline]
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Generic(v) => v.as_slice(),
            _ => &[],
        }
    }
}

impl std::ops::DerefMut for MbListBuffer {
    #[inline]
    fn deref_mut(&mut self) -> &mut [super::value::MbValue] {
        self.ensure_generic().as_mut_slice()
    }
}

impl<'a> IntoIterator for &'a mut MbListBuffer {
    type Item = &'a mut super::value::MbValue;
    type IntoIter = std::slice::IterMut<'a, super::value::MbValue>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.ensure_generic().iter_mut()
    }
}

impl IntoIterator for MbListBuffer {
    type Item = super::value::MbValue;
    type IntoIter = std::vec::IntoIter<super::value::MbValue>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.to_vec().into_iter()
    }
}

impl MbListBuffer {
    #[inline]
    pub fn len(&self) -> usize {
        match self {
            Self::Generic(v) => v.len(),
            Self::Int(v) => v.len(),
            Self::Float(v) => v.len(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Generic(v) => v.is_empty(),
            Self::Int(v) => v.is_empty(),
            Self::Float(v) => v.is_empty(),
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        match self {
            Self::Generic(v) => v.capacity(),
            Self::Int(v) => v.capacity(),
            Self::Float(v) => v.capacity(),
        }
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<super::value::MbValue> {
        match self {
            Self::Generic(v) => v.get(index).copied(),
            Self::Int(v) => v.get(index).map(|&x| super::bigint_ops::int_from_i64(x)),
            Self::Float(v) => v.get(index).map(|&x| super::value::MbValue::from_float(x)),
        }
    }

    #[inline]
    pub fn first(&self) -> Option<super::value::MbValue> {
        self.get(0)
    }

    #[inline]
    pub fn last(&self) -> Option<super::value::MbValue> {
        let len = self.len();
        if len == 0 {
            None
        } else {
            self.get(len - 1)
        }
    }

    #[inline]
    pub fn iter(&self) -> std::vec::IntoIter<super::value::MbValue> {
        self.to_vec().into_iter()
    }

    pub fn slice(&self, start: usize, end: usize) -> Vec<super::value::MbValue> {
        let len = self.len();
        let s = start.min(len);
        let e = end.min(len).max(s);
        match self {
            Self::Generic(v) => v[s..e].to_vec(),
            Self::Int(v) => v[s..e].iter().map(|&x| super::bigint_ops::int_from_i64(x)).collect(),
            Self::Float(v) => v[s..e].iter().map(|&x| super::value::MbValue::from_float(x)).collect(),
        }
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        match self {
            Self::Generic(v) => v.swap(a, b),
            Self::Int(v) => v.swap(a, b),
            Self::Float(v) => v.swap(a, b),
        }
    }

    pub fn ensure_generic(&mut self) -> &mut MbList {
        match self {
            Self::Generic(v) => v,
            Self::Int(v) => {
                let gen: MbList = v.iter().map(|&x| super::bigint_ops::int_from_i64(x)).collect();
                *self = Self::Generic(gen);
                match self {
                    Self::Generic(v) => v,
                    _ => unreachable!(),
                }
            }
            Self::Float(v) => {
                let gen: MbList = v.iter().map(|&x| super::value::MbValue::from_float(x)).collect();
                *self = Self::Generic(gen);
                match self {
                    Self::Generic(v) => v,
                    _ => unreachable!(),
                }
            }
        }
    }

    pub fn push(&mut self, val: super::value::MbValue) {
        match self {
            Self::Int(v) => {
                if let Some(i) = val.as_int() {
                    v.push(i);
                } else {
                    self.ensure_generic().push(val);
                }
            }
            Self::Float(v) => {
                if let Some(f) = val.as_float() {
                    v.push(f);
                } else {
                    self.ensure_generic().push(val);
                }
            }
            Self::Generic(v) => {
                v.push(val);
            }
        }
    }

    pub fn push_int(&mut self, val: i64) {
        match self {
            Self::Int(v) => v.push(val),
            Self::Generic(v) => v.push(super::bigint_ops::int_from_i64(val)),
            Self::Float(_) => {
                self.ensure_generic().push(super::bigint_ops::int_from_i64(val));
            }
        }
    }

    pub fn push_float(&mut self, val: f64) {
        match self {
            Self::Float(v) => v.push(val),
            Self::Generic(v) => v.push(super::value::MbValue::from_float(val)),
            Self::Int(_) => {
                self.ensure_generic().push(super::value::MbValue::from_float(val));
            }
        }
    }

    pub fn to_vec(&self) -> Vec<super::value::MbValue> {
        match self {
            Self::Generic(v) => v.to_vec(),
            Self::Int(v) => v.iter().map(|&x| super::bigint_ops::int_from_i64(x)).collect(),
            Self::Float(v) => v.iter().map(|&x| super::value::MbValue::from_float(x)).collect(),
        }
    }

    pub fn clear(&mut self) {
        match self {
            Self::Generic(v) => {
                for elem in v.drain(..) {
                    unsafe {
                        release_owned(elem);
                    }
                }
            }
            Self::Int(v) => v.clear(),
            Self::Float(v) => v.clear(),
        }
    }

    pub fn extend<I: IntoIterator<Item = super::value::MbValue>>(&mut self, iter: I) {
        for val in iter {
            self.push(val);
        }
    }

    pub fn reverse(&mut self) {
        match self {
            Self::Generic(v) => v.reverse(),
            Self::Int(v) => v.reverse(),
            Self::Float(v) => v.reverse(),
        }
    }

    pub fn pop(&mut self) -> Option<super::value::MbValue> {
        match self {
            Self::Generic(v) => v.pop(),
            Self::Int(v) => v.pop().map(super::bigint_ops::int_from_i64),
            Self::Float(v) => v.pop().map(super::value::MbValue::from_float),
        }
    }

    pub fn remove(&mut self, index: usize) -> super::value::MbValue {
        match self {
            Self::Generic(v) => v.remove(index),
            Self::Int(v) => super::bigint_ops::int_from_i64(v.remove(index)),
            Self::Float(v) => super::value::MbValue::from_float(v.remove(index)),
        }
    }

    pub fn swap_remove(&mut self, index: usize) -> super::value::MbValue {
        match self {
            Self::Generic(v) => v.swap_remove(index),
            Self::Int(v) => super::bigint_ops::int_from_i64(v.swap_remove(index)),
            Self::Float(v) => super::value::MbValue::from_float(v.swap_remove(index)),
        }
    }

    pub fn insert(&mut self, index: usize, element: super::value::MbValue) {
        match self {
            Self::Int(v) => {
                if let Some(i) = element.as_int() {
                    v.insert(index, i);
                } else {
                    self.ensure_generic().insert(index, element);
                }
            }
            Self::Float(v) => {
                if let Some(f) = element.as_float() {
                    v.insert(index, f);
                } else {
                    self.ensure_generic().insert(index, element);
                }
            }
            Self::Generic(v) => {
                v.insert(index, element);
            }
        }
    }

    pub fn retain<F: FnMut(&super::value::MbValue) -> bool>(&mut self, mut f: F) {
        match self {
            Self::Generic(v) => v.retain(|elem| f(elem)),
            Self::Int(v) => {
                v.retain(|i| f(&super::bigint_ops::int_from_i64(*i)));
            }
            Self::Float(v) => {
                v.retain(|fl| f(&super::value::MbValue::from_float(*fl)));
            }
        }
    }

    pub fn extend_from_slice(&mut self, other: &[super::value::MbValue]) {
        for &val in other {
            self.push(val);
        }
    }

    pub fn as_slice(&self) -> &[super::value::MbValue] {
        match self {
            Self::Generic(v) => v.as_slice(),
            _ => &[],
        }
    }

    pub fn as_ptr(&self) -> *const super::value::MbValue {
        match self {
            Self::Generic(v) => v.as_ptr(),
            _ => std::ptr::null(),
        }
    }

    pub fn iter_values(&self) -> Vec<super::value::MbValue> {
        self.to_vec()
    }

    pub fn insert_from_slice(&mut self, index: usize, slice: &[super::value::MbValue]) {
        self.ensure_generic();
        if let Self::Generic(v) = self {
            v.insert_many(index, slice.iter().copied());
        }
    }

    pub fn sort_by<F: FnMut(&super::value::MbValue, &super::value::MbValue) -> std::cmp::Ordering>(&mut self, compare: F) {
        self.ensure_generic().sort_by(compare);
    }

    pub fn sort_by_key<K, F: FnMut(&super::value::MbValue) -> K>(&mut self, f: F) where K: Ord {
        self.ensure_generic().sort_by_key(f);
    }

    pub fn set(&mut self, index: usize, val: super::value::MbValue) -> bool {
        if index >= self.len() {
            return false;
        }
        match self {
            Self::Int(v) => {
                if let Some(i) = val.as_int() {
                    v[index] = i;
                } else {
                    let generic_vec = self.ensure_generic();
                    let old_val = std::mem::replace(&mut generic_vec[index], val);
                    release_owned(old_val);
                }
                true
            }
            Self::Float(v) => {
                if let Some(f) = val.as_float() {
                    v[index] = f;
                } else {
                    let generic_vec = self.ensure_generic();
                    let old_val = std::mem::replace(&mut generic_vec[index], val);
                    release_owned(old_val);
                }
                true
            }
            Self::Generic(v) => {
                let old_val = std::mem::replace(&mut v[index], val);
                release_owned(old_val);
                true
            }
        }
    }
}

impl FromIterator<super::value::MbValue> for MbListBuffer {
    fn from_iter<T: IntoIterator<Item = super::value::MbValue>>(iter: T) -> Self {
        MbListBuffer::Generic(iter.into_iter().collect())
    }
}


/// Hash-indexed backing store for `ObjData::Set` (#set-perf).
///
/// Previously a set was a bare `MbList` (a `Vec`), so membership / `add`
/// dedup / `discard` all did a *linear* `eq_py` scan — making `set.add` in
/// a loop O(n²). `MbSet` keeps the ordered `items: MbList` (so iteration,
/// `pop`, GC traversal, repr, and the ~40 read-only consumer sites all keep
/// working through `Deref<Target = MbList>`) and adds a `buckets` hash index
/// that maps a value's `set_hash` to the positions in `items` that hash
/// there. Membership / add / discard become amortized O(1): hash the value,
/// look in its bucket, confirm with `eq_py` (so exact Python `==` semantics,
/// including `1 == 1.0 == True`, are preserved — values that may compare
/// equal across types share a bucket and are disambiguated by `eq_py`).
///
/// Read-only access (`.iter()`, `.len()`, `.to_vec()`, `.is_empty()`, …)
/// goes through `Deref` to `items`. All *mutation* must go through the
/// inherent methods (`set_insert` / `set_remove` / `pop_front`) so the index
/// stays in sync — `MbSet` deliberately does NOT implement `DerefMut`.
#[derive(Default)]
pub struct MbSet {
    items: MbList,
    /// `set_hash(value) -> positions into `items`` with that hash.
    buckets: FxHashMap<u64, SmallVec<[u32; 1]>>,
    version: u64,
}

impl std::ops::Deref for MbSet {
    type Target = MbList;
    #[inline]
    fn deref(&self) -> &MbList {
        &self.items
    }
}

/// Hash that is *consistent with* `mb_eq` (`eq_py`): any two values that
/// compare equal MUST hash the same so they bucket together.
///
/// - int / bool → the integer value (so `True`/`1`/`1.0` share a bucket)
/// - integral float that fits i64 → that integer value
/// - other float → its bit pattern
/// - str → the string contents
/// - tuple → structural `mb_tuple_hash`
/// - frozenset → structural `mb_frozenset_hash`
/// - everything else (custom `__eq__` instances, bytes-like, namedtuples,
///   …) → a single shared bucket (0) so `eq_py` still resolves them
///   correctly. These rarer types fall back to a linear scan *within that
///   one bucket*, exactly matching the old behavior, while the common
///   int/str/tuple element types get true O(1).
pub(crate) fn set_hash(v: super::value::MbValue) -> u64 {
    let hash_val = super::builtins::mb_hash(v);
    if let Some(h) = hash_val.as_int() {
        return h as u64;
    }
    0
}

impl MbSet {
    /// Build a set from `elements` and construct the hash index.
    ///
    /// This preserves `new_set`'s historical contract: every element in
    /// `elements` is stored verbatim and the set takes ownership of the
    /// references the caller already holds (no extra retain, no release of
    /// dropped duplicates). Callers that need set semantics (dedup) pass an
    /// already-deduplicated vector — `union`, `intersection`, `copy`, etc.
    /// all do. Building the index for duplicate-equal elements simply records
    /// multiple positions in the same bucket, which the `eq_py` confirm step
    /// tolerates.
    pub fn from_elements(elements: Vec<super::value::MbValue>) -> Self {
        let mut s = MbSet {
            items: MbList::with_capacity(elements.len()),
            buckets: FxHashMap::default(),
            version: 0,
        };
        for e in elements {
            let pos = s.items.len() as u32;
            s.items.push(e);
            s.buckets.entry(set_hash(e)).or_default().push(pos);
        }
        s
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn bump_version(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    /// Find the position of `value` in `items`, or `None`. O(1) amortized.
    #[inline]
    fn position_of(&self, value: super::value::MbValue) -> Option<usize> {
        let hh = set_hash(value);
        let bucket = self.buckets.get(&hh)?;
        for &pos in bucket.iter() {
            let existing = self.items[pos as usize];
            if super::builtins::mb_eq(existing, value).as_bool() == Some(true) {
                return Some(pos as usize);
            }
        }
        None
    }

    /// `value in set` — O(1) amortized.
    #[inline]
    pub fn contains_value(&self, value: super::value::MbValue) -> bool {
        self.position_of(value).is_some()
    }

    /// Insert `value`, retaining it (one new owned reference) if it is newly
    /// added. Returns true if newly inserted. O(1) amortized.
    #[inline]
    pub fn set_insert(&mut self, value: super::value::MbValue) -> bool {
        let hh = set_hash(value);
        if super::exception::current_exception_type().is_some() {
            return false;
        }
        if self.position_of(value).is_some() {
            return false;
        }
        if super::exception::current_exception_type().is_some() {
            return false;
        }
        self.version = self.version.wrapping_add(1);
        store_owned(value);
        let pos = self.items.len() as u32;
        self.items.push(value);
        self.buckets.entry(hh).or_default().push(pos);
        true
    }

    /// Remove `value` if present, returning the removed `MbValue` (which the
    /// caller is responsible for releasing). O(1) amortized — uses
    /// `swap_remove` and fixes up the moved element's index entry.
    pub fn set_remove(&mut self, value: super::value::MbValue) -> Option<super::value::MbValue> {
        let pos = self.position_of(value)?;
        Some(self.remove_at(pos))
    }

    /// Remove and return the element at `idx` via `swap_remove`, keeping the
    /// hash index consistent (the element previously at the last position is
    /// moved to `idx`, so its bucket entry is repointed).
    fn remove_at(&mut self, idx: usize) -> super::value::MbValue {
        self.version = self.version.wrapping_add(1);
        let last = self.items.len() - 1;
        let removed = self.items[idx];
        // Drop `removed`'s index entry (it hashed to bucket of `removed`).
        self.bucket_remove_pos(set_hash(removed), idx as u32);
        if idx != last {
            // The element at `last` is about to move into slot `idx`.
            let moved = self.items[last];
            self.bucket_replace_pos(set_hash(moved), last as u32, idx as u32);
        }
        self.items.swap_remove(idx)
    }

    /// Remove and return the first element (used by `set.pop`). O(1)
    /// amortized via the same swap-remove bookkeeping.
    pub fn pop_front(&mut self) -> Option<super::value::MbValue> {
        if self.items.is_empty() {
            return None;
        }
        Some(self.remove_at(0))
    }

    fn bucket_remove_pos(&mut self, hash: u64, pos: u32) {
        if let Some(bucket) = self.buckets.get_mut(&hash) {
            if let Some(i) = bucket.iter().position(|&p| p == pos) {
                bucket.swap_remove(i);
            }
            if bucket.is_empty() {
                self.buckets.remove(&hash);
            }
        }
    }

    fn bucket_replace_pos(&mut self, hash: u64, old: u32, new: u32) {
        if let Some(bucket) = self.buckets.get_mut(&hash) {
            for p in bucket.iter_mut() {
                if *p == old {
                    *p = new;
                    break;
                }
            }
        }
    }

    /// Clear all elements and the index (callers release element refs).
    pub fn clear_all(&mut self) {
        self.version = self.version.wrapping_add(1);
        self.items.clear();
        self.buckets.clear();
    }
}

/// Container lock for `ObjData::{List,Dict,Set,ByteArray}` + `Instance.fields`
/// (#2518). Wraps `parking_lot::RwLock` so the uncontended read+write fast
/// path drops from `std::sync::RwLock`'s ~22 ns/pair to ~16 ns/pair on
/// aarch64 — a ~30% win on a probe loop, clearing the ≥20% acceptance gate.
///
/// `read()` / `write()` return `Result<Guard, Infallible>` so the ~994
/// existing `lock.read().unwrap()` / `lock.write().unwrap()` callsites
/// across the runtime compile unchanged. `try_read()` / `try_write()`
/// return `Result<Guard, ()>` to match the std `TryLockResult` pattern
/// used by `dict_ops.rs` / `list_ops.rs`'s `match lock.try_read() { Ok(g)
/// => g, Err(_) => lock.read().unwrap() }` fallback.
///
/// parking_lot does not poison on panic, so the `Err` arm of `read()` /
/// `write()` is statically unreachable — `Infallible` makes that explicit.
pub struct MbRwLock<T: ?Sized>(parking_lot::RwLock<T>);

impl<T> MbRwLock<T> {
    #[inline]
    pub fn new(val: T) -> Self {
        Self(parking_lot::RwLock::new(val))
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.0.into_inner()
    }
}

impl<T: ?Sized> MbRwLock<T> {
    #[inline]
    pub fn read(&self) -> Result<parking_lot::RwLockReadGuard<'_, T>, std::convert::Infallible> {
        Ok(self.0.read())
    }

    #[inline]
    pub fn write(&self) -> Result<parking_lot::RwLockWriteGuard<'_, T>, std::convert::Infallible> {
        Ok(self.0.write())
    }

    #[inline]
    pub fn try_read(&self) -> Result<parking_lot::RwLockReadGuard<'_, T>, ()> {
        self.0.try_read().ok_or(())
    }

    #[inline]
    pub fn try_write(&self) -> Result<parking_lot::RwLockWriteGuard<'_, T>, ()> {
        self.0.try_write().ok_or(())
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }
}

impl<T: Default> Default for MbRwLock<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// Backing map for `ObjData::Instance.fields`.
///
/// Uses `FxHash` instead of the std `SipHash13` default — Instance attribute
/// names are short ASCII identifiers, do not need DoS resistance, and SipHash13
/// dominated the per-call cost on subset-B fixtures (urllib.parse, http,
/// mimetypes; #2381 / #2096 layer-2). FxHashMap keeps the same API surface as
/// `HashMap<String, MbValue>` so `.read()` / `.write()` / `.insert()` / `.get()`
/// callsites stay byte-identical.
pub type InstanceFields = FxHashMap<String, super::value::MbValue>;

/// Backing map for `ObjData::Dict`.
///
/// Runtime dict keys are attacker-controlled Python values, so the storage
/// table keeps `IndexMap`'s default seeded `RandomState` (SipHash-family)
/// rather than `FxHash`. Dict-key equality still follows Python semantics in
/// `DictKey`; this hasher choice is only about protecting the table from
/// collision-flooding while preserving insertion order.
pub type MbDictMap = IndexMap<DictKey, super::value::MbValue>;

/// Cycle-capable test — returns true if `v` is a heap pointer to a kind that
/// can participate in a reference cycle (containers + closures + class
/// instances). Used by immutable-container allocators (`new_tuple`,
/// `new_frozenset`) to elide `gc_track` when contents are exclusively atomic
/// (CPython's "untracked tuple" optimization). Strs/bytes/numerics can't form
/// cycles so they're skipped here.
fn value_is_cycle_capable(v: super::value::MbValue) -> bool {
    let Some(ptr) = v.as_ptr() else {
        return false;
    };
    if ptr.is_null() {
        return false;
    }
    let kind = unsafe { (*ptr).header.kind };
    matches!(
        kind,
        ObjKind::List
            | ObjKind::Dict
            | ObjKind::Tuple
            | ObjKind::Set
            | ObjKind::FrozenSet
            | ObjKind::Instance
            | ObjKind::Function
            | ObjKind::Class
    )
}

/// Type tag for heap objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ObjKind {
    Str = 0,
    List = 1,
    Dict = 2,
    Tuple = 3,
    Function = 4,
    Class = 5,
    Instance = 6,
    Set = 7,
    Bytes = 8,
    ByteArray = 9,
    FrozenSet = 10,
    /// Arbitrary-precision integer (BigInt fallback, R3/#833).
    BigInt = 11,
    /// Complex number — real + imaginary f64 pair (R3 CPython 3.12 conformance).
    Complex = 12,
    // @spec .aw/changes/mamba-compile-builtin/groups/default/specs/mamba-compile-builtin-value-rc.md#R1
    /// Code object produced by compile() (#976).
    CodeObject = 13,
    // HANDWRITE-BEGIN reason: #2182 — Generator runtime type missing; blocks lazy
    // stdlib iterator semantics (itertools.chain/islice/count/cycle/...,
    // csv.reader, glob.iglob, re.finditer, xml.etree.iter, os.walk).
    // Closing #2182 lands a true `Generator = 14` kind backed by either a
    // closure-state pair (`next_fn: fn(*mut Self) -> Option<MbValue>` + boxed
    // state) or the existing low-level coroutine infrastructure in
    // `apps/mamba/src/runtime/generator.rs` (already shipped for
    // user-defined `def f(): yield`). Until then every "iterator" surface
    // above ships as `ObjKind::List` (eager materialization), which is
    // behaviourally CPython-compatible for finite inputs but breaks
    // infinite generators (`itertools.count`, `itertools.cycle`), breaks
    // `next()/send()` step semantics, and defeats the memory-axis
    // subset-B wins tracked by #2096. Replace this carve-out with the
    // actual variant + supporting `MbObject::new_generator(...)` ctor
    // when the Phase-1 retrofit (csv.reader as worked example) lands.
    // HANDWRITE-END
}

/// Object header — prefixes every heap-allocated object.
/// Uses atomic refcount for thread-safe retain/release.
#[repr(C)]
pub struct MbObjectHeader {
    pub rc: AtomicU32,
    pub kind: ObjKind,
}

/// A heap-allocated Mamba object.
#[repr(C)]
pub struct MbObject {
    pub header: MbObjectHeader,
    pub data: ObjData,
}

// Safety: MbObject is safe to send/share across threads because:
// - rc is AtomicU32
// - Mutable collections are wrapped in RwLock
// - Immutable data (Str, Tuple, Bytes, FrozenSet) is inherently safe
unsafe impl Send for MbObject {}
unsafe impl Sync for MbObject {}

/// Union of possible object data.
/// Mutable collections are wrapped in RwLock for thread-safe access.
pub enum ObjData {
    Str(String),
    List(MbRwLock<MbListBuffer>),
    Dict(MbRwLock<MbDictMap>),
    Tuple(Vec<super::value::MbValue>),
    Instance {
        class_name: String,
        fields: MbRwLock<InstanceFields>,
    },
    Set(MbRwLock<MbSet>),
    Bytes(Vec<u8>),
    ByteArray(MbRwLock<Vec<u8>>),
    FrozenSet(Vec<super::value::MbValue>),
    /// Arbitrary-precision integer heap object (BigInt fallback, R3/#833).
    BigInt(BigInt),
    /// Complex number: (real, imag) as f64 pair (R3 CPython 3.12 conformance).
    Complex(f64, f64),
    // @spec .aw/changes/mamba-compile-builtin/groups/default/specs/mamba-compile-builtin-value-rc.md#R2
    /// Code object produced by compile(source, filename, mode) (#976).
    ///
    /// Stores the parsed AST (`Module`), original source text, filename for
    /// diagnostic threading, and compilation mode.
    CodeObject {
        source: String,
        filename: String,
        mode: String,
        ast: Box<crate::parser::ast::Module>,
    },
    // HANDWRITE-BEGIN reason: #2182 — Generator(GeneratorState) variant placeholder.
    // The companion to `ObjKind::Generator` above. Will hold either a
    // `Box<dyn FnMut() -> Option<MbValue>>` (option A — closure-state) or
    // a handle into `runtime::generator::GenEntry` (option C — reuse the
    // existing coroutine engine that already powers user-defined
    // `def f(): yield`). Whichever lands, `class.rs::mb_iter_next` and
    // the `for x in <iterable>` lowering must learn to dispatch on this
    // variant before the stdlib retrofit can proceed. Until then this
    // sibling is a doc-only carve-out — no enum entry, no field — so the
    // ObjData union stays exhaustive against `ObjKind`.
    // HANDWRITE-END
}

/// Immortal refcount sentinel — objects with this value are never freed.
/// Used for compile-time string/bytes constants embedded in JIT code.
pub const IMMORTAL_REFCOUNT: u32 = u32::MAX;

fn atomic_rc(val: u32) -> AtomicU32 {
    AtomicU32::new(val)
}

impl MbObject {
    pub fn new_str(s: String) -> *mut Self {
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::Str,
            },
            data: ObjData::Str(s),
        });
        into_raw_tracked(obj)
    }

    /// Allocate an immortal string — rc is set to IMMORTAL_REFCOUNT so
    /// `mb_retain`/`mb_release` are no-ops. Used for compile-time constants.
    pub fn new_str_immortal(s: String) -> *mut Self {
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(IMMORTAL_REFCOUNT),
                kind: ObjKind::Str,
            },
            data: ObjData::Str(s),
        });
        into_raw_tracked(obj)
    }

    /// Create a new list, taking ownership of elements (no retain).
    /// Use `new_list_borrowed` when elements are borrowed from another container.
    ///
    /// Converts the input `Vec<MbValue>` to an inline `MbList` (SmallVec
    /// inline-8) or unboxed scalar buffer at the boundary. #2517 / #1075.
    fn new_list_with_tracking(elements: Vec<super::value::MbValue>, track_gc: bool) -> *mut Self {
        let buf: MbListBuffer = if elements.is_empty() {
            MbListBuffer::Generic(SmallVec::new())
        } else if elements.iter().all(|v| v.as_int().is_some()) {
            MbListBuffer::Int(elements.iter().map(|v| v.as_int().unwrap()).collect())
        } else if elements.iter().all(|v| v.as_float().is_some()) {
            MbListBuffer::Float(elements.iter().map(|v| v.as_float().unwrap()).collect())
        } else {
            let small: MbList = if elements.len() <= 8 {
                MbList::from_slice(&elements)
            } else {
                MbList::from_vec(elements)
            };
            MbListBuffer::Generic(small)
        };
        Self::new_list_buffer_with_tracking(buf, track_gc)
    }

    pub fn new_list(elements: Vec<super::value::MbValue>) -> *mut Self {
        Self::new_list_with_tracking(elements, true)
    }

    pub fn new_list_untracked(elements: Vec<super::value::MbValue>) -> *mut Self {
        Self::new_list_with_tracking(elements, false)
    }

    fn new_list_buffer_with_tracking(buf: MbListBuffer, track_gc: bool) -> *mut Self {
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::List,
            },
            data: ObjData::List(MbRwLock::new(buf)),
        });
        let ptr = into_raw_tracked(obj);
        if track_gc {
            super::gc::gc_track(ptr);
        }
        ptr
    }

    fn new_list_inline_with_tracking(buf: MbList, track_gc: bool) -> *mut Self {
        Self::new_list_buffer_with_tracking(MbListBuffer::Generic(buf), track_gc)
    }

    pub fn new_list_inline(buf: MbList) -> *mut Self {
        Self::new_list_inline_with_tracking(buf, true)
    }

    pub fn new_list_inline_untracked(buf: MbList) -> *mut Self {
        Self::new_list_inline_with_tracking(buf, false)
    }

    pub fn new_int_list_inline(buf: SmallVec<[i64; 8]>) -> *mut Self {
        Self::new_list_buffer_with_tracking(MbListBuffer::Int(buf), true)
    }

    pub fn new_int_list_inline_untracked(buf: SmallVec<[i64; 8]>) -> *mut Self {
        Self::new_list_buffer_with_tracking(MbListBuffer::Int(buf), false)
    }

    pub fn new_float_list_inline(buf: SmallVec<[f64; 8]>) -> *mut Self {
        Self::new_list_buffer_with_tracking(MbListBuffer::Float(buf), true)
    }

    pub fn new_float_list_inline_untracked(buf: SmallVec<[f64; 8]>) -> *mut Self {
        Self::new_list_buffer_with_tracking(MbListBuffer::Float(buf), false)
    }

    /// Create a new list, retaining all pointer elements.
    /// Use when elements are borrowed from another container (e.g., cloned Vec from a list).
    /// `release_contained_values` will release them when this list is freed.
    pub fn new_list_borrowed(elements: Vec<super::value::MbValue>) -> *mut Self {
        unsafe {
            for &item in &elements {
                super::rc::retain_if_ptr(item);
            }
        }
        Self::new_list(elements)
    }

    fn new_dict_with_tracking(track_gc: bool) -> *mut Self {
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::Dict,
            },
            data: ObjData::Dict(MbRwLock::new(MbDictMap::default())),
        });
        let ptr = into_raw_tracked(obj);
        if track_gc {
            super::gc::gc_track(ptr);
        }
        ptr
    }

    pub fn new_dict() -> *mut Self {
        Self::new_dict_with_tracking(true)
    }

    pub fn new_dict_untracked() -> *mut Self {
        Self::new_dict_with_tracking(false)
    }

    /// Create a dict with pre-allocated capacity.
    /// Used when the number of entries is known or estimated.
    pub fn new_dict_with_capacity(capacity: usize) -> *mut Self {
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::Dict,
            },
            data: ObjData::Dict(MbRwLock::new(MbDictMap::with_capacity(capacity))),
        });
        let ptr = into_raw_tracked(obj);
        super::gc::gc_track(ptr);
        ptr
    }

    pub fn new_tuple(elements: Vec<super::value::MbValue>) -> *mut Self {
        let needs_tracking = elements.iter().any(|v| value_is_cycle_capable(*v));
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::Tuple,
            },
            data: ObjData::Tuple(elements),
        });
        let ptr = into_raw_tracked(obj);
        if needs_tracking {
            super::gc::gc_track(ptr);
        }
        ptr
    }

    /// Create a new tuple, retaining all pointer elements (borrowed from another container).
    pub fn new_tuple_borrowed(elements: Vec<super::value::MbValue>) -> *mut Self {
        unsafe {
            for &item in &elements {
                super::rc::retain_if_ptr(item);
            }
        }
        Self::new_tuple(elements)
    }

    pub fn new_set(elements: Vec<super::value::MbValue>) -> *mut Self {
        let set = MbSet::from_elements(elements);
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::Set,
            },
            data: ObjData::Set(MbRwLock::new(set)),
        });
        let ptr = into_raw_tracked(obj);
        super::gc::gc_track(ptr);
        ptr
    }

    /// Create a new set, retaining all pointer elements (borrowed from another container).
    pub fn new_set_borrowed(elements: Vec<super::value::MbValue>) -> *mut Self {
        unsafe {
            for &item in &elements {
                super::rc::retain_if_ptr(item);
            }
        }
        Self::new_set(elements)
    }

    // Carve-out for #2107 (bytes-object materialization hot path —
    // speed-axis sibling of #2096 memory-allocator overhead).
    //
    // Per #2107's post-#2108 reframe, this constructor itself is already
    // optimal: it is a pure `Box::new` + move (no extra memcpy, no
    // allocator round-trip beyond what `Box::new` already costs). The
    // residual cross-cutting cost lives at shim sites that do
    //
    //     let mut buf = Vec::with_capacity(N);
    //     buf.extend_from_slice(src);
    //     MbObject::new_bytes(buf)
    //
    // where the `extend_from_slice` adds a full memcpy on top of
    // whatever the source already held. Compression shims
    // (`zlib_mod`, `gzip_mod`, `lzma_mod`, `bz2_mod`) already avoid
    // that pattern by passing `Vec::with_capacity(...)` directly into
    // the encoder and moving the encoder-owned `Vec` straight into
    // `new_bytes` — no audit action needed there.
    //
    // Re-open #2107 only when a workload dominated by repeated
    // bytes round-trips (large `bytes(buf)` allocate-drop cycles,
    // `io.BytesIO`-heavy pipelines, codec encode/decode chains)
    // surfaces in a bench. Until then this hot path is intentionally
    // unchanged; see the `bytes_materialization_hot_path_carveout`
    // doc-test below for the regression marker.
    pub fn new_bytes(mut data: Vec<u8>) -> *mut Self {
        // #2096 mitigation: bytes are immutable post-construction, so any
        // capacity slack (extend_from_slice / Vec doubling / encoder
        // over-allocate) is dead weight for the lifetime of the object.
        // The cross-runtime memory bench for `base64/encode_decode`
        // measured mamba at 0.50× CPython (2× more RSS); a portion of
        // that comes from Vec capacity > len on the bytes returned by
        // `b64encode`/`b64decode` (each calls Vec::with_capacity then
        // pushes/extends, which can round up). Trim once here so the
        // object's lifetime memory footprint matches its payload.
        if data.capacity() > data.len() {
            data.shrink_to_fit();
        }
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::Bytes,
            },
            data: ObjData::Bytes(data),
        });
        into_raw_tracked(obj)
    }

    /// Allocate immortal bytes — rc is set to IMMORTAL_REFCOUNT so
    /// `mb_retain`/`mb_release` are no-ops. Used for compile-time constants.
    pub fn new_bytes_immortal(mut data: Vec<u8>) -> *mut Self {
        if data.capacity() > data.len() {
            data.shrink_to_fit();
        }
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(IMMORTAL_REFCOUNT),
                kind: ObjKind::Bytes,
            },
            data: ObjData::Bytes(data),
        });
        into_raw_tracked(obj)
    }

    pub fn new_bytearray(data: Vec<u8>) -> *mut Self {
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::ByteArray,
            },
            data: ObjData::ByteArray(MbRwLock::new(data)),
        });
        into_raw_tracked(obj)
    }

    pub fn new_frozenset(elements: Vec<super::value::MbValue>) -> *mut Self {
        let needs_tracking = elements.iter().any(|v| value_is_cycle_capable(*v));
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::FrozenSet,
            },
            data: ObjData::FrozenSet(elements),
        });
        let ptr = into_raw_tracked(obj);
        if needs_tracking {
            super::gc::gc_track(ptr);
        }
        ptr
    }

    /// Allocate a BigInt heap object (#833).
    pub fn new_bigint(value: BigInt) -> *mut Self {
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::BigInt,
            },
            data: ObjData::BigInt(value),
        });
        into_raw_tracked(obj)
    }

    /// Allocate an immortal BigInt for compile-time integer constants.
    pub fn new_bigint_immortal(value: BigInt) -> *mut Self {
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(IMMORTAL_REFCOUNT),
                kind: ObjKind::BigInt,
            },
            data: ObjData::BigInt(value),
        });
        into_raw_tracked(obj)
    }

    /// Allocate a Complex heap object (R3 CPython 3.12 conformance).
    pub fn new_complex(real: f64, imag: f64) -> *mut Self {
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::Complex,
            },
            data: ObjData::Complex(real, imag),
        });
        into_raw_tracked(obj)
    }

    /// Allocate a CodeObject heap object produced by compile() (#976).
    ///
    /// Not GC-tracked — CodeObject is immutable (like BigInt/Complex) and
    /// cannot participate in reference cycles.
    // @spec .aw/changes/mamba-compile-builtin/groups/default/specs/mamba-compile-builtin-value-rc.md#R3
    pub fn new_code_object(
        source: String,
        filename: String,
        mode: String,
        ast: crate::parser::ast::Module,
    ) -> *mut Self {
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::CodeObject,
            },
            data: ObjData::CodeObject {
                source,
                filename,
                mode,
                ast: Box::new(ast),
            },
        });
        into_raw_tracked(obj)
    }

    pub fn new_instance(class_name: String) -> *mut Self {
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::Instance,
            },
            data: ObjData::Instance {
                class_name,
                fields: MbRwLock::new(InstanceFields::default()),
            },
        });
        let ptr = into_raw_tracked(obj);
        super::gc::gc_track(ptr);
        ptr
    }

    /// Create an instance with pre-allocated field capacity.
    /// Used when the number of __init__ params is known, avoiding HashMap resizing
    /// during field assignment in __init__.
    pub fn new_instance_with_capacity(class_name: String, capacity: usize) -> *mut Self {
        let obj = Box::new(MbObject {
            header: MbObjectHeader {
                rc: atomic_rc(1),
                kind: ObjKind::Instance,
            },
            data: ObjData::Instance {
                class_name,
                fields: MbRwLock::new(InstanceFields::with_capacity_and_hasher(
                    capacity,
                    Default::default(),
                )),
            },
        });
        let ptr = into_raw_tracked(obj);
        super::gc::gc_track(ptr);
        ptr
    }
}

// #2585 E2: explicit arming control for the release-path UAF/dangling-
// pointer detector (`debug_validate_obj`, A1). Monotonic (never disarms
// once set) so there is no ordering hazard between the lazy env-var read
// and `force_arm_uaf_detector()`. Two knobs, both gate the *same* flag:
//   - env var `MAMBA_UAF_DETECTOR=1` (any non-empty, non-"0" value):
//     arms for the whole process — the dedicated detector run.
//   - `force_arm_uaf_detector()`: arms in-process — used by the A2
//     positive control so it trips regardless of the env var.
// Default (neither set) is DISARMED: `debug_validate_obj` is a no-op,
// reproducing the pre-#2539 code path exactly (the regression anchor,
// AC5a, `13,765 passed; 0 failed; 2 ignored`). Armed is AC5b: the
// dedicated detector run.
#[cfg(debug_assertions)]
static UAF_DETECTOR_ARMED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
#[cfg(debug_assertions)]
static UAF_DETECTOR_ENV_CHECKED: std::sync::Once = std::sync::Once::new();

#[cfg(debug_assertions)]
fn parse_uaf_detector_env(val: Option<&str>) -> bool {
    match val {
        Some(v) => v != "0" && !v.is_empty(),
        None => false,
    }
}

#[cfg(debug_assertions)]
fn uaf_detector_armed() -> bool {
    UAF_DETECTOR_ENV_CHECKED.call_once(|| {
        let env_val = std::env::var("MAMBA_UAF_DETECTOR").ok();
        if parse_uaf_detector_env(env_val.as_deref()) {
            UAF_DETECTOR_ARMED.store(true, Ordering::Relaxed);
        }
    });
    UAF_DETECTOR_ARMED.load(Ordering::Relaxed)
}

/// Force-arm the UAF/dangling-pointer detector in-process, independent of
/// `MAMBA_UAF_DETECTOR`. Monotonic — never disarms. Used by the A2
/// positive control (`test_uaf_detector_trips_on_misaligned_pointer`) so
/// it keeps tripping even when the regression anchor runs disarmed by
/// default (#2585 E2).
#[cfg(debug_assertions)]
pub fn force_arm_uaf_detector() {
    UAF_DETECTOR_ARMED.store(true, Ordering::Relaxed);
}

// #2585 E2: attribution breadcrumb. The JIT executes on a *spawned* worker
// thread (see `tests/cpython_ported/harness.rs::jit_capture_with_exception`)
// that does not inherit the outer test thread's name, so a violation
// previously panicked as `thread '<unnamed>' panicked at ...` with no way
// to attribute it to a test. `JIT_LOCK` serializes JIT execution to at
// most one in flight per process, so a single process-global (not
// thread-local) "currently executing test" string is sufficient — no
// per-thread bookkeeping needed. Best-effort only: an unset/poisoned name
// degrades to "<unknown test>" rather than panicking-while-panicking.
#[cfg(debug_assertions)]
static CURRENT_TEST_NAME: std::sync::RwLock<String> = std::sync::RwLock::new(String::new());

#[cfg(debug_assertions)]
pub fn set_current_test_name(name: &str) {
    if let Ok(mut guard) = CURRENT_TEST_NAME.write() {
        guard.clear();
        guard.push_str(name);
    }
}

#[cfg(debug_assertions)]
fn current_test_name_for_diagnostics() -> String {
    match CURRENT_TEST_NAME.try_read() {
        Ok(guard) if !guard.is_empty() => guard.clone(),
        Ok(_) => "<unknown test>".to_string(),
        Err(_) => "<test-name lock unavailable>".to_string(),
    }
}

/// Debug: validate an MbObject pointer looks reasonable before dereferencing.
/// Checks pointer alignment and that the kind field is a valid ObjKind variant.
/// No-op unless the detector is armed (see `uaf_detector_armed`, #2585 E2).
#[cfg(debug_assertions)]
unsafe fn debug_validate_obj(obj: *mut MbObject, caller: &str) {
    if !uaf_detector_armed() {
        return;
    }
    let addr = obj as usize;
    // Check alignment (MbObject should be at least 4-byte aligned for AtomicU32)
    if addr % 4 != 0 {
        panic!(
            "{caller}: misaligned MbObject pointer {obj:?} (addr={addr:#x}) \
             [test={}]",
            current_test_name_for_diagnostics()
        );
    }
    // Check that kind field is a valid ObjKind variant (0..=13)
    let kind_byte = (*obj).header.kind as u8;
    if kind_byte > 13 {
        panic!(
            "{caller}: invalid ObjKind={kind_byte} at {obj:?} — likely use-after-free \
             or dangling pointer. rc={:#x} [test={}]",
            (*obj).header.rc.load(Ordering::Relaxed),
            current_test_name_for_diagnostics()
        );
    }
}

// =========================================================================
// #2830: Debug-only allocation & final-deallocation leak balance recorder
// =========================================================================

#[cfg(debug_assertions)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LeakBalanceStatus {
    Inactive,
    Active,
    Finished,
    Incomplete,
}

#[cfg(debug_assertions)]
impl Default for LeakBalanceStatus {
    fn default() -> Self { LeakBalanceStatus::Inactive }
}

#[cfg(debug_assertions)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LeakBalanceSnapshot {
    name: &'static str,
    allocations: usize,
    deallocations: usize,
    status: LeakBalanceStatus,
}

#[cfg(debug_assertions)]
impl LeakBalanceSnapshot {
    pub fn name(&self) -> &'static str { self.name }
    pub fn allocations(&self) -> usize { self.allocations }
    pub fn deallocations(&self) -> usize { self.deallocations }
    pub fn status(&self) -> LeakBalanceStatus { self.status }
    pub fn delta(&self) -> isize { self.allocations as isize - self.deallocations as isize }
    pub fn is_balanced(&self) -> bool {
        (self.status == LeakBalanceStatus::Active || self.status == LeakBalanceStatus::Finished)
            && self.allocations == self.deallocations
    }
}

#[cfg(debug_assertions)]
#[derive(Debug, Default)]
struct LeakBalanceState {
    name: &'static str,
    status: LeakBalanceStatus,
    allocations: usize,
    deallocations: usize,
}

#[cfg(debug_assertions)]
std::thread_local! {
    static LEAK_BALANCE_STATE: std::cell::RefCell<LeakBalanceState> =
        std::cell::RefCell::new(LeakBalanceState::default());
}

#[cfg(debug_assertions)]
fn record_allocation_event() {
    LEAK_BALANCE_STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.status == LeakBalanceStatus::Active {
            s.allocations += 1;
        }
    });
}

#[cfg(debug_assertions)]
fn record_deallocation_event() {
    LEAK_BALANCE_STATE.with(|st| {
        let mut s = st.borrow_mut();
        if s.status == LeakBalanceStatus::Active {
            s.deallocations += 1;
        }
    });
}

#[cfg(debug_assertions)]
#[inline]
fn into_raw_tracked(obj: Box<MbObject>) -> *mut MbObject {
    record_allocation_event();
    Box::into_raw(obj)
}

#[cfg(not(debug_assertions))]
#[inline(always)]
fn into_raw_tracked(obj: Box<MbObject>) -> *mut MbObject {
    Box::into_raw(obj)
}

#[cfg(debug_assertions)]
#[derive(Debug)]
pub(crate) struct LeakBalanceScopeGuard {
    finished: bool,
    _phantom: std::marker::PhantomData<*const ()>,
}

#[cfg(debug_assertions)]
impl Drop for LeakBalanceScopeGuard {
    fn drop(&mut self) {
        if !self.finished {
            LEAK_BALANCE_STATE.with(|st| {
                let mut s = st.borrow_mut();
                if s.status == LeakBalanceStatus::Active {
                    s.status = LeakBalanceStatus::Incomplete;
                }
            });
        }
    }
}

#[cfg(debug_assertions)]
impl LeakBalanceScopeGuard {
    pub fn finish(mut self) -> Result<LeakBalanceSnapshot, String> {
        let snap = finish_leak_balance_scope()?;
        self.finished = true;
        Ok(snap)
    }
}

#[cfg(debug_assertions)]
pub(crate) fn start_leak_balance_scope(name: &'static str) -> Result<LeakBalanceScopeGuard, String> {
    LEAK_BALANCE_STATE.with(|st| {
        let mut s = st.borrow_mut();
        match s.status {
            LeakBalanceStatus::Active => Err(format!(
                "cannot start leak balance scope '{name}': scope '{}' is already active",
                s.name
            )),
            LeakBalanceStatus::Incomplete => Err(format!(
                "cannot start leak balance scope '{name}': previous scope '{}' is incomplete",
                s.name
            )),
            LeakBalanceStatus::Inactive | LeakBalanceStatus::Finished => {
                s.name = name;
                s.status = LeakBalanceStatus::Active;
                s.allocations = 0;
                s.deallocations = 0;
                Ok(LeakBalanceScopeGuard {
                    finished: false,
                    _phantom: std::marker::PhantomData,
                })
            }
        }
    })
}

#[cfg(debug_assertions)]
pub(crate) fn finish_leak_balance_scope() -> Result<LeakBalanceSnapshot, String> {
    LEAK_BALANCE_STATE.with(|st| {
        let mut s = st.borrow_mut();
        match s.status {
            LeakBalanceStatus::Inactive => Err("leak balance scope is inactive".to_string()),
            LeakBalanceStatus::Incomplete => Err(format!(
                "leak balance scope '{}' is incomplete (allocations={}, deallocations={})",
                s.name, s.allocations, s.deallocations
            )),
            LeakBalanceStatus::Finished => Err(format!(
                "leak balance scope '{}' is already finished",
                s.name
            )),
            LeakBalanceStatus::Active => {
                let snap = LeakBalanceSnapshot {
                    name: s.name,
                    allocations: s.allocations,
                    deallocations: s.deallocations,
                    status: LeakBalanceStatus::Active,
                };
                if !snap.is_balanced() {
                    Err(format!(
                        "leak balance mismatch for scope '{}': allocations={}, deallocations={}, delta={}",
                        s.name, s.allocations, s.deallocations, snap.delta()
                    ))
                } else {
                    s.status = LeakBalanceStatus::Finished;
                    Ok(LeakBalanceSnapshot {
                        status: LeakBalanceStatus::Finished,
                        ..snap
                    })
                }
            }
        }
    })
}

#[cfg(debug_assertions)]
pub(crate) fn get_leak_balance_snapshot() -> Result<LeakBalanceSnapshot, String> {
    LEAK_BALANCE_STATE.with(|st| {
        let s = st.borrow();
        match s.status {
            LeakBalanceStatus::Inactive => Err("leak balance scope is inactive".to_string()),
            LeakBalanceStatus::Incomplete => Err(format!(
                "leak balance scope '{}' is incomplete (allocations={}, deallocations={})",
                s.name, s.allocations, s.deallocations
            )),
            LeakBalanceStatus::Finished | LeakBalanceStatus::Active => Ok(LeakBalanceSnapshot {
                name: s.name,
                allocations: s.allocations,
                deallocations: s.deallocations,
                status: s.status,
            }),
        }
    })
}

#[cfg(debug_assertions)]
pub(crate) fn assert_leak_balanced() -> Result<LeakBalanceSnapshot, String> {
    let snap = match get_leak_balance_snapshot() {
        Ok(s) => s,
        Err(e) => panic!("{e}"),
    };
    if !snap.is_balanced() {
        panic!(
            "leak balance mismatch for scope '{}': allocations={}, deallocations={}, delta={}",
            snap.name, snap.allocations, snap.deallocations, snap.delta()
        );
    }
    Ok(snap)
}

#[cfg(all(debug_assertions, test))]
pub(crate) fn reset_leak_balance_scope_for_testing() {
    LEAK_BALANCE_STATE.with(|st| {
        let mut s = st.borrow_mut();
        s.status = LeakBalanceStatus::Inactive;
        s.allocations = 0;
        s.deallocations = 0;
    });
}


/// Increment the reference count atomically.
///
/// # Safety
/// `obj` must be a valid pointer returned by `MbObject::new_*`.
pub unsafe fn mb_retain(obj: *mut MbObject) {
    if !obj.is_null() {
        #[cfg(debug_assertions)]
        debug_validate_obj(obj, "mb_retain");
        // Immortal objects must never have their refcount modified.
        if (*obj).header.rc.load(Ordering::Relaxed) == IMMORTAL_REFCOUNT {
            return;
        }
        (*obj).header.rc.fetch_add(1, Ordering::Relaxed);
    }
}

/// Decrement the reference count atomically. Frees the object when it
/// reaches zero.
///
/// # Safety
/// `obj` must be a valid pointer returned by `MbObject::new_*`.
pub unsafe fn mb_release(obj: *mut MbObject) {
    if obj.is_null() {
        return;
    }
    #[cfg(debug_assertions)]
    debug_validate_obj(obj, "mb_release");
    // Immortal objects must never be freed.
    if (*obj).header.rc.load(Ordering::Relaxed) == IMMORTAL_REFCOUNT {
        return;
    }
    // Use AcqRel: Release on the decrement so prior writes are visible,
    // Acquire on the load so we see all prior writes before freeing.
    if (*obj).header.rc.fetch_sub(1, Ordering::Release) == 1 {
        std::sync::atomic::fence(Ordering::Acquire);
        crate::runtime::stdlib::weakref_mod::notify_referent_collected(obj);
        // #2096 subset A — eager free Bytes on rc=0 (Task #57).
        // Bytes has no contained MbValue pointers (verified in
        // release_contained_values' `_ => {}` arm) and is never inserted
        // into the GC tracked set (new_bytes doesn't call gc_track), so
        // skip the immortal-store re-entrancy guard, gc_untrack lookup,
        // and contained-values walk. Drop the Box (which frees the
        // Vec<u8> buffer) directly.
        if matches!((*obj).header.kind, ObjKind::Bytes) {
            #[cfg(debug_assertions)]
            record_deallocation_event();
            drop(Box::from_raw(obj));
            return;
        }
        // Mark as immortal BEFORE releasing contained values to prevent
        // re-entrant mb_release from cyclic references (A→B→A).
        (*obj).header.rc.store(IMMORTAL_REFCOUNT, Ordering::Relaxed);
        super::gc::gc_untrack(obj);
        release_contained_values(obj);
        #[cfg(debug_assertions)]
        record_deallocation_event();
        drop(Box::from_raw(obj));
    }
}

/// Release all MbValues contained in a heap object.
/// Called before freeing the object to cascade reference decrement.
/// Mirrors CPython's tp_dealloc calling Py_DECREF on contained refs.
/// Public entry point for GC sweep to release contained values.
pub unsafe fn release_contained_values_pub(obj: *mut MbObject) {
    release_contained_values(obj);
}

unsafe fn release_contained_values(obj: *mut MbObject) {
    match &(*obj).data {
        ObjData::List(lock) => {
            let buf = lock.read().unwrap();
            if let MbListBuffer::Generic(ref items) = *buf {
                for item in items.iter() {
                    release_if_ptr(*item);
                }
            }
        }
        ObjData::Dict(lock) => {
            let map = lock.read().unwrap();
            for val in map.values() {
                release_if_ptr(*val);
            }
        }
        ObjData::Tuple(items) => {
            for item in items {
                release_if_ptr(*item);
            }
        }
        ObjData::Set(lock) => {
            let items = lock.read().unwrap();
            for item in items.iter() {
                release_if_ptr(*item);
            }
        }
        ObjData::FrozenSet(items) => {
            for item in items {
                release_if_ptr(*item);
            }
        }
        ObjData::Instance { fields, .. } => {
            let map = fields.read().unwrap();
            for val in map.values() {
                release_if_ptr(*val);
            }
        }
        // Str, Bytes, ByteArray, BigInt, Complex — no contained MbValues
        _ => {}
    }
}

/// Release a runtime-owned value stored by a container/global slot.
///
/// Historical name: this also releases small closure handles and registered
/// positive integer handles (Decimal, Fraction, and other handle-backed
/// stdlib objects).
#[inline]
pub unsafe fn release_if_ptr(val: super::value::MbValue) {
    if is_typed_native_wrapper(val) {
        return;
    }
    if let Some(ptr) = val.as_ptr() {
        #[cfg(debug_assertions)]
        {
            // Validate before releasing to avoid panicking inside an extern "C"
            // call stack (which would abort the process).
            let kind_byte = (*ptr).header.kind as u8;
            if kind_byte > 13 {
                return; // skip invalid pointer — likely UAF
            }
        }
        mb_release(ptr);
        return;
    }
    if super::closure::release_closure_handle_if_live(val) {
        return;
    }
    if let Some(id) = val.as_int() {
        if id > 0 {
            super::integer_handle_registry::release(id as u64);
        }
    }
}

/// Retain a runtime-owned value for container/global storage.
///
/// Historical name: this also retains small closure handles and registered
/// positive integer handles (Decimal, Fraction, and other handle-backed
/// stdlib objects).
#[inline]
pub unsafe fn retain_if_ptr(val: super::value::MbValue) {
    if is_typed_native_wrapper(val) {
        return;
    }
    if let Some(ptr) = val.as_ptr() {
        mb_retain(ptr);
        return;
    }
    if super::closure::retain_closure_handle_if_live(val) {
        return;
    }
    if let Some(id) = val.as_int() {
        if id > 0 {
            super::integer_handle_registry::retain(id as u64);
        }
    }
}

#[cfg(debug_assertions)]
fn debug_heap_refcount(val: super::value::MbValue) -> Option<u32> {
    if is_typed_native_wrapper(val) {
        return None;
    }
    let ptr = val.as_ptr()?;
    if ptr.is_null() {
        return None;
    }
    let rc = unsafe { (*ptr).header.rc.load(Ordering::Relaxed) };
    if rc == IMMORTAL_REFCOUNT {
        None
    } else {
        Some(rc)
    }
}

#[cfg(debug_assertions)]
fn debug_assert_refcount_delta(
    value: super::value::MbValue,
    before: Option<u32>,
    expected_delta: u32,
    caller: &str,
) {
    let Some(before) = before else {
        return;
    };
    let Some(ptr) = value.as_ptr() else {
        return;
    };
    let after = unsafe { (*ptr).header.rc.load(Ordering::Relaxed) };
    debug_assert_eq!(
        after,
        before.saturating_add(expected_delta),
        "{caller}: expected refcount delta +{expected_delta} for {ptr:?}, before={before}, after={after}"
    );
}

/// Transfer one new owned reference into a container/global slot.
#[inline]
pub fn store_owned(value: super::value::MbValue) -> super::value::MbValue {
    #[cfg(debug_assertions)]
    let before = debug_heap_refcount(value);
    unsafe {
        retain_if_ptr(value);
    }
    #[cfg(debug_assertions)]
    debug_assert_refcount_delta(value, before, 1, "store_owned");
    value
}

/// Transfer one new owned reference to the caller as a return value.
#[inline]
pub fn return_owned(value: super::value::MbValue) -> super::value::MbValue {
    #[cfg(debug_assertions)]
    let before = debug_heap_refcount(value);
    unsafe {
        retain_if_ptr(value);
    }
    #[cfg(debug_assertions)]
    debug_assert_refcount_delta(value, before, 1, "return_owned");
    value
}

/// Transfer two independent new owners: one stored owner and one return owner.
#[inline]
pub fn store_and_return_owned(value: super::value::MbValue) -> super::value::MbValue {
    #[cfg(debug_assertions)]
    let before = debug_heap_refcount(value);
    unsafe {
        retain_if_ptr(value);
        retain_if_ptr(value);
    }
    #[cfg(debug_assertions)]
    debug_assert_refcount_delta(value, before, 2, "store_and_return_owned");
    value
}

/// Release a reference currently owned by a container/global slot.
#[inline]
pub fn release_owned(value: super::value::MbValue) {
    unsafe {
        release_if_ptr(value);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct LocalSlotReleaseRule {
    pub declared_i64: bool,
    pub raw_value: bool,
    pub native_bool: bool,
    pub assigned_on_path: bool,
    pub borrowed_param: bool,
    pub return_value: bool,
}

/// Authoritative emitter-side rule for whether a local slot owns a value that
/// should be released at a pre-write or epilogue release point.
#[inline]
pub fn should_release_local_slot(rule: LocalSlotReleaseRule) -> bool {
    rule.declared_i64
        && !rule.raw_value
        && !rule.native_bool
        && rule.assigned_on_path
        && !rule.borrowed_param
        && !rule.return_value
}

/// Returning a borrowed parameter creates a new owned result unless the slot
/// is a native raw value where retain is provably a no-op.
#[inline]
pub fn should_retain_borrowed_return(borrowed_param: bool, raw_value: bool) -> bool {
    borrowed_param && !raw_value
}

/// Loop pre-seeding may initialize local owner slots, but never borrowed
/// parameter slots whose original value remains owned by the caller.
#[inline]
pub fn should_preseed_loop_owner_slot(borrowed_param: bool) -> bool {
    !borrowed_param
}

/// Get the current reference count.
///
/// # Safety
/// `obj` must be a valid pointer.
pub unsafe fn mb_refcount(obj: *mut MbObject) -> u32 {
    (*obj).header.rc.load(Ordering::Relaxed)
}

// ── JIT-callable retain/release value wrappers ──
//
// These take a raw `u64` (NaN-boxed MbValue) from JIT-compiled code,
// check if it's a heap pointer, and delegate to mb_retain/mb_release.
// Non-pointer values (ints, bools, None, floats) are no-ops.

/// Increment the reference count of a NaN-boxed MbValue, if it points to a
/// heap object. Called from JIT-compiled code.
///
/// # Safety
/// `val` must be a valid NaN-boxed MbValue (as produced by the JIT).
pub unsafe extern "C" fn mb_retain_value(val: u64) {
    let v = super::value::MbValue::from_bits(val);
    if is_typed_native_wrapper(v) {
        return;
    }
    if let Some(ptr) = v.as_ptr() {
        mb_retain(ptr);
        return;
    }
    if super::closure::retain_closure_handle_if_live(v) {
        return;
    }
    // #2111: integer-pattern handles (array, hashlib, BytesIO, …) have no
    // heap pointer, so retain/release were previously no-ops and per-iter
    // rebinds leaked the backing storage. Dispatch to the registry so
    // handle-aware modules can refcount their entries.
    if let Some(id) = v.as_int() {
        if id > 0 {
            super::integer_handle_registry::retain(id as u64);
        }
    }
}

/// Decrement the reference count of a NaN-boxed MbValue, freeing the
/// object if the count reaches zero. Non-pointer values are no-ops.
/// Called from JIT-compiled code.
///
/// # Safety
/// `val` must be a valid NaN-boxed MbValue (as produced by the JIT).
pub unsafe extern "C" fn mb_release_value(val: u64) {
    let v = super::value::MbValue::from_bits(val);
    if is_typed_native_wrapper(v) {
        return;
    }
    if let Some(ptr) = v.as_ptr() {
        // `mb_release` runs `debug_validate_obj` under `cfg(debug_assertions)`,
        // which panics deterministically on a use-after-free / dangling
        // pointer (invalid ObjKind byte or misaligned address) instead of
        // silently skipping the release. This function is `extern "C"`
        // (called from JIT-compiled code), so a panic here unwinds straight
        // into an abort rather than being catchable — that is intentional:
        // a corrupted refcount slipping past as a silent leak is exactly
        // the failure mode that let this bug family go undetected before
        // (#1985, #2539). A loud, deterministic crash naming the offending
        // site is strictly preferable to a quiet double-release.
        mb_release(ptr);
        return;
    }
    if super::closure::release_closure_handle_if_live(v) {
        return;
    }
    // #2111: integer-pattern handles (array, hashlib, BytesIO, …) refcounted
    // via the integer-handle registry. Drops the underlying handle table
    // entry when the per-handle refcount reaches zero.
    if let Some(id) = v.as_int() {
        if id > 0 {
            super::integer_handle_registry::release(id as u64);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::value::MbValue;
    use super::*;

    #[test]
    fn test_str_object_lifecycle() {
        unsafe {
            let obj = MbObject::new_str("hello".into());
            assert_eq!(mb_refcount(obj), 1);
            assert_eq!((*obj).header.kind, ObjKind::Str);

            mb_retain(obj);
            assert_eq!(mb_refcount(obj), 2);

            mb_release(obj);
            assert_eq!(mb_refcount(obj), 1);

            mb_release(obj); // should free
        }
    }

    /// A2 positive control (issue #2539): proves the release-path UAF /
    /// dangling-pointer detector (`debug_validate_obj`, invoked from both
    /// `mb_retain`/`mb_release` and, since the #2539 fix, unconditionally
    /// from `mb_release_value`) still fires deterministically. This test
    /// must keep panicking forever — if it ever stops panicking, either the
    /// detector was weakened/disabled or a real regression silently
    /// resurrected the double-release class of bug.
    ///
    /// Two designs were tried and rejected before this one:
    ///
    /// 1. A naive back-to-back `mb_release(obj); mb_release(obj);` is NOT a
    ///    reliable trigger: the first release stores `IMMORTAL_REFCOUNT`
    ///    into the header immediately before freeing, deallocation does not
    ///    zero memory, and the stale header therefore often still reads as
    ///    "immortal" on the second release — which silently no-ops instead
    ///    of tripping the guard. That is exactly why the original bug was
    ///    intermittent: it only manifested once the freed slot was reused
    ///    for a *different*, incompatible allocation.
    /// 2. Corrupting `header.kind` in place to an out-of-range `ObjKind`
    ///    discriminant (then reading it back through `debug_validate_obj`'s
    ///    `(*obj).header.kind as u8`) is UB per Rust's enum validity
    ///    invariant, and it is NOT reliable across optimization levels in
    ///    practice: verified empirically that this trips the panic at
    ///    `CARGO_PROFILE_TEST_OPT_LEVEL=0` but the panic silently fails to
    ///    fire at `=1` (the compiler is entitled to assume an `ObjKind`
    ///    place always holds a valid discriminant once read as that type,
    ///    and may fold the out-of-range comparison away) — i.e. exactly the
    ///    kind of opt-level-sensitive false-negative this whole ticket
    ///    exists to rule out, so it cannot be the control.
    ///
    /// Instead, exercise `debug_validate_obj`'s *alignment* check, which is
    /// plain address arithmetic on a `usize` with no type-punning and no
    /// enum validity invariant involved, so it is optimization-level
    /// agnostic: offset a legitimately allocated object pointer by one byte
    /// (no real `MbObject` allocation is ever byte-unaligned) and release
    /// through it.
    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "misaligned MbObject pointer")]
    fn test_uaf_detector_trips_on_misaligned_pointer() {
        // #2585 E2: the detector is disarmed by default (regression-anchor
        // behavior); force-arm it in-process so this positive control
        // still trips regardless of `MAMBA_UAF_DETECTOR`.
        force_arm_uaf_detector();
        unsafe {
            let obj = MbObject::new_str("doomed".into());
            let bad_ptr = (obj as *mut u8).add(1) as *mut MbObject;
            mb_release(bad_ptr); // must panic via debug_validate_obj, not silently proceed
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_uaf_detector_env_decision() {
        assert!(!parse_uaf_detector_env(None));
        assert!(!parse_uaf_detector_env(Some("")));
        assert!(!parse_uaf_detector_env(Some("0")));
        assert!(parse_uaf_detector_env(Some("1")));
        assert!(parse_uaf_detector_env(Some("false")));
        assert!(parse_uaf_detector_env(Some("enabled")));
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_uaf_detector_monotonic_force_arm() {
        force_arm_uaf_detector();
        assert!(uaf_detector_armed());
        force_arm_uaf_detector();
        assert!(uaf_detector_armed());
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_uaf_detector_caught_panic_attribution() {
        force_arm_uaf_detector();
        set_current_test_name("test_uaf_detector_caught_panic_attribution");
        let res = std::panic::catch_unwind(|| unsafe {
            let dummy = 0x12345677 as *mut MbObject;
            mb_retain(dummy);
        });
        assert!(
            res.is_err(),
            "expected panic from misaligned pointer in mb_retain"
        );
        let err = res.unwrap_err();
        let msg = err
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| err.downcast_ref::<&'static str>().map(|s| s.to_string()))
            .expect("panic payload should be a string");
        assert!(
            msg.contains("mb_retain"),
            "expected 'mb_retain' in panic msg, got: {}",
            msg
        );
        assert!(
            msg.contains("[test=test_uaf_detector_caught_panic_attribution]"),
            "expected test identity in panic msg, got: {}",
            msg
        );
    }

    #[test]
    fn test_list_object() {
        unsafe {
            let list = MbObject::new_list(vec![MbValue::from_int(1), MbValue::from_int(2)]);
            assert_eq!((*list).header.kind, ObjKind::List);
            if let ObjData::List(ref lock) = (*list).data {
                let items = lock.read().unwrap();
                assert_eq!(items.len(), 2);
            } else {
                panic!("expected List");
            }
            mb_release(list);
        }
    }

    #[test]
    fn test_dict_object() {
        unsafe {
            let dict = MbObject::new_dict();
            assert_eq!((*dict).header.kind, ObjKind::Dict);
            mb_release(dict);
        }
    }

    #[test]
    fn test_concurrent_retain_release() {
        use std::thread;

        let obj = MbObject::new_str("shared".into());
        // Bump refcount so it survives all threads
        unsafe {
            (*obj).header.rc.store(1001, Ordering::Relaxed);
        }
        let addr = obj as usize;

        let handles: Vec<_> = (0..10)
            .map(|_| {
                thread::spawn(move || {
                    let ptr = addr as *mut MbObject;
                    for _ in 0..100 {
                        unsafe {
                            mb_retain(ptr);
                            mb_release(ptr);
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // After balanced retain/release, refcount should be back to 1001
        unsafe {
            assert_eq!(mb_refcount(obj), 1001);
            // Clean up
            (*obj).header.rc.store(1, Ordering::Relaxed);
            mb_release(obj);
        }
    }

    // ── Additional tests ──

    #[test]
    fn test_tuple_object() {
        unsafe {
            let tup = MbObject::new_tuple(vec![
                MbValue::from_int(10),
                MbValue::from_int(20),
                MbValue::from_int(30),
            ]);
            assert_eq!((*tup).header.kind, ObjKind::Tuple);
            assert_eq!(mb_refcount(tup), 1);
            if let ObjData::Tuple(ref items) = (*tup).data {
                assert_eq!(items.len(), 3);
                assert_eq!(items[0].as_int(), Some(10));
                assert_eq!(items[2].as_int(), Some(30));
            } else {
                panic!("expected Tuple");
            }
            mb_release(tup);
        }
    }

    #[test]
    fn test_set_object() {
        unsafe {
            let set = MbObject::new_set(vec![MbValue::from_int(1), MbValue::from_int(2)]);
            assert_eq!((*set).header.kind, ObjKind::Set);
            if let ObjData::Set(ref lock) = (*set).data {
                let items = lock.read().unwrap();
                assert_eq!(items.len(), 2);
            } else {
                panic!("expected Set");
            }
            mb_release(set);
        }
    }

    #[test]
    fn test_bytes_object() {
        unsafe {
            let b = MbObject::new_bytes(vec![0x41, 0x42, 0x43]);
            assert_eq!((*b).header.kind, ObjKind::Bytes);
            assert_eq!(mb_refcount(b), 1);
            if let ObjData::Bytes(ref data) = (*b).data {
                assert_eq!(data, &[0x41, 0x42, 0x43]);
            } else {
                panic!("expected Bytes");
            }
            mb_release(b);
        }
    }

    #[test]
    fn test_bytearray_object() {
        unsafe {
            let ba = MbObject::new_bytearray(vec![1, 2, 3]);
            assert_eq!((*ba).header.kind, ObjKind::ByteArray);
            if let ObjData::ByteArray(ref lock) = (*ba).data {
                let data = lock.read().unwrap();
                assert_eq!(&*data, &[1, 2, 3]);
            } else {
                panic!("expected ByteArray");
            }
            mb_release(ba);
        }
    }

    #[test]
    fn test_frozenset_object() {
        unsafe {
            let fs = MbObject::new_frozenset(vec![MbValue::from_int(10)]);
            assert_eq!((*fs).header.kind, ObjKind::FrozenSet);
            if let ObjData::FrozenSet(ref items) = (*fs).data {
                assert_eq!(items.len(), 1);
                assert_eq!(items[0].as_int(), Some(10));
            } else {
                panic!("expected FrozenSet");
            }
            mb_release(fs);
        }
    }

    #[test]
    fn test_instance_object() {
        unsafe {
            let inst = MbObject::new_instance("MyClass".to_string());
            assert_eq!((*inst).header.kind, ObjKind::Instance);
            assert_eq!(mb_refcount(inst), 1);
            if let ObjData::Instance {
                ref class_name,
                ref fields,
            } = (*inst).data
            {
                assert_eq!(class_name, "MyClass");
                assert!(fields.read().unwrap().is_empty());
            } else {
                panic!("expected Instance");
            }
            mb_release(inst);
        }
    }

    #[test]
    fn test_retain_null_is_safe() {
        unsafe {
            mb_retain(std::ptr::null_mut());
        }
        // Should not panic
    }

    #[test]
    fn test_release_null_is_safe() {
        unsafe {
            mb_release(std::ptr::null_mut());
        }
        // Should not panic
    }

    #[test]
    fn test_multiple_retain_release() {
        unsafe {
            let obj = MbObject::new_str("multi".into());
            mb_retain(obj);
            mb_retain(obj);
            assert_eq!(mb_refcount(obj), 3);
            mb_release(obj);
            mb_release(obj);
            assert_eq!(mb_refcount(obj), 1);
            mb_release(obj); // frees
        }
    }

    #[test]
    fn test_empty_list_object() {
        unsafe {
            let list = MbObject::new_list(vec![]);
            assert_eq!((*list).header.kind, ObjKind::List);
            if let ObjData::List(ref lock) = (*list).data {
                assert!(lock.read().unwrap().is_empty());
            }
            mb_release(list);
        }
    }

    #[test]
    fn test_empty_str_object() {
        unsafe {
            let obj = MbObject::new_str(String::new());
            assert_eq!((*obj).header.kind, ObjKind::Str);
            if let ObjData::Str(ref s) = (*obj).data {
                assert!(s.is_empty());
            }
            mb_release(obj);
        }
    }

    #[test]
    fn test_obj_kind_values() {
        assert_eq!(ObjKind::Str as u8, 0);
        assert_eq!(ObjKind::List as u8, 1);
        assert_eq!(ObjKind::Dict as u8, 2);
        assert_eq!(ObjKind::Tuple as u8, 3);
        assert_eq!(ObjKind::Function as u8, 4);
        assert_eq!(ObjKind::Class as u8, 5);
        assert_eq!(ObjKind::Instance as u8, 6);
        assert_eq!(ObjKind::Set as u8, 7);
        assert_eq!(ObjKind::Bytes as u8, 8);
        assert_eq!(ObjKind::ByteArray as u8, 9);
        assert_eq!(ObjKind::FrozenSet as u8, 10);
    }

    // ── Refcount JIT tests (#1129) ──

    #[test]
    fn test_immortal_refcount_constant() {
        // R4: IMMORTAL_REFCOUNT must be u32::MAX
        assert_eq!(IMMORTAL_REFCOUNT, u32::MAX);
    }

    #[test]
    fn test_new_str_immortal() {
        // R4: new_str_immortal creates object with rc == IMMORTAL_REFCOUNT
        unsafe {
            let obj = MbObject::new_str_immortal("hello".into());
            assert_eq!(mb_refcount(obj), IMMORTAL_REFCOUNT);
            assert_eq!((*obj).header.kind, ObjKind::Str);
            if let ObjData::Str(ref s) = (*obj).data {
                assert_eq!(s, "hello");
            } else {
                panic!("expected Str data");
            }
            // Cleanup: immortal objects are never freed by mb_release,
            // so force-free via Box::from_raw.
            drop(Box::from_raw(obj));
        }
    }

    #[test]
    fn test_new_bytes_immortal() {
        // R4: new_bytes_immortal creates object with rc == IMMORTAL_REFCOUNT
        unsafe {
            let obj = MbObject::new_bytes_immortal(vec![1, 2, 3]);
            assert_eq!(mb_refcount(obj), IMMORTAL_REFCOUNT);
            assert_eq!((*obj).header.kind, ObjKind::Bytes);
            if let ObjData::Bytes(ref data) = (*obj).data {
                assert_eq!(data, &[1, 2, 3]);
            } else {
                panic!("expected Bytes data");
            }
            drop(Box::from_raw(obj));
        }
    }

    /// #2107 carve-out regression marker — bytes-object materialization
    /// hot path. Locks in the invariant that `new_bytes` is a pure
    /// `Box::new` + move (zero memcpy of the payload buffer): the
    /// `Vec<u8>` storage pointer the caller hands in must be the
    /// SAME pointer reachable through `ObjData::Bytes` afterwards.
    ///
    /// If a future refactor copies the payload (e.g. by switching to
    /// an inline / compact-bytes layout that re-allocs for sizes
    /// above some threshold), this test will fail and the author
    /// must update #2107 with the new cost model before re-baselining
    /// the gzip / zlib / lzma / base64 cross-runtime benches.
    #[test]
    fn test_new_bytes_zero_copy_carveout_2107() {
        // 1 MiB payload — matches the cross-runtime bench fixture size
        // (`tests/cpython/std-libs/{gzip,zlib,lzma}/bench/compress_1mb.py`)
        // so a regression here is directly visible in those benches.
        let mut data = vec![0u8; 1024 * 1024];
        // Touch the first byte so MIRI / LLVM cannot fold the alloc.
        data[0] = 0x5A;
        let src_ptr = data.as_ptr();
        let src_len = data.len();
        let src_cap = data.capacity();

        unsafe {
            let obj = MbObject::new_bytes(data);
            // Same pointer ⇒ no memcpy of payload occurred during materialization.
            if let ObjData::Bytes(ref stored) = (*obj).data {
                assert_eq!(
                    stored.as_ptr(),
                    src_ptr,
                    "new_bytes must not memcpy payload (#2107)"
                );
                assert_eq!(stored.len(), src_len);
                assert_eq!(stored.capacity(), src_cap);
                assert_eq!(stored[0], 0x5A);
            } else {
                panic!("expected Bytes data");
            }
            drop(Box::from_raw(obj));
        }
    }

    #[test]
    fn test_retain_value_int_noop() {
        // R1: mb_retain_value on an integer MbValue is a no-op (no crash)
        unsafe {
            let int_val = MbValue::from_int(42);
            mb_retain_value(int_val.to_bits());
            // No crash, no state change — just verifying it doesn't segfault
        }
    }

    #[test]
    fn test_release_value_int_noop() {
        // R1: mb_release_value on an integer MbValue is a no-op (no crash)
        unsafe {
            let int_val = MbValue::from_int(42);
            mb_release_value(int_val.to_bits());
        }
    }

    #[test]
    fn test_retain_value_none_noop() {
        // R1: mb_retain_value on None is a no-op (no crash)
        unsafe {
            let none_val = MbValue::none();
            mb_retain_value(none_val.to_bits());
        }
    }

    #[test]
    fn test_release_value_zero_noop() {
        // R1: mb_release_value(0) is a no-op (uninitialized VReg default)
        unsafe {
            mb_release_value(0);
        }
    }

    #[test]
    fn test_retain_value_heap_obj() {
        // R1: mb_retain_value on a heap object increments refcount
        unsafe {
            let obj = MbObject::new_list(vec![MbValue::from_int(1)]);
            assert_eq!(mb_refcount(obj), 1);

            let val = MbValue::from_ptr(obj);
            mb_retain_value(val.to_bits());
            assert_eq!(mb_refcount(obj), 2);

            // Cleanup
            mb_release(obj);
            mb_release(obj); // frees at rc=0
        }
    }

    #[test]
    fn test_release_value_heap_obj() {
        // R1: mb_release_value on a heap object with rc=2 decrements refcount
        unsafe {
            let obj = MbObject::new_str("temp".into());
            assert_eq!(mb_refcount(obj), 1);
            mb_retain(obj);
            assert_eq!(mb_refcount(obj), 2);

            let val = MbValue::from_ptr(obj);
            mb_release_value(val.to_bits());
            assert_eq!(mb_refcount(obj), 1);

            // Cleanup
            mb_release(obj); // frees at rc=0
        }
    }

    #[test]
    fn test_store_and_return_owned_adds_two_refs() {
        unsafe {
            let obj = MbObject::new_str("owned".into());
            let val = MbValue::from_ptr(obj);
            assert_eq!(mb_refcount(obj), 1);

            let returned = store_and_return_owned(val);
            assert_eq!(returned.to_bits(), val.to_bits());
            assert_eq!(mb_refcount(obj), 3);

            release_owned(returned);
            release_owned(val);
            assert_eq!(mb_refcount(obj), 1);
            mb_release(obj);
        }
    }

    #[test]
    fn test_local_slot_release_rule_excludes_borrowed_params() {
        let base = LocalSlotReleaseRule {
            declared_i64: true,
            raw_value: false,
            native_bool: false,
            assigned_on_path: true,
            borrowed_param: false,
            return_value: false,
        };
        assert!(should_release_local_slot(base));
        assert!(!should_release_local_slot(LocalSlotReleaseRule {
            borrowed_param: true,
            ..base
        }));
        assert!(!should_release_local_slot(LocalSlotReleaseRule {
            return_value: true,
            ..base
        }));
    }

    #[test]
    fn test_retain_immortal_noop() {
        // R4: mb_retain_value on an immortal string keeps rc at IMMORTAL_REFCOUNT
        unsafe {
            let obj = MbObject::new_str_immortal("constant".into());
            assert_eq!(mb_refcount(obj), IMMORTAL_REFCOUNT);

            let val = MbValue::from_ptr(obj);
            mb_retain_value(val.to_bits());
            assert_eq!(mb_refcount(obj), IMMORTAL_REFCOUNT);

            // Cleanup: force-free since mb_release won't touch immortals
            drop(Box::from_raw(obj));
        }
    }

    #[test]
    fn test_release_immortal_noop() {
        // R4: mb_release_value on an immortal string keeps rc at IMMORTAL_REFCOUNT, not freed
        unsafe {
            let obj = MbObject::new_str_immortal("persistent".into());
            assert_eq!(mb_refcount(obj), IMMORTAL_REFCOUNT);

            let val = MbValue::from_ptr(obj);
            mb_release_value(val.to_bits());
            // Must still be alive with IMMORTAL_REFCOUNT
            assert_eq!(mb_refcount(obj), IMMORTAL_REFCOUNT);

            // Verify it's still accessible (not freed)
            if let ObjData::Str(ref s) = (*obj).data {
                assert_eq!(s, "persistent");
            } else {
                panic!("immortal object was corrupted or freed");
            }

            drop(Box::from_raw(obj));
        }
    }

    #[test]
    fn test_release_value_bool_noop() {
        // R1: mb_release_value on a boolean MbValue is a no-op
        unsafe {
            mb_release_value(MbValue::from_bool(true).to_bits());
            mb_release_value(MbValue::from_bool(false).to_bits());
        }
    }

    #[test]
    fn test_retain_release_value_float_noop() {
        // R1: mb_retain_value/mb_release_value on a float MbValue is a no-op
        unsafe {
            let float_val = MbValue::from_float(3.14);
            mb_retain_value(float_val.to_bits());
            mb_release_value(float_val.to_bits());
        }
    }

    #[test]
    fn test_typed_native_wrapper_refcount_ops_noop() {
        #[derive(Debug)]
        struct NativeProbe {
            marker: u32,
        }

        let registry_value = mamba_registry::convert::mb_wrap_native_typed(
            "NativeProbe",
            NativeProbe { marker: 41 },
        );
        let value = MbValue::from_bits(registry_value.to_bits());

        unsafe {
            mb_retain_value(value.to_bits());
            retain_if_ptr(value);
            mb_release_value(value.to_bits());
            release_if_ptr(value);
        }

        let probe: &NativeProbe =
            unsafe { mamba_registry::convert::mb_unwrap_native_ref(registry_value) }
                .expect("typed native wrapper should remain readable");
        assert_eq!(
            probe.marker, 41,
            "runtime refcount ops must not treat typed native wrappers as MbObject"
        );
    }

    #[test]
    fn test_bigint_and_complex_constructors() {
        // Anchors ObjKind::BigInt (=11) and ObjKind::Complex (=12) —
        // both in code but missing from spec R3 (tick-232 drift finding).
        unsafe {
            let bi = MbObject::new_bigint(BigInt::from(999_999_999_999i64));
            assert_eq!((*bi).header.kind, ObjKind::BigInt);
            assert_eq!(mb_refcount(bi), 1);
            mb_release(bi);

            let cx = MbObject::new_complex(1.0, -2.5);
            assert_eq!((*cx).header.kind, ObjKind::Complex);
            if let ObjData::Complex(r, i) = &(*cx).data {
                assert_eq!(*r, 1.0);
                assert_eq!(*i, -2.5);
            } else {
                panic!("expected ObjData::Complex");
            }
            mb_release(cx);
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_ownership_leak_balance_clean_scalar() {
        let guard = start_leak_balance_scope("clean_scalar").expect("start");

        unsafe {
            let s = MbObject::new_str("clean_scalar".to_string());
            mb_release(s);
        }
        let snap1 = get_leak_balance_snapshot().expect("active scope");
        assert_eq!(snap1.allocations, 1);
        assert_eq!(snap1.deallocations, 1);
        assert!(snap1.is_balanced());

        unsafe {
            let b = MbObject::new_bytes(vec![1, 2, 3, 4]);
            mb_release(b);
        }
        let snap2 = get_leak_balance_snapshot().expect("active scope");
        assert_eq!(snap2.allocations, 2);
        assert_eq!(snap2.deallocations, 2);
        assert!(snap2.is_balanced());

        unsafe {
            mb_release(MbObject::new_list(vec![]));
            mb_release(MbObject::new_dict());
            mb_release(MbObject::new_dict_with_capacity(4));
            mb_release(MbObject::new_tuple(vec![]));
            mb_release(MbObject::new_set(vec![]));
            mb_release(MbObject::new_bytearray(vec![0]));
            mb_release(MbObject::new_frozenset(vec![]));
            mb_release(MbObject::new_bigint(BigInt::from(42)));
            mb_release(MbObject::new_complex(1.0, 2.0));
            mb_release(MbObject::new_code_object(
                "x = 1".into(),
                "test.py".into(),
                "exec".into(),
                crate::parser::ast::Module { stmts: vec![] },
            ));
            mb_release(MbObject::new_instance("Foo".into()));
            mb_release(MbObject::new_instance_with_capacity("Bar".into(), 4));
        }

        let snap_final = guard.finish().expect("balanced");
        assert_eq!(snap_final.name, "clean_scalar");
        assert_eq!(snap_final.allocations, 14);
        assert_eq!(snap_final.deallocations, 14);
        assert_eq!(snap_final.delta(), 0);
        assert_eq!(snap_final.status, LeakBalanceStatus::Finished);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_ownership_leak_balance_nested_container() {
        let guard = start_leak_balance_scope("nested_container").expect("start");

        unsafe {
            let child = MbObject::new_str("child_str".to_string());
            let child_val = MbValue::from_ptr(child);
            let list = MbObject::new_list(vec![child_val]);

            let mid_snap = get_leak_balance_snapshot().expect("active scope");
            assert_eq!(mid_snap.allocations, 2);
            assert_eq!(mid_snap.deallocations, 0);
            assert_eq!(mid_snap.delta(), 2);
            assert!(!mid_snap.is_balanced());

            mb_release(list);
        }

        let final_snap = guard.finish().expect("balanced after releasing root");
        assert_eq!(final_snap.name, "nested_container");
        assert_eq!(final_snap.allocations, 2);
        assert_eq!(final_snap.deallocations, 2);
        assert_eq!(final_snap.delta(), 0);
        assert_eq!(final_snap.status, LeakBalanceStatus::Finished);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_ownership_leak_balance_deliberate_leak() {
        let guard = start_leak_balance_scope("deliberate_leak").expect("start");
        let leaked_ptr = unsafe { MbObject::new_str("deliberately_leaked".to_string()) };

        let mid_snap = get_leak_balance_snapshot().expect("active scope");
        assert_eq!(mid_snap.allocations, 1);
        assert_eq!(mid_snap.deallocations, 0);
        assert_eq!(mid_snap.delta(), 1);

        let res = std::panic::catch_unwind(|| {
            let _ = assert_leak_balanced();
        });
        assert!(res.is_err(), "assert_leak_balanced must panic on leak");

        unsafe {
            mb_release(leaked_ptr);
        }

        let post_cleanup_snap = guard.finish().expect("balanced after cleanup");
        assert_eq!(post_cleanup_snap.name, "deliberate_leak");
        assert_eq!(post_cleanup_snap.allocations, 1);
        assert_eq!(post_cleanup_snap.deallocations, 1);
        assert_eq!(post_cleanup_snap.delta(), 0);
        assert_eq!(post_cleanup_snap.status, LeakBalanceStatus::Finished);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_ownership_leak_balance_missing_scope_falsification() {
        reset_leak_balance_scope_for_testing();

        let snap_res = get_leak_balance_snapshot();
        assert!(snap_res.is_err());
        assert_eq!(snap_res.unwrap_err(), "leak balance scope is inactive");

        let assert_res = std::panic::catch_unwind(|| {
            let _ = assert_leak_balanced();
        });
        assert!(assert_res.is_err(), "assert_leak_balanced must panic when scope is inactive");

        let outer_guard = start_leak_balance_scope("outer_scope").expect("start outer");
        let inner_res = start_leak_balance_scope("inner_scope");
        assert!(inner_res.is_err(), "nested start must be rejected");
        assert!(inner_res.unwrap_err().contains("already active"));

        outer_guard.finish().expect("finish outer");

        {
            let _unfinished_guard =
                start_leak_balance_scope("unfinished_scope").expect("start unfinished");
        }

        let inc_snap_res = get_leak_balance_snapshot();
        assert!(inc_snap_res.is_err(), "incomplete scope must return Err");
        let inc_err = inc_snap_res.unwrap_err();
        assert!(inc_err.contains("unfinished_scope"), "error must include unfinished scope name");
        assert!(inc_err.contains("incomplete"), "error must identify incomplete status");

        let new_start_res = start_leak_balance_scope("new_scope");
        assert!(new_start_res.is_err(), "new start must be rejected while scope is incomplete");

        reset_leak_balance_scope_for_testing();
    }
}
