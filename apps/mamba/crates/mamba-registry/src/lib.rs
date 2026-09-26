//! Mamba native-module registry.
//!
//! Provides the infrastructure for Rust crates to register themselves as
//! Mamba-callable modules at link time using the `linkme` distributed slice.
//!
//! # Usage
//!
//! ```ignore
//! use mamba_registry::{MambaModule, ModuleRegistrar, MAMBA_MODULES, rt_sym};
//! use linkme::distributed_slice;
//!
//! pub struct MyModule;
//!
//! impl MambaModule for MyModule {
//!     fn name(&self) -> &'static str { "my_module" }
//!     fn register(&self, r: &mut ModuleRegistrar) {
//!         r.add_symbol(rt_sym!(my_fn, my_fn as unsafe extern "C" fn()));
//!     }
//! }
//!
//! #[distributed_slice(MAMBA_MODULES)]
//! static MY_MODULE: &dyn MambaModule = &MyModule;
//! ```

pub mod convert;
pub mod exc;
pub mod http;
pub mod ops;
pub mod rc;
pub mod runtime;
pub mod test_ops;
pub use convert::{FromMbValue, IntoMbValue, MbConvError};
pub use exc::{
    raise_instance, raise_key_error, raise_runtime_error, raise_type_error, raise_value_error,
};
pub use ops::{ops, set_object_ops, ObjectOps};

use linkme::distributed_slice;

// ── MbValue ──────────────────────────────────────────────────────────────────

/// NaN-boxed Mamba value (64-bit).
///
/// Mirrors the layout used by `cclab-mamba`. Defined here so binding crates
/// can depend only on this lightweight registry crate, not the full compiler.
///
/// Layout:
///   - Float: any `f64` that is NOT a signaling NaN with our tag prefix
///   - Tagged NaN: `1_11111111111_1_TTT_PPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPPP`
///     - TTT = 000 → pointer, 001 → i64, 010 → bool, 011 → None, 100 → func ptr
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MbValue(u64);

const NAN_PREFIX: u64 = 0xFFF8_0000_0000_0000;
const TAG_MASK: u64 = 0x0007_0000_0000_0000;
const TAG_SHIFT: u32 = 48;
const PAYLOAD_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

const TAG_PTR: u64 = 0;
const TAG_INT: u64 = 1;
const TAG_BOOL: u64 = 2;
const TAG_NONE: u64 = 3;
const TAG_FUNC: u64 = 4;

impl MbValue {
    pub fn from_float(f: f64) -> Self {
        let bits = f.to_bits();
        if bits & NAN_PREFIX == NAN_PREFIX && bits != f64::NAN.to_bits() {
            Self(f64::NAN.to_bits())
        } else {
            Self(bits)
        }
    }

    pub fn from_int(i: i64) -> Self {
        let payload = (i as u64) & PAYLOAD_MASK;
        Self(NAN_PREFIX | (TAG_INT << TAG_SHIFT) | payload)
    }

    pub fn from_bool(b: bool) -> Self {
        Self(NAN_PREFIX | (TAG_BOOL << TAG_SHIFT) | (b as u64))
    }

    pub fn none() -> Self {
        Self(NAN_PREFIX | (TAG_NONE << TAG_SHIFT))
    }

    pub fn from_ptr(addr: usize) -> Self {
        Self(NAN_PREFIX | (TAG_PTR << TAG_SHIFT) | (addr as u64 & PAYLOAD_MASK))
    }

    pub fn from_func(addr: usize) -> Self {
        Self(NAN_PREFIX | (TAG_FUNC << TAG_SHIFT) | (addr as u64 & PAYLOAD_MASK))
    }

    fn tag(self) -> Option<u64> {
        if (self.0 & NAN_PREFIX) == NAN_PREFIX && self.0 != f64::NAN.to_bits() {
            Some((self.0 & TAG_MASK) >> TAG_SHIFT)
        } else {
            None
        }
    }

    pub fn is_float(self) -> bool {
        (self.0 & NAN_PREFIX) != NAN_PREFIX || self.0 == f64::NAN.to_bits()
    }
    pub fn is_int(self) -> bool {
        self.tag() == Some(TAG_INT)
    }
    pub fn is_bool(self) -> bool {
        self.tag() == Some(TAG_BOOL)
    }
    pub fn is_none(self) -> bool {
        self.tag() == Some(TAG_NONE)
    }
    pub fn is_ptr(self) -> bool {
        self.tag() == Some(TAG_PTR)
    }
    pub fn is_func(self) -> bool {
        self.tag() == Some(TAG_FUNC)
    }

    pub fn as_float(self) -> Option<f64> {
        if self.is_float() {
            Some(f64::from_bits(self.0))
        } else {
            None
        }
    }
    pub fn as_int(self) -> Option<i64> {
        if self.is_int() {
            let raw = (self.0 & PAYLOAD_MASK) as i64;
            Some((raw << 16) >> 16)
        } else {
            None
        }
    }
    pub fn as_bool(self) -> Option<bool> {
        if self.is_bool() {
            Some((self.0 & 1) != 0)
        } else {
            None
        }
    }
    pub fn as_ptr(self) -> Option<usize> {
        if self.is_ptr() {
            Some((self.0 & PAYLOAD_MASK) as usize)
        } else {
            None
        }
    }
    pub fn as_func(self) -> Option<usize> {
        if self.is_func() {
            Some((self.0 & PAYLOAD_MASK) as usize)
        } else {
            None
        }
    }

    pub fn to_bits(self) -> u64 {
        self.0
    }
    pub fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// Read the string content from a `str`-shaped `MbValue`. Returns
    /// `None` if the value is not a pointer to a mamba string.
    ///
    /// # Return type change (PR-6)
    ///
    /// Previously returned `Option<&str>` by reading through the now-
    /// deleted `ObjData::Str` layout mirror. Now routes through
    /// [`ops::ops`]`().str_read`, which must allocate a fresh `String`
    /// (mamba's real `ObjData` layout isn't exposed to this crate). All
    /// known callers only needed a short-lived view, so this is a safe
    /// allocation trade for eliminating the mirror's drift-UB risk.
    ///
    /// # Safety
    ///
    /// Preserved `unsafe` for source compatibility with existing callers
    /// that wrap the call in `unsafe { … }`. The body is now safe — new
    /// code should call [`crate::ops()`]`().str_read` directly.
    pub unsafe fn as_obj_str(&self) -> Option<String> {
        (ops::ops().str_read)(*self)
    }
}

impl std::fmt::Debug for MbValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_none() {
            write!(f, "MbValue::None")
        } else if let Some(i) = self.as_int() {
            write!(f, "MbValue::Int({i})")
        } else if let Some(b) = self.as_bool() {
            write!(f, "MbValue::Bool({b})")
        } else if let Some(fl) = self.as_float() {
            write!(f, "MbValue::Float({fl})")
        } else if self.is_ptr() {
            write!(f, "MbValue::Ptr({:#x})", self.0 & PAYLOAD_MASK)
        } else {
            write!(f, "MbValue::Raw({:#018x})", self.0)
        }
    }
}

// ── RuntimeSymbol ─────────────────────────────────────────────────────────────

/// A named symbol exposed by a native Mamba module.
#[derive(Debug, Clone)]
pub struct RuntimeSymbol {
    /// Python-visible name of the symbol (e.g. `"get_logger"`).
    pub name: &'static str,
    /// FFI symbol name used for JIT registration (e.g. `"mb_log_get_logger"`).
    /// Derived automatically by the `rt_sym!` macro from the function identifier.
    pub ffi_name: &'static str,
    /// Raw function pointer (ABI: `extern "C" fn(*const MbValue, usize) -> MbValue`).
    pub func_ptr: usize,
    /// Human-readable signature string for introspection.
    pub signature: &'static str,
}

impl RuntimeSymbol {
    pub const fn new(
        name: &'static str,
        ffi_name: &'static str,
        func_ptr: usize,
        signature: &'static str,
    ) -> Self {
        Self {
            name,
            ffi_name,
            func_ptr,
            signature,
        }
    }
}

/// A named non-function value exposed by a native Mamba module.
#[derive(Debug, Clone)]
pub struct RuntimeValue {
    /// Python-visible name of the value (e.g. `"HTTPStatus"`).
    pub name: &'static str,
    /// Produces the Mamba value during runtime module registration.
    pub value_fn: fn() -> MbValue,
}

impl RuntimeValue {
    pub const fn new(name: &'static str, value_fn: fn() -> MbValue) -> Self {
        Self { name, value_fn }
    }

    pub fn value(&self) -> MbValue {
        (self.value_fn)()
    }
}

/// Declare a [`RuntimeSymbol`] from a function identifier.
///
/// The first argument is the **Python-visible name** (e.g. `"get_logger"`).
/// The FFI symbol name is derived automatically from the function identifier
/// via `stringify!`.
///
/// ```ignore
/// rt_sym!("sqrt", fast_sqrt, "sqrt(x: float) -> float")
/// ```
///
/// Short form: `rt_sym!(name, fn_ptr)` — uses the Python name as signature too.
#[macro_export]
macro_rules! rt_sym {
    ($name:literal, $fn_ptr:expr, $sig:literal) => {
        $crate::RuntimeSymbol::new($name, stringify!($fn_ptr), $fn_ptr as usize, $sig)
    };
    ($name:literal, $fn_ptr:expr) => {
        $crate::RuntimeSymbol::new($name, stringify!($fn_ptr), $fn_ptr as usize, $name)
    };
}

// ── ModuleRegistrar ───────────────────────────────────────────────────────────

/// Collects symbols registered by a [`MambaModule`] implementation.
#[derive(Default)]
pub struct ModuleRegistrar {
    symbols: Vec<RuntimeSymbol>,
    values: Vec<RuntimeValue>,
}

impl ModuleRegistrar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a single native symbol.
    pub fn add_symbol(&mut self, sym: RuntimeSymbol) {
        self.symbols.push(sym);
    }

    /// Register multiple native symbols at once.
    pub fn add_symbols(&mut self, syms: impl IntoIterator<Item = RuntimeSymbol>) {
        self.symbols.extend(syms);
    }

    /// Register a non-function module value.
    pub fn add_value(&mut self, value: RuntimeValue) {
        self.values.push(value);
    }

    /// Register multiple module values at once.
    pub fn add_values(&mut self, values: impl IntoIterator<Item = RuntimeValue>) {
        self.values.extend(values);
    }

    /// Consume the registrar and return all collected symbols.
    pub fn into_symbols(self) -> Vec<RuntimeSymbol> {
        self.symbols
    }

    /// Consume the registrar and return symbols plus module values.
    pub fn into_parts(self) -> (Vec<RuntimeSymbol>, Vec<RuntimeValue>) {
        (self.symbols, self.values)
    }

    /// Borrow the collected symbols.
    pub fn symbols(&self) -> &[RuntimeSymbol] {
        &self.symbols
    }

    /// Borrow the collected non-function values.
    pub fn values(&self) -> &[RuntimeValue] {
        &self.values
    }
}

// ── MambaModule trait ─────────────────────────────────────────────────────────

/// Trait implemented by each native Mamba-binding crate.
///
/// Implementors use `#[distributed_slice(MAMBA_MODULES)]` to auto-register at
/// link time.
pub trait MambaModule: Send + Sync {
    /// The Mamba import name for this module (e.g. `"mambalibs.pg"`).
    fn name(&self) -> &'static str;

    /// Register all exposed symbols into the provided [`ModuleRegistrar`].
    fn register(&self, registrar: &mut ModuleRegistrar);

    /// Optional: module docstring shown to introspection tools.
    fn doc(&self) -> &'static str {
        ""
    }
}

// ── MAMBA_MODULES distributed slice ──────────────────────────────────────────

/// All registered Mamba native modules, collected at link time via `linkme`.
///
/// Binding crates self-register with:
/// ```ignore
/// #[distributed_slice(MAMBA_MODULES)]
/// static MY_MODULE: &dyn MambaModule = &MyModuleImpl;
/// ```
#[distributed_slice]
pub static MAMBA_MODULES: [&'static dyn MambaModule];

/// Iterate all registered Mamba modules.
pub fn all_modules() -> impl Iterator<Item = &'static dyn MambaModule> {
    MAMBA_MODULES.iter().copied()
}

/// Look up a registered module by its Mamba import name.
pub fn find_module(name: &str) -> Option<&'static dyn MambaModule> {
    MAMBA_MODULES.iter().find(|m| m.name() == name).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mbvalue_int_roundtrip() {
        for i in [0i64, 1, -1, 42, -1000, (1 << 47) - 1, -(1 << 47)] {
            let v = MbValue::from_int(i);
            assert!(v.is_int());
            assert_eq!(v.as_int(), Some(i));
            assert!(!v.is_float());
        }
    }

    #[test]
    fn test_mbvalue_float_roundtrip() {
        for f in [0.0f64, 1.0, -1.0, 3.14, f64::INFINITY] {
            let v = MbValue::from_float(f);
            assert!(v.is_float());
            assert_eq!(v.as_float(), Some(f));
        }
    }

    #[test]
    fn test_mbvalue_bool() {
        assert_eq!(MbValue::from_bool(true).as_bool(), Some(true));
        assert_eq!(MbValue::from_bool(false).as_bool(), Some(false));
    }

    #[test]
    fn test_mbvalue_none() {
        let n = MbValue::none();
        assert!(n.is_none());
        assert!(!n.is_int());
    }

    #[test]
    fn test_registrar() {
        let mut r = ModuleRegistrar::new();
        r.add_symbol(RuntimeSymbol::new(
            "my_fn",
            "mb_my_fn",
            0xDEAD,
            "my_fn() -> None",
        ));
        r.add_value(RuntimeValue::new("answer", || MbValue::from_int(42)));
        assert_eq!(r.symbols().len(), 1);
        assert_eq!(r.symbols()[0].name, "my_fn");
        assert_eq!(r.symbols()[0].ffi_name, "mb_my_fn");
        assert_eq!(r.values().len(), 1);
        assert_eq!(r.values()[0].name, "answer");
        assert_eq!(r.values()[0].value().as_int(), Some(42));
    }

    #[test]
    fn test_mamba_modules_slice_accessible() {
        // Just ensure the slice is reachable; count may be 0 in unit test binary.
        let _count = MAMBA_MODULES.iter().count();
    }
}
