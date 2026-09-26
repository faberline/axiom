use mamba_registry::{
    convert::{mb_unwrap_native_ref, mb_wrap_native_typed},
    rt_sym, FromMbValue, IntoMbValue, MbValue, ModuleRegistrar,
};
use mambalibs_http::health::{HealthCheck, HealthManager, HealthStatus};

/// @spec .score/tech_design/projects/httpkit/health.md#x-mamba-binding
#[no_mangle]
pub unsafe extern "C" fn health_check_new(args: *const MbValue, nargs: usize) -> MbValue {
    let read = |i: usize| -> MbValue {
        if i < nargs {
            unsafe { *args.add(i) }
        } else {
            MbValue::none()
        }
    };
    let name = String::from_mb_value(read(0)).unwrap_or_default();
    let status = String::from_mb_value(read(1))
        .ok()
        .and_then(|s| s.parse::<HealthStatus>().ok())
        .unwrap_or(HealthStatus::Healthy);
    let description = String::from_mb_value(read(2)).ok();

    match HealthCheck::new(name, status, description) {
        Ok(value) => mb_wrap_native_typed("HealthCheck", value),
        Err(msg) => {
            if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
                (o.raise)("ValueError", &msg);
            }
            MbValue::none()
        }
    }
}

/// @spec .score/tech_design/projects/httpkit/health.md#x-mamba-attributes.name
#[no_mangle]
pub unsafe extern "C" fn health_check_get_name(args: *const MbValue, _nargs: usize) -> MbValue {
    let self_: &HealthCheck = match unsafe { mb_unwrap_native_ref(*args) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    self_.name.clone().into_mb_value()
}

/// @spec .score/tech_design/projects/httpkit/health.md#x-mamba-attributes.status
#[no_mangle]
pub unsafe extern "C" fn health_check_get_status(args: *const MbValue, _nargs: usize) -> MbValue {
    let self_: &HealthCheck = match unsafe { mb_unwrap_native_ref(*args) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    self_.status.as_str().to_string().into_mb_value()
}

/// @spec .score/tech_design/projects/httpkit/health.md#x-mamba-attributes.description
#[no_mangle]
pub unsafe extern "C" fn health_check_get_description(
    args: *const MbValue,
    _nargs: usize,
) -> MbValue {
    let self_: &HealthCheck = match unsafe { mb_unwrap_native_ref(*args) } {
        Some(r) => r,
        None => return MbValue::none(),
    };
    self_.description.clone().into_mb_value()
}

/// @spec .score/tech_design/projects/httpkit/health.md#x-mamba-binding
#[no_mangle]
pub unsafe extern "C" fn health_manager_new(_args: *const MbValue, _nargs: usize) -> MbValue {
    match HealthManager::new(Vec::new()) {
        Ok(value) => mb_wrap_native_typed("HealthManager", value),
        Err(msg) => {
            if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
                (o.raise)("ValueError", &msg);
            }
            MbValue::none()
        }
    }
}

pub fn register(r: &mut ModuleRegistrar) {
    r.add_symbol(rt_sym!(
        "HealthCheck",
        health_check_new,
        "HealthCheck(name: str, status: HealthStatus, description: str | None = None) -> HealthCheck"
    ));
    r.add_symbol(rt_sym!(
        "HealthManager",
        health_manager_new,
        "HealthManager(checks: list | None = None) -> HealthManager"
    ));
    if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
        (o.register_getter)("HealthCheck", "name", health_check_get_name);
        (o.register_getter)("HealthCheck", "status", health_check_get_status);
        (o.register_getter)("HealthCheck", "description", health_check_get_description);
    }
}
