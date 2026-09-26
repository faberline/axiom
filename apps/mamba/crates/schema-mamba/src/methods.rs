// MbValue is a newtype around u64; the JIT passes it by value as a 64-bit word.
// The `improper_ctypes_definitions` lint fires because MbValue lacks #[repr(C)],
// but this is safe in practice for our prototype.
#![allow(improper_ctypes_definitions)]

//! FFI functions exposed by `mambalibs.dataclasses` to Mamba scripts.
//!
//! All functions follow the Mamba native-call ABI:
//! ```text
//! extern "C" fn name(args: *const MbValue, nargs: usize) -> MbValue
//! ```
//!
//! # Exposed API
//!
//! | Symbol                     | Mamba call                              |
//! |----------------------------|-----------------------------------------|
//! | `mb_schema_base_model_new` | `BaseModel.__init_subclass__(cls, **kw)` |
//! | `mb_schema_field`          | `Field(**kwargs)`                        |
//! | `mb_schema_validate`       | `model.__validate__(data_dict)`          |
//! | `mb_schema_model_validate` | `model.model_validate(data_dict)`        |
//! | `mb_schema_model_dump`     | `model.model_dump(data_dict)`            |
//! | `mb_schema_model_dump_json`| `model.model_dump_json(data_dict)`       |
//! | `mb_schema_model_validate_json` | `model.model_validate_json(json_text)` |
//! | `mb_schema_field_validator`| `@field_validator(field, fn)`            |
//! | `mb_schema_to_json_schema` | `model.__json_schema__()`                |

use mamba_registry::convert::{
    mb_unwrap_native_mut, mb_unwrap_native_ref, mb_wrap_native_typed,
};
use mamba_registry::MbValue;

use schema::coercion::{apply_coercion, CoercionMode};
use schema::constraints::FieldDescriptor;
use schema::errors::{ValidationContext, ValidationErrors};
use schema::types::{TypeDescriptor, Value as SchemaValue};
use schema::validators::{validate, validate_with_context};
use std::cell::RefCell;
use std::collections::HashMap;

use crate::types::{
    field_descriptor_from_spec, field_from_kwargs, mb_to_schema_value, MbBaseModel,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

type NativeFn = unsafe extern "C" fn(*const MbValue, usize) -> MbValue;

#[derive(Debug, Clone, Copy)]
struct ModelOptions {
    coercion: CoercionMode,
    by_alias: bool,
}

thread_local! {
    static ACTIVE_MODEL: RefCell<Option<MbValue>> = const { RefCell::new(None) };
}

/// Read `args[idx]` safely, returning `MbValue::none()` if out-of-bounds.
#[inline]
unsafe fn arg(args: *const MbValue, nargs: usize, idx: usize) -> MbValue {
    if idx < nargs {
        unsafe { *args.add(idx) }
    } else {
        MbValue::none()
    }
}

/// Return an error string as an opaque PTR handle (MbObject).
fn error_value(msg: &str) -> MbValue {
    (mamba_registry::ops().str_new)(msg)
}

fn native_func(func: NativeFn) -> MbValue {
    MbValue::from_func(func as usize)
}

fn bind_model(receiver: MbValue) {
    ACTIVE_MODEL.with(|slot| {
        *slot.borrow_mut() = Some(receiver);
    });
}

fn active_model() -> Option<MbValue> {
    ACTIVE_MODEL.with(|slot| *slot.borrow())
}

fn add_field_to_model(model_value: MbValue, field_value: MbValue) -> MbValue {
    let Some(model) = (unsafe { mb_unwrap_native_mut::<MbBaseModel>(model_value) }) else {
        return error_value("add_field: invalid model handle");
    };
    let Some(field) = (unsafe { mb_unwrap_native_ref::<FieldDescriptor>(field_value) }) else {
        return error_value("add_field: invalid field handle");
    };
    model.add_field(field.clone());
    model_value
}

fn field_descriptors_from_map(
    fields_value: MbValue,
    api_name: &str,
) -> Result<Vec<FieldDescriptor>, String> {
    if fields_value.is_none() {
        return Ok(Vec::new());
    }

    let Some(entries) = (mamba_registry::ops().dict_iter_str_items)(fields_value) else {
        return Err(format!("{api_name}: fields must be a dict"));
    };

    let mut fields = Vec::with_capacity(entries.len());
    for (name, spec) in entries {
        let Some(field) = field_descriptor_from_spec(&name, spec) else {
            return Err(format!(
                "{api_name}: unsupported field spec for field '{name}'"
            ));
        };
        fields.push(field);
    }
    Ok(fields)
}

fn add_fields_to_model(model_value: MbValue, fields_value: MbValue, api_name: &str) -> MbValue {
    let fields = match field_descriptors_from_map(fields_value, api_name) {
        Ok(fields) => fields,
        Err(err) => return error_value(&err),
    };

    let Some(model) = (unsafe { mb_unwrap_native_mut::<MbBaseModel>(model_value) }) else {
        return error_value(&format!("{api_name}: invalid model handle"));
    };
    for field in fields {
        model.add_field(field);
    }
    model_value
}

// ── mb_schema_base_model_new ──────────────────────────────────────────────────

/// Create a new `BaseModel` definition.
///
/// # ABI
/// ```text
/// args[0] = class_name  (MbValue::Ptr → heap String)
/// ```
/// Returns an opaque PTR to [`MbBaseModel`].
///
/// # Mamba usage
/// ```python
/// class UserCreate(BaseModel):
///     pass
/// ```
#[no_mangle]
pub unsafe extern "C" fn mb_schema_base_model_new(args: *const MbValue, nargs: usize) -> MbValue {
    let name_val = unsafe { arg(args, nargs, 0) };
    let fields_val = unsafe { arg(args, nargs, 1) };
    let name = read_str(name_val).unwrap_or_else(|| "AnonymousModel".to_string());
    let fields = match field_descriptors_from_map(fields_val, "BaseModel") {
        Ok(fields) => fields,
        Err(err) => return error_value(&err),
    };
    let mut model = MbBaseModel::new(name);
    for field in fields {
        model.add_field(field);
    }
    mb_wrap_native_typed("BaseModel", model)
}

/// Create a new `BaseModel` definition from a field spec map.
///
/// # ABI
/// ```text
/// args[0] = model_name  (MbValue::Ptr → heap String)
/// args[1] = fields      (MbValue::Ptr → runtime dict)
/// ```
///
/// # Mamba usage
/// ```python
/// Item = create_model("Item", {"name": {"type": "str"}})
/// ```
#[no_mangle]
pub unsafe extern "C" fn mb_schema_create_model(args: *const MbValue, nargs: usize) -> MbValue {
    unsafe { mb_schema_base_model_new(args, nargs) }
}

// ── mb_schema_field ───────────────────────────────────────────────────────────

/// Define a schema field from keyword arguments.
///
/// # ABI
/// ```text
/// args[0] = field_name  (MbValue::Ptr → heap String)
/// args[1] = kwargs      (MbValue::Ptr → runtime dict)
/// ```
/// Returns an opaque PTR to a [`schema::constraints::FieldDescriptor`].
///
/// # Mamba usage
/// ```python
/// username: str = Field(min_length=3, description="Login name")
/// ```
#[no_mangle]
pub unsafe extern "C" fn mb_schema_field(args: *const MbValue, nargs: usize) -> MbValue {
    let name_val = unsafe { arg(args, nargs, 0) };
    let mut kwargs = unsafe { arg(args, nargs, 1) };

    let name = read_str(name_val).unwrap_or_default();
    if nargs == 1 && name.is_empty() {
        kwargs = name_val;
    }
    let mf = field_from_kwargs(&name, kwargs);
    let fd = mf.into_field_descriptor();
    mb_wrap_native_typed("Field", fd)
}

// ── mb_schema_model_add_field ────────────────────────────────────────────────

/// Register a field descriptor on a `BaseModel`.
///
/// # ABI
/// ```text
/// args[0] = model  (MbValue::Ptr → MbBaseModel)
/// args[1] = field  (MbValue::Ptr → FieldDescriptor)
/// ```
/// Returns the model handle for chaining.
#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_add_field(args: *const MbValue, nargs: usize) -> MbValue {
    let model_val = unsafe { arg(args, nargs, 0) };
    let field_val = unsafe { arg(args, nargs, 1) };
    add_field_to_model(model_val, field_val)
}

/// Register multiple field specs on a `BaseModel`.
#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_add_fields(args: *const MbValue, nargs: usize) -> MbValue {
    let model_val = unsafe { arg(args, nargs, 0) };
    let fields_val = unsafe { arg(args, nargs, 1) };
    add_fields_to_model(model_val, fields_val, "add_fields")
}

// ── mb_schema_validate ────────────────────────────────────────────────────────

/// Validate a data object against a `BaseModel`.
///
/// # ABI
/// ```text
/// args[0] = model  (MbValue::Ptr → MbBaseModel)
/// args[1] = data   (MbValue::Ptr → runtime dict)
/// ```
/// Returns:
/// - `MbValue::Bool(true)`  on success
/// - `MbValue::Ptr → String` (error message) on validation failure
///
/// # Mamba usage
/// ```python
/// UserCreate.validate({"username": "alice"})
/// ```
#[no_mangle]
pub unsafe extern "C" fn mb_schema_validate(args: *const MbValue, nargs: usize) -> MbValue {
    let model_val = unsafe { arg(args, nargs, 0) };
    let data_val = unsafe { arg(args, nargs, 1) };

    let Some(model) = (unsafe { mb_unwrap_native_ref::<MbBaseModel>(model_val) }) else {
        return error_value("validate: invalid model handle");
    };

    // Convert the data to a SchemaValue::Object.
    let data = build_schema_object(data_val, model);

    match validate(&data, &model.type_descriptor()) {
        Ok(_) => MbValue::from_bool(true),
        Err(errs) => error_value(&format!("ValidationError: {errs:?}")),
    }
}

// ── mb_schema_model_validate / model_dump ────────────────────────────────────

/// Parse and normalize a data object against a `BaseModel`.
///
/// This is a `mambalibs.dataclasses` extension modeled after Pydantic's
/// `model_validate`. It intentionally leaves the existing `validate()` API
/// untouched: `validate()` still returns bool/string, while this returns a
/// normalized dict/string.
///
/// # ABI
/// ```text
/// args[0] = model    (MbValue::Ptr → MbBaseModel)
/// args[1] = data     (MbValue::Ptr → runtime dict)
/// args[2] = options  (optional dict; {"strict": bool})
/// ```
/// Returns:
/// - `MbValue::Ptr → dict` on success
/// - `MbValue::Ptr → String` (error message) on validation failure
#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_validate(args: *const MbValue, nargs: usize) -> MbValue {
    let model_val = unsafe { arg(args, nargs, 0) };
    let data_val = unsafe { arg(args, nargs, 1) };
    let options_val = unsafe { arg(args, nargs, 2) };
    model_validate_value(model_val, data_val, options_val, "model_validate", false)
}

/// Compatibility alias for callers familiar with Pydantic v1.
#[no_mangle]
pub unsafe extern "C" fn mb_schema_parse_obj(args: *const MbValue, nargs: usize) -> MbValue {
    unsafe { mb_schema_model_validate(args, nargs) }
}

/// Parse and normalize a JSON string against a `BaseModel`.
#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_validate_json(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let model_val = unsafe { arg(args, nargs, 0) };
    let json_val = unsafe { arg(args, nargs, 1) };
    let options_val = unsafe { arg(args, nargs, 2) };
    model_validate_json_value(model_val, json_val, options_val, "model_validate_json")
}

/// Compatibility alias for callers familiar with Pydantic v1 raw parsing.
#[no_mangle]
pub unsafe extern "C" fn mb_schema_parse_raw(args: *const MbValue, nargs: usize) -> MbValue {
    let model_val = unsafe { arg(args, nargs, 0) };
    let json_val = unsafe { arg(args, nargs, 1) };
    let options_val = unsafe { arg(args, nargs, 2) };
    model_validate_json_value(model_val, json_val, options_val, "parse_raw")
}

/// Dump a normalized dict. In this prototype API the model definition is the
/// callable object, so `model_dump(data)` is equivalent to `model_validate(data)`.
#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_dump(args: *const MbValue, nargs: usize) -> MbValue {
    let model_val = unsafe { arg(args, nargs, 0) };
    let data_val = unsafe { arg(args, nargs, 1) };
    let options_val = unsafe { arg(args, nargs, 2) };
    model_validate_value(model_val, data_val, options_val, "model_dump", true)
}

/// Dump a normalized object as compact JSON.
#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_dump_json(args: *const MbValue, nargs: usize) -> MbValue {
    let model_val = unsafe { arg(args, nargs, 0) };
    let data_val = unsafe { arg(args, nargs, 1) };
    let options_val = unsafe { arg(args, nargs, 2) };

    let Some(model) = (unsafe { mb_unwrap_native_ref::<MbBaseModel>(model_val) }) else {
        return error_value("model_dump_json: invalid model handle");
    };

    let options = parse_options(options_val);
    let data = build_normalized_schema_object(data_val, model, options.coercion);
    match validate(&data, &model.type_descriptor()) {
        Ok(_) => {
            let output = model_output_value(&data, model, options.by_alias);
            let json = schema_value_to_json_value(&output);
            (mamba_registry::ops().str_new)(
                &serde_json::to_string(&json).unwrap_or_else(|_| "{}".to_string()),
            )
        }
        Err(errs) => error_value(&format!("ValidationError: {errs:?}")),
    }
}

/// Pydantic-style schema alias for `to_json_schema`.
#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_json_schema(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    unsafe { mb_schema_to_json_schema(args, nargs) }
}

/// Return FastAPI/Pydantic-style detail JSON for invalid model data.
///
/// This is a Rust-side helper for framework adapters. The public
/// `model_dump_json` Mamba behavior stays string-based for compatibility.
pub fn model_validation_detail_json(
    model_val: MbValue,
    data_val: MbValue,
    location: &str,
) -> Option<String> {
    let model = unsafe { mb_unwrap_native_ref::<MbBaseModel>(model_val) }?;
    let data =
        build_normalized_schema_value(mb_to_schema_value(data_val)?, model, CoercionMode::Lax);
    let mut ctx = ValidationContext::with_location(location);
    match validate_with_context(&data, &model.type_descriptor(), &mut ctx) {
        Ok(_) => Some("[]".to_string()),
        Err(errs) => Some(validation_detail_json(&errs)),
    }
}

/// Normalize JSON text against a model and return compact JSON output.
///
/// Used by HTTP adapters that receive raw request bodies before a Mamba dict
/// exists. Public Mamba callers should use `model_validate_json` / `parse_raw`.
pub fn model_dump_json_from_json_text(
    model_val: MbValue,
    text: &str,
    options_val: MbValue,
) -> Result<String, String> {
    let model = unsafe { mb_unwrap_native_ref::<MbBaseModel>(model_val) }
        .ok_or_else(|| "model_validate_json: invalid model handle".to_string())?;
    let input = schema_value_from_json_text(text)?;
    let options = parse_options(options_val);
    let output = model_validate_schema_value(model, input, options, options.by_alias)
        .map_err(|errs| format!("ValidationError: {errs:?}"))?;
    let json = schema_value_to_json_value(&output);
    serde_json::to_string(&json).map_err(|err| format!("model_validate_json: {err}"))
}

/// Return FastAPI-style validation details for JSON text model validation.
pub fn model_validation_detail_json_from_json_text(
    model_val: MbValue,
    text: &str,
    location: &str,
) -> Option<String> {
    let model = unsafe { mb_unwrap_native_ref::<MbBaseModel>(model_val) }?;
    let input = match schema_value_from_json_text(text) {
        Ok(value) => value,
        Err(err) => return Some(json_decode_detail_json(location, &err)),
    };
    let data = build_normalized_schema_value(input, model, CoercionMode::Lax);
    let mut ctx = ValidationContext::with_location(location);
    match validate_with_context(&data, &model.type_descriptor(), &mut ctx) {
        Ok(_) => Some("[]".to_string()),
        Err(errs) => Some(validation_detail_json(&errs)),
    }
}

// ── Bound BaseModel methods ──────────────────────────────────────────────────

pub unsafe extern "C" fn get_model_add_field(args: *const MbValue, nargs: usize) -> MbValue {
    let receiver = unsafe { arg(args, nargs, 0) };
    bind_model(receiver);
    native_func(mb_schema_model_add_field_bound)
}

pub unsafe extern "C" fn get_model_add_fields(args: *const MbValue, nargs: usize) -> MbValue {
    let receiver = unsafe { arg(args, nargs, 0) };
    bind_model(receiver);
    native_func(mb_schema_model_add_fields_bound)
}

pub unsafe extern "C" fn get_model_validate(args: *const MbValue, nargs: usize) -> MbValue {
    let receiver = unsafe { arg(args, nargs, 0) };
    bind_model(receiver);
    native_func(mb_schema_validate_bound)
}

pub unsafe extern "C" fn get_model_model_validate(args: *const MbValue, nargs: usize) -> MbValue {
    let receiver = unsafe { arg(args, nargs, 0) };
    bind_model(receiver);
    native_func(mb_schema_model_validate_bound)
}

pub unsafe extern "C" fn get_model_parse_obj(args: *const MbValue, nargs: usize) -> MbValue {
    let receiver = unsafe { arg(args, nargs, 0) };
    bind_model(receiver);
    native_func(mb_schema_parse_obj_bound)
}

pub unsafe extern "C" fn get_model_model_validate_json(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let receiver = unsafe { arg(args, nargs, 0) };
    bind_model(receiver);
    native_func(mb_schema_model_validate_json_bound)
}

pub unsafe extern "C" fn get_model_parse_raw(args: *const MbValue, nargs: usize) -> MbValue {
    let receiver = unsafe { arg(args, nargs, 0) };
    bind_model(receiver);
    native_func(mb_schema_parse_raw_bound)
}

pub unsafe extern "C" fn get_model_model_dump(args: *const MbValue, nargs: usize) -> MbValue {
    let receiver = unsafe { arg(args, nargs, 0) };
    bind_model(receiver);
    native_func(mb_schema_model_dump_bound)
}

pub unsafe extern "C" fn get_model_model_dump_json(args: *const MbValue, nargs: usize) -> MbValue {
    let receiver = unsafe { arg(args, nargs, 0) };
    bind_model(receiver);
    native_func(mb_schema_model_dump_json_bound)
}

pub unsafe extern "C" fn get_model_to_json_schema(args: *const MbValue, nargs: usize) -> MbValue {
    let receiver = unsafe { arg(args, nargs, 0) };
    bind_model(receiver);
    native_func(mb_schema_to_json_schema_bound)
}

pub unsafe extern "C" fn get_model_model_json_schema(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let receiver = unsafe { arg(args, nargs, 0) };
    bind_model(receiver);
    native_func(mb_schema_model_json_schema_bound)
}

#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_add_field_bound(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let Some(model_val) = active_model() else {
        return error_value("add_field: no active model receiver");
    };
    let field_val = unsafe { arg(args, nargs, 0) };
    add_field_to_model(model_val, field_val)
}

#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_add_fields_bound(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let Some(model_val) = active_model() else {
        return error_value("add_fields: no active model receiver");
    };
    let fields_val = unsafe { arg(args, nargs, 0) };
    add_fields_to_model(model_val, fields_val, "add_fields")
}

#[no_mangle]
pub unsafe extern "C" fn mb_schema_validate_bound(args: *const MbValue, nargs: usize) -> MbValue {
    let Some(model_val) = active_model() else {
        return error_value("validate: no active model receiver");
    };
    let data_val = unsafe { arg(args, nargs, 0) };
    let forwarded = [model_val, data_val];
    unsafe { mb_schema_validate(forwarded.as_ptr(), forwarded.len()) }
}

#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_validate_bound(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let Some(model_val) = active_model() else {
        return error_value("model_validate: no active model receiver");
    };
    let data_val = unsafe { arg(args, nargs, 0) };
    let options_val = unsafe { arg(args, nargs, 1) };
    let forwarded = [model_val, data_val, options_val];
    unsafe { mb_schema_model_validate(forwarded.as_ptr(), forwarded.len()) }
}

#[no_mangle]
pub unsafe extern "C" fn mb_schema_parse_obj_bound(args: *const MbValue, nargs: usize) -> MbValue {
    let Some(model_val) = active_model() else {
        return error_value("parse_obj: no active model receiver");
    };
    let data_val = unsafe { arg(args, nargs, 0) };
    let options_val = unsafe { arg(args, nargs, 1) };
    let forwarded = [model_val, data_val, options_val];
    unsafe { mb_schema_parse_obj(forwarded.as_ptr(), forwarded.len()) }
}

#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_validate_json_bound(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let Some(model_val) = active_model() else {
        return error_value("model_validate_json: no active model receiver");
    };
    let json_val = unsafe { arg(args, nargs, 0) };
    let options_val = unsafe { arg(args, nargs, 1) };
    let forwarded = [model_val, json_val, options_val];
    unsafe { mb_schema_model_validate_json(forwarded.as_ptr(), forwarded.len()) }
}

#[no_mangle]
pub unsafe extern "C" fn mb_schema_parse_raw_bound(args: *const MbValue, nargs: usize) -> MbValue {
    let Some(model_val) = active_model() else {
        return error_value("parse_raw: no active model receiver");
    };
    let json_val = unsafe { arg(args, nargs, 0) };
    let options_val = unsafe { arg(args, nargs, 1) };
    let forwarded = [model_val, json_val, options_val];
    unsafe { mb_schema_parse_raw(forwarded.as_ptr(), forwarded.len()) }
}

#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_dump_bound(args: *const MbValue, nargs: usize) -> MbValue {
    let Some(model_val) = active_model() else {
        return error_value("model_dump: no active model receiver");
    };
    let data_val = unsafe { arg(args, nargs, 0) };
    let options_val = unsafe { arg(args, nargs, 1) };
    let forwarded = [model_val, data_val, options_val];
    unsafe { mb_schema_model_dump(forwarded.as_ptr(), forwarded.len()) }
}

#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_dump_json_bound(
    args: *const MbValue,
    nargs: usize,
) -> MbValue {
    let Some(model_val) = active_model() else {
        return error_value("model_dump_json: no active model receiver");
    };
    let data_val = unsafe { arg(args, nargs, 0) };
    let options_val = unsafe { arg(args, nargs, 1) };
    let forwarded = [model_val, data_val, options_val];
    unsafe { mb_schema_model_dump_json(forwarded.as_ptr(), forwarded.len()) }
}

#[no_mangle]
pub unsafe extern "C" fn mb_schema_to_json_schema_bound(
    _args: *const MbValue,
    _nargs: usize,
) -> MbValue {
    let Some(model_val) = active_model() else {
        return error_value("json_schema: no active model receiver");
    };
    let forwarded = [model_val];
    unsafe { mb_schema_to_json_schema(forwarded.as_ptr(), forwarded.len()) }
}

#[no_mangle]
pub unsafe extern "C" fn mb_schema_model_json_schema_bound(
    _args: *const MbValue,
    _nargs: usize,
) -> MbValue {
    let Some(model_val) = active_model() else {
        return error_value("model_json_schema: no active model receiver");
    };
    let forwarded = [model_val];
    unsafe { mb_schema_model_json_schema(forwarded.as_ptr(), forwarded.len()) }
}

// ── mb_schema_field_validator ─────────────────────────────────────────────────

/// Register a field-level validator callback on a model.
///
/// # ABI
/// ```text
/// args[0] = model       (MbValue::Ptr → MbBaseModel)
/// args[1] = field_name  (MbValue::Ptr → heap String)
/// args[2] = validator   (MbValue::Func → fn ptr)
/// ```
/// Returns `MbValue::none()`.
///
/// # Mamba usage
/// ```python
/// @field_validator("username")
/// def check_username(v: str) -> str: ...
/// ```
#[no_mangle]
pub unsafe extern "C" fn mb_schema_field_validator(args: *const MbValue, nargs: usize) -> MbValue {
    let model_val = unsafe { arg(args, nargs, 0) };
    let fname_val = unsafe { arg(args, nargs, 1) };
    // args[2] is the callback function handle; stored opaquely (full FFI
    // callback support is beyond this prototype scope).

    let Some(model) = (unsafe { mb_unwrap_native_mut::<MbBaseModel>(model_val) }) else {
        return MbValue::none();
    };

    // Mark the field as having a validator via a description sentinel.
    if let Some(fname) = read_str(fname_val) {
        if let Some(fd) = model.fields.get_mut(&fname) {
            let note = " [has_validator]";
            let existing = fd.description.get_or_insert_with(String::new);
            if !existing.ends_with(note) {
                existing.push_str(note);
            }
        }
    }

    MbValue::none()
}

// ── mb_schema_to_json_schema ──────────────────────────────────────────────────

/// Export a model definition as a JSON Schema string.
///
/// # ABI
/// ```text
/// args[0] = model  (MbValue::Ptr → MbBaseModel)
/// ```
/// Returns `MbValue::Ptr → heap String` (JSON text).
///
/// # Mamba usage
/// ```python
/// UserCreate.__json_schema__()
/// ```
#[no_mangle]
pub unsafe extern "C" fn mb_schema_to_json_schema(args: *const MbValue, nargs: usize) -> MbValue {
    let model_val = unsafe { arg(args, nargs, 0) };

    let Some(model) = (unsafe { mb_unwrap_native_ref::<MbBaseModel>(model_val) }) else {
        return error_value("json_schema: invalid model handle");
    };

    (mamba_registry::ops().str_new)(&model.to_json_schema_string())
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Read a heap `String` from a PTR-typed `MbValue` (pointer to MbObject).
fn read_str(v: MbValue) -> Option<String> {
    (mamba_registry::ops().str_read)(v)
}

/// Convert a Mamba data `MbValue` to a `SchemaValue::Object`.
///
/// The `data` value is expected to be a runtime dict readable through
/// `ObjectOps`. Falls back to an empty object when the value is `None` or
/// unrecognised.
fn build_schema_object(data: MbValue, model: &MbBaseModel) -> SchemaValue {
    if data.is_none() {
        return SchemaValue::Object(vec![]);
    }

    let Some(items) = (mamba_registry::ops().dict_iter_str_items)(data) else {
        return SchemaValue::Object(vec![]);
    };

    let mut out: Vec<(String, SchemaValue)> = Vec::new();
    for (k, v) in items {
        if !model.fields.is_empty() && !model.fields.contains_key(&k) {
            continue; // enforce declared-field filtering
        }
        if let Some(sv) = mb_to_schema_value(v) {
            out.push((k.clone(), sv));
        }
    }

    SchemaValue::Object(out)
}

fn model_validate_value(
    model_val: MbValue,
    data_val: MbValue,
    options_val: MbValue,
    api_name: &str,
    allow_by_alias: bool,
) -> MbValue {
    let Some(model) = (unsafe { mb_unwrap_native_ref::<MbBaseModel>(model_val) }) else {
        return error_value(&format!("{api_name}: invalid model handle"));
    };

    let mut options = parse_options(options_val);
    if !allow_by_alias {
        options.by_alias = false;
    }
    let Some(input) = mb_to_schema_value(data_val) else {
        return error_value(&format!("{api_name}: unsupported input value"));
    };

    match model_validate_schema_value(model, input, options, options.by_alias) {
        Ok(output) => schema_value_to_mb_value(&output),
        Err(errs) => error_value(&format!("ValidationError: {errs:?}")),
    }
}

fn model_validate_json_value(
    model_val: MbValue,
    json_val: MbValue,
    options_val: MbValue,
    api_name: &str,
) -> MbValue {
    let Some(model) = (unsafe { mb_unwrap_native_ref::<MbBaseModel>(model_val) }) else {
        return error_value(&format!("{api_name}: invalid model handle"));
    };
    let Some(text) = read_str(json_val) else {
        return error_value(&format!("ValidationError: {api_name} expected JSON text"));
    };
    let input = match schema_value_from_json_text(&text) {
        Ok(value) => value,
        Err(err) => return error_value(&err),
    };
    let mut options = parse_options(options_val);
    options.by_alias = false;

    match model_validate_schema_value(model, input, options, false) {
        Ok(output) => schema_value_to_mb_value(&output),
        Err(errs) => error_value(&format!("ValidationError: {errs:?}")),
    }
}

fn parse_options(options: MbValue) -> ModelOptions {
    let strict = (mamba_registry::ops().dict_get_str)(options, "strict")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let by_alias = (mamba_registry::ops().dict_get_str)(options, "by_alias")
        .or_else(|| (mamba_registry::ops().dict_get_str)(options, "byAlias"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    let coercion = if strict {
        CoercionMode::Strict
    } else {
        CoercionMode::Lax
    };

    ModelOptions { coercion, by_alias }
}

fn build_normalized_schema_object(
    data: MbValue,
    model: &MbBaseModel,
    mode: CoercionMode,
) -> SchemaValue {
    let Some(input) = mb_to_schema_value(data) else {
        return SchemaValue::Object(vec![]);
    };
    build_normalized_schema_value(input, model, mode)
}

fn build_normalized_schema_value(
    input: SchemaValue,
    model: &MbBaseModel,
    mode: CoercionMode,
) -> SchemaValue {
    let TypeDescriptor::Object { fields, .. } = model.type_descriptor() else {
        return SchemaValue::Object(vec![]);
    };
    match input {
        SchemaValue::Object(pairs) => normalize_object_fields(pairs, &fields, mode),
        other => normalize_schema_value(other, &model.type_descriptor(), mode),
    }
}

fn model_validate_schema_value(
    model: &MbBaseModel,
    input: SchemaValue,
    options: ModelOptions,
    by_alias: bool,
) -> Result<SchemaValue, ValidationErrors> {
    let data = build_normalized_schema_value(input, model, options.coercion);
    validate(&data, &model.type_descriptor())?;
    Ok(model_output_value(&data, model, by_alias))
}

fn model_output_value(value: &SchemaValue, model: &MbBaseModel, by_alias: bool) -> SchemaValue {
    if !by_alias {
        return value.clone();
    }

    let SchemaValue::Object(fields) = value else {
        return value.clone();
    };

    let output = fields
        .iter()
        .map(|(name, value)| {
            let output_name = model
                .fields
                .get(name)
                .map(|field| field.serialization_name().to_string())
                .unwrap_or_else(|| name.clone());
            (output_name, value.clone())
        })
        .collect();

    SchemaValue::Object(output)
}

fn schema_value_from_json_text(text: &str) -> Result<SchemaValue, String> {
    let value = serde_json::from_str::<serde_json::Value>(text)
        .map_err(|err| format!("ValidationError: invalid JSON: {err}"))?;
    Ok(json_value_to_schema_value(value))
}

fn json_value_to_schema_value(value: serde_json::Value) -> SchemaValue {
    match value {
        serde_json::Value::Null => SchemaValue::Null,
        serde_json::Value::Bool(value) => SchemaValue::Bool(value),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                SchemaValue::Int(value)
            } else if let Some(value) = value.as_f64() {
                SchemaValue::Float(value)
            } else {
                SchemaValue::Null
            }
        }
        serde_json::Value::String(value) => SchemaValue::String(value),
        serde_json::Value::Array(items) => {
            SchemaValue::List(items.into_iter().map(json_value_to_schema_value).collect())
        }
        serde_json::Value::Object(fields) => SchemaValue::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, json_value_to_schema_value(value)))
                .collect(),
        ),
    }
}

fn normalize_object_fields(
    input_pairs: Vec<(String, SchemaValue)>,
    fields: &[FieldDescriptor],
    mode: CoercionMode,
) -> SchemaValue {
    let input: HashMap<String, SchemaValue> = input_pairs.into_iter().collect();
    let mut fields: Vec<&FieldDescriptor> = fields.iter().collect();
    fields.sort_by(|a, b| a.name.cmp(&b.name));

    let mut out = Vec::with_capacity(fields.len());
    for field in fields {
        let value = field
            .all_validation_names()
            .into_iter()
            .find_map(|candidate| input.get(candidate).cloned())
            .or_else(|| field.default.clone());
        let Some(value) = value else {
            continue;
        };
        out.push((
            field.name.clone(),
            normalize_schema_value(value, &field.type_desc, mode),
        ));
    }

    SchemaValue::Object(out)
}

fn normalize_schema_value(
    value: SchemaValue,
    type_desc: &TypeDescriptor,
    mode: CoercionMode,
) -> SchemaValue {
    let coerced = apply_coercion(&value, type_desc, mode).unwrap_or(value);

    match (coerced, type_desc) {
        (
            SchemaValue::Object(fields),
            TypeDescriptor::Object {
                fields: schema_fields,
                ..
            },
        ) => normalize_object_fields(fields, schema_fields, mode),
        (
            SchemaValue::List(items),
            TypeDescriptor::List {
                items: item_type, ..
            },
        ) => SchemaValue::List(
            items
                .into_iter()
                .map(|item| normalize_schema_value(item, item_type, mode))
                .collect(),
        ),
        (SchemaValue::Null, TypeDescriptor::Optional(_)) => SchemaValue::Null,
        (value, TypeDescriptor::Optional(inner)) => normalize_schema_value(value, inner, mode),
        (SchemaValue::Null, TypeDescriptor::Union { nullable: true, .. }) => SchemaValue::Null,
        (value, TypeDescriptor::Union { variants, .. }) => variants
            .iter()
            .find_map(|variant| {
                let normalized = normalize_schema_value(value.clone(), variant, mode);
                validate(&normalized, variant).ok().map(|_| normalized)
            })
            .unwrap_or(value),
        (value, _) => value,
    }
}

fn schema_value_to_mb_value(value: &SchemaValue) -> MbValue {
    let ops = mamba_registry::ops();
    match value {
        SchemaValue::Null => MbValue::none(),
        SchemaValue::Bool(value) => MbValue::from_bool(*value),
        SchemaValue::Int(value) => MbValue::from_int(*value),
        SchemaValue::Float(value) => MbValue::from_float(*value),
        SchemaValue::String(value) => (ops.str_new)(value),
        SchemaValue::Bytes(bytes) => (ops.list_new)(
            bytes
                .iter()
                .map(|byte| MbValue::from_int(i64::from(*byte)))
                .collect(),
        ),
        SchemaValue::List(items) => {
            let values = items.iter().map(schema_value_to_mb_value).collect();
            (ops.list_new)(values)
        }
        SchemaValue::Object(fields) => {
            let dict = (ops.dict_new)();
            for (key, value) in fields {
                (ops.dict_insert_str)(dict, key, schema_value_to_mb_value(value));
            }
            dict
        }
    }
}

fn schema_value_to_json_value(value: &SchemaValue) -> serde_json::Value {
    match value {
        SchemaValue::Null => serde_json::Value::Null,
        SchemaValue::Bool(value) => serde_json::Value::Bool(*value),
        SchemaValue::Int(value) => serde_json::Value::Number((*value).into()),
        SchemaValue::Float(value) => serde_json::Number::from_f64(*value)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        SchemaValue::String(value) => serde_json::Value::String(value.clone()),
        SchemaValue::Bytes(bytes) => serde_json::Value::Array(
            bytes
                .iter()
                .map(|byte| serde_json::Value::Number(i64::from(*byte).into()))
                .collect(),
        ),
        SchemaValue::List(items) => {
            serde_json::Value::Array(items.iter().map(schema_value_to_json_value).collect())
        }
        SchemaValue::Object(fields) => serde_json::Value::Object(
            fields
                .iter()
                .map(|(key, value)| (key.clone(), schema_value_to_json_value(value)))
                .collect(),
        ),
    }
}

fn validation_detail_json(errors: &ValidationErrors) -> String {
    let details = errors
        .as_slice()
        .iter()
        .map(|error| {
            serde_json::json!({
                "loc": detail_loc(&error.location, &error.field),
                "msg": error.message,
                "type": error.error_type.to_string(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&details).unwrap_or_else(|_| "[]".to_string())
}

fn json_decode_detail_json(location: &str, msg: &str) -> String {
    serde_json::to_string(&vec![serde_json::json!({
        "loc": detail_loc(location, ""),
        "msg": msg,
        "type": "value_error.jsondecode",
    })])
    .unwrap_or_else(|_| "[]".to_string())
}

fn detail_loc(location: &str, field: &str) -> Vec<serde_json::Value> {
    let mut loc = Vec::new();
    if !location.is_empty() {
        loc.push(serde_json::Value::String(location.to_string()));
    }
    for segment in field.split('.').filter(|segment| !segment.is_empty()) {
        if let Some(index) = segment
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
            .and_then(|value| value.parse::<i64>().ok())
        {
            loc.push(serde_json::Value::Number(index.into()));
        } else {
            loc.push(serde_json::Value::String(segment.to_string()));
        }
    }
    loc
}
