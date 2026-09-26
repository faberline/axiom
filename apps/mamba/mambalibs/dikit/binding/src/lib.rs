//! Mamba interface for `mambalibs.di`.
//!
//! The core DI container lives in the sibling `mambalibs-di` crate. This
//! binding crate owns the Mamba import namespace `mambalibs.di`.

use mamba_registry::{
    convert::{mb_unwrap_native_ref, mb_wrap_native_typed},
    rt_sym, FromMbValue, IntoMbValue, MambaModule, MbValue, ModuleRegistrar, MAMBA_MODULES,
};
use linkme::distributed_slice;
use mambalibs_di::{Container, DependencyMarker, ProviderKey, RequestScope, ScopeKind};
use std::collections::HashMap;

#[derive(Clone)]
pub struct MbDiContainer {
    pub inner: Container,
}

impl Default for MbDiContainer {
    fn default() -> Self {
        Self {
            inner: Container::new(),
        }
    }
}

#[derive(Clone)]
pub struct MbDiScope {
    pub inner: RequestScope,
}

type NativeFn = unsafe extern "C" fn(*const MbValue, usize) -> MbValue;

unsafe fn read(args: *const MbValue, nargs: usize, index: usize) -> MbValue {
    if index < nargs {
        unsafe { *args.add(index) }
    } else {
        MbValue::none()
    }
}

fn read_string(value: MbValue) -> Option<String> {
    String::from_mb_value(value).ok()
}

unsafe fn read_container(value: MbValue) -> Option<&'static MbDiContainer> {
    unsafe { mb_unwrap_native_ref::<MbDiContainer>(value) }
}

unsafe fn read_scope(value: MbValue) -> Option<&'static MbDiScope> {
    unsafe { mb_unwrap_native_ref::<MbDiScope>(value) }
}

fn read_scope_kind(value: MbValue) -> ScopeKind {
    read_string(value)
        .and_then(|scope| ScopeKind::parse(&scope))
        .unwrap_or(ScopeKind::Singleton)
}

fn none_or_string(result: mambalibs_di::DiResult<std::sync::Arc<String>>) -> MbValue {
    match result {
        Ok(value) => value.as_str().into_mb_value(),
        Err(_) => MbValue::none(),
    }
}

fn none_or_string_map(
    result: mambalibs_di::DiResult<HashMap<String, std::sync::Arc<String>>>,
) -> MbValue {
    match result {
        Ok(values) => values
            .into_iter()
            .map(|(key, value)| (key, value.as_str().to_string()))
            .collect::<HashMap<_, _>>()
            .into_mb_value(),
        Err(_) => MbValue::none(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn di_container_new(_args: *const MbValue, _nargs: usize) -> MbValue {
    mb_wrap_native_typed("Container", MbDiContainer::default())
}

#[no_mangle]
pub unsafe extern "C" fn di_request_scope_new(args: *const MbValue, nargs: usize) -> MbValue {
    let container_value = unsafe { read(args, nargs, 0) };
    let Some(container) = (unsafe { read_container(container_value) }) else {
        return MbValue::none();
    };
    mb_wrap_native_typed(
        "RequestScope",
        MbDiScope {
            inner: container.inner.request_scope(),
        },
    )
}

#[no_mangle]
pub unsafe extern "C" fn di_depends_new(args: *const MbValue, nargs: usize) -> MbValue {
    let key_value = unsafe { read(args, nargs, 0) };
    let marker = read_string(key_value)
        .and_then(|key| DependencyMarker::new(key).ok())
        .unwrap_or_else(DependencyMarker::inferred);
    mb_wrap_native_typed("Depends", marker)
}

#[no_mangle]
pub unsafe extern "C" fn di_container_register_value(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let container_value = unsafe { read(args, nargs, 0) };
    let key_value = unsafe { read(args, nargs, 1) };
    let stored_value = unsafe { read(args, nargs, 2) };
    let scope_value = unsafe { read(args, nargs, 3) };

    let Some(container) = (unsafe { read_container(container_value) }) else {
        return MbValue::none();
    };
    let Some(key) = read_string(key_value) else {
        return MbValue::none();
    };
    let value = read_string(stored_value).unwrap_or_default();
    let scope = read_scope_kind(scope_value);

    if container.inner.register_value(key, scope, value).is_err() {
        return MbValue::none();
    }
    container_value
}

#[no_mangle]
pub unsafe extern "C" fn di_container_override_value(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let container_value = unsafe { read(args, nargs, 0) };
    let key_value = unsafe { read(args, nargs, 1) };
    let stored_value = unsafe { read(args, nargs, 2) };

    let Some(container) = (unsafe { read_container(container_value) }) else {
        return MbValue::none();
    };
    let Some(key) = read_string(key_value) else {
        return MbValue::none();
    };
    let value = read_string(stored_value).unwrap_or_default();

    if container.inner.override_value(key, value).is_err() {
        return MbValue::none();
    }
    container_value
}

#[no_mangle]
pub unsafe extern "C" fn di_container_clear_override(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let container_value = unsafe { read(args, nargs, 0) };
    let key_value = unsafe { read(args, nargs, 1) };

    let Some(container) = (unsafe { read_container(container_value) }) else {
        return MbValue::none();
    };
    let Some(key) = read_string(key_value) else {
        return MbValue::none();
    };

    if container.inner.clear_override(key).is_err() {
        return MbValue::none();
    }
    container_value
}

#[no_mangle]
pub unsafe extern "C" fn di_container_resolve(args: *const MbValue, nargs: usize) -> MbValue {
    let container_value = unsafe { read(args, nargs, 0) };
    let key_value = unsafe { read(args, nargs, 1) };

    let Some(container) = (unsafe { read_container(container_value) }) else {
        return MbValue::none();
    };
    let Some(key) = read_string(key_value) else {
        return MbValue::none();
    };

    none_or_string(container.inner.resolve::<String>(key))
}

#[no_mangle]
pub unsafe extern "C" fn di_container_resolve_many(args: *const MbValue, nargs: usize) -> MbValue {
    let container_value = unsafe { read(args, nargs, 0) };
    let keys_value = unsafe { read(args, nargs, 1) };

    let Some(container) = (unsafe { read_container(container_value) }) else {
        return MbValue::none();
    };
    let Ok(keys) = Vec::<String>::from_mb_value(keys_value) else {
        return MbValue::none();
    };

    none_or_string_map(container.inner.resolve_many::<String, _, _>(keys))
}

#[no_mangle]
pub unsafe extern "C" fn di_scope_resolve(args: *const MbValue, nargs: usize) -> MbValue {
    let scope_value = unsafe { read(args, nargs, 0) };
    let key_value = unsafe { read(args, nargs, 1) };

    let Some(scope) = (unsafe { read_scope(scope_value) }) else {
        return MbValue::none();
    };
    let Some(key) = read_string(key_value) else {
        return MbValue::none();
    };

    none_or_string(scope.inner.resolve::<String>(key))
}

#[no_mangle]
pub unsafe extern "C" fn di_scope_resolve_many(args: *const MbValue, nargs: usize) -> MbValue {
    let scope_value = unsafe { read(args, nargs, 0) };
    let keys_value = unsafe { read(args, nargs, 1) };

    let Some(scope) = (unsafe { read_scope(scope_value) }) else {
        return MbValue::none();
    };
    let Ok(keys) = Vec::<String>::from_mb_value(keys_value) else {
        return MbValue::none();
    };

    none_or_string_map(scope.inner.resolve_many::<String, _, _>(keys))
}

fn register_di_surface(r: &mut ModuleRegistrar) {
    r.add_symbols([
        rt_sym!("Container", di_container_new, "Container() -> container"),
        rt_sym!(
            "RequestScope",
            di_request_scope_new,
            "RequestScope(container) -> request_scope"
        ),
        rt_sym!("Depends", di_depends_new, "Depends(key: str | None = None) -> dependency"),
        rt_sym!(
            "container_register_value",
            di_container_register_value,
            "container_register_value(container, key: str, value: str, scope: str = 'singleton') -> container"
        ),
        rt_sym!(
            "container_override_value",
            di_container_override_value,
            "container_override_value(container, key: str, value: str) -> container"
        ),
        rt_sym!(
            "container_clear_override",
            di_container_clear_override,
            "container_clear_override(container, key: str) -> container"
        ),
        rt_sym!(
            "container_resolve",
            di_container_resolve,
            "container_resolve(container, key: str) -> str | None"
        ),
        rt_sym!(
            "container_resolve_many",
            di_container_resolve_many,
            "container_resolve_many(container, keys: list[str]) -> dict[str, str] | None"
        ),
        rt_sym!(
            "scope_resolve",
            di_scope_resolve,
            "scope_resolve(scope, key: str) -> str | None"
        ),
        rt_sym!(
            "scope_resolve_many",
            di_scope_resolve_many,
            "scope_resolve_many(scope, keys: list[str]) -> dict[str, str] | None"
        ),
    ]);
}

pub struct MambalibsDiModule;

impl MambaModule for MambalibsDiModule {
    fn name(&self) -> &'static str {
        "mambalibs.di"
    }

    fn doc(&self) -> &'static str {
        "Reusable dependency injection for Mamba native libraries"
    }

    fn register(&self, r: &mut ModuleRegistrar) {
        register_di_surface(r);
    }
}

#[distributed_slice(MAMBA_MODULES)]
static MAMBALIBS_DI_MODULE: &dyn MambaModule = &MambalibsDiModule;

pub fn dependency_key(marker: &DependencyMarker) -> Option<&ProviderKey> {
    marker.key()
}

#[allow(dead_code)]
fn _assert_native_fn(_: NativeFn) {}
