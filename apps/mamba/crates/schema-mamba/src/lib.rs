//! Mamba binding for `schema`.
//!
//! This crate wires `schema` validation capabilities into the Mamba
//! runtime via the `mamba-registry` infrastructure.
//!
//! # Module name
//!
//! Import in Mamba as `mambalibs.dataclasses`:
//! ```python
//! from mambalibs.dataclasses import BaseModel, Field
//! ```
//! `cclab_schema_mamba` remains registered as a compatibility alias.
//!
//! # Blueprint pattern
//!
//! Each `{crate}-mamba` binding follows this structure:
//! - `lib.rs`  — [`MambaModule`] impl + [`MAMBA_MODULES`] registration
//! - `types.rs` — [`FromMbValue`] / [`IntoMbValue`] conversions
//! - `methods.rs` — `extern "C"` FFI functions (the actual binding surface)

pub mod methods;
pub mod types;

use linkme::distributed_slice;
use mamba_registry::{rt_sym, MambaModule, ModuleRegistrar, MAMBA_MODULES};

fn register_schema_surface(r: &mut ModuleRegistrar) {
    use crate::methods::{
        get_model_add_field, get_model_add_fields, get_model_model_dump, get_model_model_dump_json,
        get_model_model_json_schema, get_model_model_validate, get_model_model_validate_json,
        get_model_parse_obj, get_model_parse_raw, get_model_to_json_schema, get_model_validate,
        mb_schema_base_model_new, mb_schema_create_model, mb_schema_field,
        mb_schema_field_validator, mb_schema_model_add_field, mb_schema_model_add_field_bound,
        mb_schema_model_add_fields, mb_schema_model_add_fields_bound, mb_schema_model_dump,
        mb_schema_model_dump_bound, mb_schema_model_dump_json, mb_schema_model_dump_json_bound,
        mb_schema_model_json_schema, mb_schema_model_json_schema_bound, mb_schema_model_validate,
        mb_schema_model_validate_bound, mb_schema_model_validate_json,
        mb_schema_model_validate_json_bound, mb_schema_parse_obj, mb_schema_parse_obj_bound,
        mb_schema_parse_raw, mb_schema_parse_raw_bound, mb_schema_to_json_schema,
        mb_schema_to_json_schema_bound, mb_schema_validate, mb_schema_validate_bound,
    };

    r.add_symbols([
        rt_sym!(
            "BaseModel",
            mb_schema_base_model_new,
            "BaseModel(name: str) -> model"
        ),
        rt_sym!(
            "DataClass",
            mb_schema_base_model_new,
            "DataClass(name: str) -> model"
        ),
        rt_sym!(
            "Field",
            mb_schema_field,
            "Field(name: str, kwargs: dict) | Field(kwargs: dict) -> field"
        ),
        rt_sym!(
            "create_model",
            mb_schema_create_model,
            "create_model(name: str, fields: dict = {}) -> model"
        ),
        rt_sym!(
            "add_field",
            mb_schema_model_add_field,
            "add_field(model, field) -> model"
        ),
        rt_sym!(
            "add_fields",
            mb_schema_model_add_fields,
            "add_fields(model, fields: dict) -> model"
        ),
        rt_sym!(
            "validate",
            mb_schema_validate,
            "validate(model, data: dict) -> bool"
        ),
        rt_sym!(
            "model_validate",
            mb_schema_model_validate,
            "model_validate(model, data: dict, options: dict = {}) -> dict"
        ),
        rt_sym!(
            "parse_obj",
            mb_schema_parse_obj,
            "parse_obj(model, data: dict, options: dict = {}) -> dict"
        ),
        rt_sym!(
            "model_validate_json",
            mb_schema_model_validate_json,
            "model_validate_json(model, json_text: str, options: dict = {}) -> dict"
        ),
        rt_sym!(
            "parse_raw",
            mb_schema_parse_raw,
            "parse_raw(model, json_text: str, options: dict = {}) -> dict"
        ),
        rt_sym!(
            "model_dump",
            mb_schema_model_dump,
            "model_dump(model, data: dict, options: dict = {}) -> dict"
        ),
        rt_sym!(
            "model_dump_json",
            mb_schema_model_dump_json,
            "model_dump_json(model, data: dict, options: dict = {}) -> str"
        ),
        rt_sym!(
            "field_validator",
            mb_schema_field_validator,
            "field_validator(model, field: str, fn) -> None"
        ),
        rt_sym!(
            "to_json_schema",
            mb_schema_to_json_schema,
            "to_json_schema(model) -> str"
        ),
        rt_sym!(
            "model_json_schema",
            mb_schema_model_json_schema,
            "model_json_schema(model) -> str"
        ),
        rt_sym!(
            "_schema_model_add_field_bound",
            mb_schema_model_add_field_bound,
            "_schema_model_add_field_bound(field) -> model"
        ),
        rt_sym!(
            "_schema_model_add_fields_bound",
            mb_schema_model_add_fields_bound,
            "_schema_model_add_fields_bound(fields: dict) -> model"
        ),
        rt_sym!(
            "_schema_model_validate_bound",
            mb_schema_validate_bound,
            "_schema_model_validate_bound(data: dict) -> bool"
        ),
        rt_sym!(
            "_schema_model_model_validate_bound",
            mb_schema_model_validate_bound,
            "_schema_model_model_validate_bound(data: dict, options: dict = {}) -> dict"
        ),
        rt_sym!(
            "_schema_model_parse_obj_bound",
            mb_schema_parse_obj_bound,
            "_schema_model_parse_obj_bound(data: dict, options: dict = {}) -> dict"
        ),
        rt_sym!(
            "_schema_model_model_validate_json_bound",
            mb_schema_model_validate_json_bound,
            "_schema_model_model_validate_json_bound(json_text: str, options: dict = {}) -> dict"
        ),
        rt_sym!(
            "_schema_model_parse_raw_bound",
            mb_schema_parse_raw_bound,
            "_schema_model_parse_raw_bound(json_text: str, options: dict = {}) -> dict"
        ),
        rt_sym!(
            "_schema_model_model_dump_bound",
            mb_schema_model_dump_bound,
            "_schema_model_model_dump_bound(data: dict, options: dict = {}) -> dict"
        ),
        rt_sym!(
            "_schema_model_model_dump_json_bound",
            mb_schema_model_dump_json_bound,
            "_schema_model_model_dump_json_bound(data: dict, options: dict = {}) -> str"
        ),
        rt_sym!(
            "_schema_model_to_json_schema_bound",
            mb_schema_to_json_schema_bound,
            "_schema_model_to_json_schema_bound() -> str"
        ),
        rt_sym!(
            "_schema_model_model_json_schema_bound",
            mb_schema_model_json_schema_bound,
            "_schema_model_model_json_schema_bound() -> str"
        ),
    ]);

    register_getter("BaseModel", "add_field", get_model_add_field);
    register_getter("BaseModel", "add_fields", get_model_add_fields);
    register_getter("BaseModel", "validate", get_model_validate);
    register_getter("BaseModel", "model_validate", get_model_model_validate);
    register_getter("BaseModel", "parse_obj", get_model_parse_obj);
    register_getter(
        "BaseModel",
        "model_validate_json",
        get_model_model_validate_json,
    );
    register_getter("BaseModel", "parse_raw", get_model_parse_raw);
    register_getter("BaseModel", "model_dump", get_model_model_dump);
    register_getter("BaseModel", "model_dump_json", get_model_model_dump_json);
    register_getter("BaseModel", "to_json_schema", get_model_to_json_schema);
    register_getter(
        "BaseModel",
        "model_json_schema",
        get_model_model_json_schema,
    );
}

type NativeGetter =
    unsafe extern "C" fn(*const mamba_registry::MbValue, usize) -> mamba_registry::MbValue;

fn register_getter(type_name: &str, attr: &str, getter: NativeGetter) {
    if let Some(o) = mamba_registry::ops::OBJECT_OPS.get() {
        (o.register_getter)(type_name, attr, getter);
    }
}

/// The primary `mambalibs.dataclasses` native module descriptor.
pub struct SchemaMambaModule;

impl MambaModule for SchemaMambaModule {
    fn name(&self) -> &'static str {
        "mambalibs.dataclasses"
    }

    fn doc(&self) -> &'static str {
        "Mamba-native dataclass and schema model interface"
    }

    fn register(&self, r: &mut ModuleRegistrar) {
        register_schema_surface(r);
    }
}

/// Legacy import alias kept for callers that still use `cclab_schema_mamba`.
pub struct CclabSchemaMambaCompatModule;

impl MambaModule for CclabSchemaMambaCompatModule {
    fn name(&self) -> &'static str {
        "cclab_schema_mamba"
    }

    fn doc(&self) -> &'static str {
        "Compatibility alias for mambalibs.dataclasses"
    }

    fn register(&self, r: &mut ModuleRegistrar) {
        register_schema_surface(r);
    }
}

/// Register [`SchemaMambaModule`] into the global `MAMBA_MODULES` slice at
/// link time.  Any binary that links this crate will automatically expose
/// the `mambalibs.dataclasses` module to Mamba scripts.
#[distributed_slice(MAMBA_MODULES)]
static SCHEMA_MAMBA_MODULE: &dyn MambaModule = &SchemaMambaModule;

#[distributed_slice(MAMBA_MODULES)]
static CCLAB_SCHEMA_MAMBA_COMPAT_MODULE: &dyn MambaModule = &CclabSchemaMambaCompatModule;
