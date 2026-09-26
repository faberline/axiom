//! Opaque types and helper functions for the `schema-mamba` FFI layer.
//!
//! We deliberately avoid implementing `IntoMbValue` / `FromMbValue` for
//! external types (which would violate the orphan rule).  Instead, this module
//! defines first-party types and conversion helper functions used by
//! [`super::methods`].

use mamba_registry::convert::{mb_unwrap_native_ref, native_type_name};
use mamba_registry::MbValue;
use schema::constraints::{
    FieldDescriptor, ListConstraints, NumericConstraints, StringConstraints, StringFormat,
};
use schema::types::TypeDescriptor;
use std::collections::HashMap;

// Re-export the schema Value type under our conventional alias.
pub use schema::types::Value as SchemaValue;

// ── MbBaseModel — opaque model definition ────────────────────────────────────

/// An opaque Mamba-visible handle to a schema model definition.
///
/// Created by [`crate::methods::mb_schema_base_model_new`] and passed as an
/// opaque PTR to the other `mb_schema_*` functions.
#[derive(Debug)]
pub struct MbBaseModel {
    /// Python class name (e.g. `"UserCreate"`).
    pub name: String,
    /// Field descriptors keyed by field name.
    pub fields: HashMap<String, FieldDescriptor>,
}

impl MbBaseModel {
    /// Create a new empty model definition.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            fields: HashMap::new(),
        }
    }

    /// Register a field descriptor on this model.
    pub fn add_field(&mut self, fd: FieldDescriptor) {
        self.fields.insert(fd.name.clone(), fd);
    }

    /// Build a deterministic object type descriptor for this model.
    pub fn type_descriptor(&self) -> TypeDescriptor {
        let mut fields: Vec<FieldDescriptor> = self.fields.values().cloned().collect();
        fields.sort_by(|a, b| a.name.cmp(&b.name));
        TypeDescriptor::Object {
            fields,
            additional: None,
        }
    }

    /// Export this model as compact JSON Schema text.
    pub fn to_json_schema_string(&self) -> String {
        let mut schema = schema::type_descriptor_to_json_schema(&self.type_descriptor());
        schema.title = Some(self.name.clone());
        let raw = schema.to_json();
        compact_json(&raw)
    }
}

// ── MbSchemaField — field definition helper ───────────────────────────────────

/// Intermediate representation of a field extracted from Mamba kwargs.
#[derive(Debug, Default)]
pub struct MbSchemaField {
    pub name: String,
    pub type_name: Option<String>,
    pub item_type_name: Option<String>,
    pub required: bool,
    pub default: Option<SchemaValue>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub examples: Vec<SchemaValue>,
    pub alias: Option<String>,
    pub validation_alias: Option<String>,
    pub serialization_alias: Option<String>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<String>,
    pub format: Option<StringFormat>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub exclusive_min_value: Option<f64>,
    pub exclusive_max_value: Option<f64>,
    pub multiple_of: Option<f64>,
    pub min_items: Option<usize>,
    pub max_items: Option<usize>,
    pub unique_items: bool,
    pub nullable: bool,
    pub deprecated: bool,
    pub read_only: bool,
    pub write_only: bool,
    pub nested_model_type: Option<TypeDescriptor>,
    pub items_model_type: Option<TypeDescriptor>,
}

impl MbSchemaField {
    /// Convert into a [`FieldDescriptor`] for schema registration.
    pub fn into_field_descriptor(self) -> FieldDescriptor {
        let type_desc = self.type_descriptor();
        FieldDescriptor {
            name: self.name,
            type_desc,
            required: self.required,
            default: self.default,
            title: self.title,
            description: self.description,
            examples: self.examples,
            alias: self.alias,
            validation_alias: self.validation_alias,
            serialization_alias: self.serialization_alias,
            private: false,
            init_only: false,
            deprecated: self.deprecated,
            read_only: self.read_only,
            write_only: self.write_only,
        }
    }

    fn type_descriptor(&self) -> TypeDescriptor {
        type_descriptor_from_name(
            self.type_name.as_deref(),
            self.item_type_name.as_deref(),
            self,
        )
    }
}

// ── Parse kwargs dict → MbSchemaField ─────────────────────────────────────────

/// Extract a [`MbSchemaField`] from a Mamba kwargs dict (opaque PTR wrapping a
/// `Vec<(String, MbValue)>` or similar).
///
/// Since Mamba dict internals are opaque, we read through the installed
/// `ObjectOps` dictionary hooks and fall back to defaults if the value is not
/// a recognised runtime dict.
///
/// Recognised kwargs keys:
/// - `"default"` — scalar default
/// - `"title"` — JSON Schema/OpenAPI field title
/// - `"description"` — documentation string (as heap String)
/// - `"examples"` / `"example"` — JSON Schema/OpenAPI examples
/// - `"deprecated"` — JSON Schema/OpenAPI deprecation marker
/// - `"read_only"` / `"readOnly"` — JSON Schema/OpenAPI read-only marker
/// - `"write_only"` / `"writeOnly"` — JSON Schema/OpenAPI write-only marker
/// - `"alias"` — validation and serialization alias
/// - `"validation_alias"` — input-only alias
/// - `"serialization_alias"` — output/schema alias
/// - `"type"` — scalar/list type name such as `str`, `int`, `bool`, `list[str]`
/// - `"model"` / `"schema_model"` — nested BaseModel handle for object fields
/// - `"items_model"` — nested BaseModel handle for list/array item fields
/// - `"items"` — item type when `"type"` is `"list"` or `"array"`
/// - `"min_length"` / `"max_length"` — int constraints
/// - `"pattern"` / `"regex"` — string pattern aliases
/// - `"min_value"` / `"max_value"` — inclusive float/int numeric constraints
/// - `"minimum"` / `"maximum"` — inclusive JSON Schema numeric aliases
/// - `"ge"` / `"le"` — inclusive Pydantic numeric aliases
/// - `"gt"` / `"lt"` — exclusive Pydantic numeric aliases
/// - `"exclusive_minimum"` / `"exclusiveMaximum"` — exclusive JSON Schema aliases
/// - `"multiple_of"` / `"multipleOf"` — numeric divisibility constraint
/// - `"min_items"` / `"max_items"` — list length constraints
/// - `"minItems"` / `"maxItems"` — JSON Schema list length aliases
/// - `"required"` — bool
pub fn field_from_kwargs(name: &str, kwargs: MbValue) -> MbSchemaField {
    let mut f = MbSchemaField {
        name: name.to_string(),
        required: true,
        ..Default::default()
    };

    if kwargs.is_none() {
        return f;
    }

    let ops = mamba_registry::ops();

    if let Some(dv) = (ops.dict_get_str)(kwargs, "default") {
        f.default = mb_to_schema_value(dv);
        f.required = false;
        if dv.is_none() {
            f.nullable = true;
        }
    }
    if let Some(rv) = (ops.dict_get_str)(kwargs, "required") {
        if let Some(b) = rv.as_bool() {
            f.required = b;
        }
    }
    if let Some(nullable) = (ops.dict_get_str)(kwargs, "nullable") {
        if let Some(b) = nullable.as_bool() {
            f.nullable = b;
        }
    }
    if let Some(optional) = (ops.dict_get_str)(kwargs, "optional") {
        if let Some(b) = optional.as_bool() {
            f.nullable = b;
        }
    }
    if let Some(tv) = (ops.dict_get_str)(kwargs, "type") {
        f.type_name = read_string(tv);
    }
    if let Some(model) =
        (ops.dict_get_str)(kwargs, "model").or_else(|| (ops.dict_get_str)(kwargs, "schema_model"))
    {
        f.nested_model_type = model_type_descriptor(model);
    }
    if let Some(model) = (ops.dict_get_str)(kwargs, "items_model") {
        f.items_model_type = model_type_descriptor(model);
    }
    if let Some(tv) = (ops.dict_get_str)(kwargs, "items") {
        f.item_type_name = read_string(tv);
    }
    if let Some(desc) = (ops.dict_get_str)(kwargs, "description") {
        f.description = read_string(desc);
    }
    if let Some(title) = (ops.dict_get_str)(kwargs, "title") {
        f.title = read_string(title);
    }
    if let Some(examples) = (ops.dict_get_str)(kwargs, "examples") {
        f.examples = examples_from_mb(examples);
    }
    if f.examples.is_empty() {
        if let Some(example) = (ops.dict_get_str)(kwargs, "example") {
            if let Some(value) = mb_to_schema_value(example) {
                f.examples.push(value);
            }
        }
    }
    if let Some(value) = (ops.dict_get_str)(kwargs, "deprecated") {
        f.deprecated = value.as_bool().unwrap_or(false);
    }
    if let Some(value) =
        (ops.dict_get_str)(kwargs, "read_only").or_else(|| (ops.dict_get_str)(kwargs, "readOnly"))
    {
        f.read_only = value.as_bool().unwrap_or(false);
    }
    if let Some(value) =
        (ops.dict_get_str)(kwargs, "write_only").or_else(|| (ops.dict_get_str)(kwargs, "writeOnly"))
    {
        f.write_only = value.as_bool().unwrap_or(false);
    }
    if let Some(alias) = (ops.dict_get_str)(kwargs, "alias") {
        f.alias = read_string(alias);
    }
    if let Some(alias) = (ops.dict_get_str)(kwargs, "validation_alias") {
        f.validation_alias = read_string(alias);
    }
    if let Some(alias) = (ops.dict_get_str)(kwargs, "serialization_alias") {
        f.serialization_alias = read_string(alias);
    }
    if let Some(ml) = (ops.dict_get_str)(kwargs, "min_length") {
        f.min_length = ml.as_int().map(|i| i as usize);
    }
    if let Some(ml) = (ops.dict_get_str)(kwargs, "max_length") {
        f.max_length = ml.as_int().map(|i| i as usize);
    }
    if let Some(pattern) = (ops.dict_get_str)(kwargs, "pattern") {
        f.pattern = read_string(pattern);
    }
    if f.pattern.is_none() {
        if let Some(pattern) = (ops.dict_get_str)(kwargs, "regex") {
            f.pattern = read_string(pattern);
        }
    }
    if let Some(format) = (ops.dict_get_str)(kwargs, "format") {
        f.format = read_string(format).and_then(|value| string_format_from_name(&value));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "min_value") {
        f.min_value = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "minimum") {
        f.min_value = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "ge") {
        f.min_value = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "max_value") {
        f.max_value = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "maximum") {
        f.max_value = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "le") {
        f.max_value = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "gt") {
        f.exclusive_min_value = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "exclusive_minimum") {
        f.exclusive_min_value = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "exclusiveMinimum") {
        f.exclusive_min_value = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "lt") {
        f.exclusive_max_value = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "exclusive_maximum") {
        f.exclusive_max_value = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "exclusiveMaximum") {
        f.exclusive_max_value = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mv) = (ops.dict_get_str)(kwargs, "multiple_of")
        .or_else(|| (ops.dict_get_str)(kwargs, "multipleOf"))
    {
        f.multiple_of = mv.as_float().or_else(|| mv.as_int().map(|i| i as f64));
    }
    if let Some(mi) = (ops.dict_get_str)(kwargs, "min_items") {
        f.min_items = mi.as_int().map(|i| i as usize);
    }
    if let Some(mi) = (ops.dict_get_str)(kwargs, "minItems") {
        f.min_items = mi.as_int().map(|i| i as usize);
    }
    if let Some(mi) = (ops.dict_get_str)(kwargs, "max_items") {
        f.max_items = mi.as_int().map(|i| i as usize);
    }
    if let Some(mi) = (ops.dict_get_str)(kwargs, "maxItems") {
        f.max_items = mi.as_int().map(|i| i as usize);
    }
    if let Some(unique) = (ops.dict_get_str)(kwargs, "unique_items") {
        if let Some(b) = unique.as_bool() {
            f.unique_items = b;
        }
    }
    if let Some(unique) = (ops.dict_get_str)(kwargs, "uniqueItems") {
        if let Some(b) = unique.as_bool() {
            f.unique_items = b;
        }
    }
    if f.min_items.is_none() {
        f.min_items = f.min_length;
    }
    if f.max_items.is_none() {
        f.max_items = f.max_length;
    }

    f
}

/// Convert a create_model/add_fields field-map value into a descriptor.
///
/// This keeps the original `Field(name, kwargs)` path intact while adding a
/// `mambalibs.dataclasses` extension for Pydantic-style model builders.
pub fn field_descriptor_from_spec(name: &str, spec: MbValue) -> Option<FieldDescriptor> {
    match native_type_name(spec) {
        Some("Field") => {
            let mut field = unsafe { mb_unwrap_native_ref::<FieldDescriptor>(spec) }?.clone();
            field.name = name.to_string();
            return Some(field);
        }
        Some("BaseModel") => {
            let model = unsafe { mb_unwrap_native_ref::<MbBaseModel>(spec) }?;
            return Some(FieldDescriptor::new(name, model.type_descriptor()));
        }
        _ => {}
    }

    if (mamba_registry::ops().dict_iter_str_items)(spec).is_some() {
        return Some(field_from_kwargs(name, spec).into_field_descriptor());
    }

    read_string(spec).map(|type_name| {
        MbSchemaField {
            name: name.to_string(),
            type_name: Some(type_name),
            required: true,
            ..Default::default()
        }
        .into_field_descriptor()
    })
}

// ── MbValue → SchemaValue conversion ─────────────────────────────────────────

/// Convert a raw [`MbValue`] to a [`SchemaValue`] for schema validation.
///
/// Strings are read from opaque runtime string objects through `ObjectOps`.
pub fn mb_to_schema_value(v: MbValue) -> Option<SchemaValue> {
    if v.is_none() {
        Some(SchemaValue::Null)
    } else if let Some(i) = v.as_int() {
        Some(SchemaValue::Int(i))
    } else if let Some(f) = v.as_float() {
        Some(SchemaValue::Float(f))
    } else if let Some(b) = v.as_bool() {
        Some(SchemaValue::Bool(b))
    } else {
        let ops = mamba_registry::ops();
        if let Some(len) = (ops.list_len)(v) {
            let mut items = Vec::with_capacity(len);
            for idx in 0..len {
                let item = (ops.list_get)(v, idx)?;
                items.push(mb_to_schema_value(item)?);
            }
            Some(SchemaValue::List(items))
        } else if let Some(entries) = (ops.dict_iter_str_items)(v) {
            let mut out = Vec::with_capacity(entries.len());
            for (key, value) in entries {
                out.push((key, mb_to_schema_value(value)?));
            }
            Some(SchemaValue::Object(out))
        } else {
            let s = (ops.str_read)(v)?;
            Some(SchemaValue::String(s))
        }
    }
}

fn examples_from_mb(value: MbValue) -> Vec<SchemaValue> {
    match mb_to_schema_value(value) {
        Some(SchemaValue::List(items)) => items,
        Some(value) => vec![value],
        None => Vec::new(),
    }
}

fn type_descriptor_from_name(
    type_name: Option<&str>,
    item_type_name: Option<&str>,
    field: &MbSchemaField,
) -> TypeDescriptor {
    let normalized_raw = type_name
        .map(normalize_type_name)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "str".to_string());
    let (normalized, nullable_from_type) =
        nullable_inner_type(&normalized_raw).map_or((normalized_raw, false), |inner| (inner, true));

    let descriptor = if let Some(item) = list_item_type_from_name(&normalized) {
        let item_descriptor = field.items_model_type.clone().unwrap_or_else(|| {
            let item_field = MbSchemaField::default();
            type_descriptor_from_name(Some(&item), None, &item_field)
        });
        list_type_descriptor(item_descriptor, field)
    } else if matches!(normalized.as_str(), "list" | "array") {
        let item_descriptor = field.items_model_type.clone().unwrap_or_else(|| {
            item_type_name
                .map(normalize_type_name)
                .map(|item| {
                    let item_field = MbSchemaField::default();
                    type_descriptor_from_name(Some(&item), None, &item_field)
                })
                .unwrap_or(TypeDescriptor::Any)
        });
        list_type_descriptor(item_descriptor, field)
    } else if let Some(model_type) = &field.nested_model_type {
        model_type.clone()
    } else if let Some((variants, union_nullable)) = union_type_parts(&normalized) {
        let item_field = MbSchemaField::default();
        let variants = variants
            .into_iter()
            .map(|variant| type_descriptor_from_name(Some(&variant), None, &item_field))
            .collect();
        TypeDescriptor::Union {
            variants,
            nullable: union_nullable,
        }
    } else {
        match normalized.as_str() {
            "str" | "string" => TypeDescriptor::String(StringConstraints {
                min_length: field.min_length,
                max_length: field.max_length,
                pattern: field.pattern.clone(),
                format: field.format,
            }),
            "int" | "integer" | "int64" => TypeDescriptor::Int64(NumericConstraints {
                minimum: field.min_value.and_then(float_to_i64),
                maximum: field.max_value.and_then(float_to_i64),
                exclusive_minimum: field.exclusive_min_value.and_then(float_to_i64),
                exclusive_maximum: field.exclusive_max_value.and_then(float_to_i64),
                multiple_of: field.multiple_of.and_then(float_to_i64),
            }),
            "float" | "number" | "double" => TypeDescriptor::Float64(NumericConstraints {
                minimum: field.min_value,
                maximum: field.max_value,
                exclusive_minimum: field.exclusive_min_value,
                exclusive_maximum: field.exclusive_max_value,
                multiple_of: field.multiple_of,
            }),
            "bool" | "boolean" => TypeDescriptor::Bool,
            "none" | "null" | "nonetype" => TypeDescriptor::Null,
            "email" => TypeDescriptor::Email,
            "url" | "uri" => TypeDescriptor::Url,
            "uuid" => TypeDescriptor::Uuid,
            "datetime" | "date-time" => TypeDescriptor::DateTime,
            "date" => TypeDescriptor::Date,
            "time" => TypeDescriptor::Time,
            _ => TypeDescriptor::String(StringConstraints {
                min_length: field.min_length,
                max_length: field.max_length,
                pattern: field.pattern.clone(),
                format: field.format,
            }),
        }
    };

    if field.nullable || nullable_from_type {
        TypeDescriptor::Optional(Box::new(descriptor))
    } else {
        descriptor
    }
}

fn list_type_descriptor(item_descriptor: TypeDescriptor, field: &MbSchemaField) -> TypeDescriptor {
    TypeDescriptor::List {
        items: Box::new(item_descriptor),
        constraints: ListConstraints {
            min_items: field.min_items,
            max_items: field.max_items,
            unique_items: field.unique_items,
        },
    }
}

fn list_item_type_from_name(type_name: &str) -> Option<String> {
    type_name
        .strip_prefix("list[")
        .or_else(|| type_name.strip_prefix("array["))
        .and_then(|inner| inner.strip_suffix(']'))
        .map(normalize_type_name)
}

fn nullable_inner_type(type_name: &str) -> Option<String> {
    type_name
        .strip_prefix("optional[")
        .or_else(|| type_name.strip_prefix("typing.optional["))
        .and_then(|inner| inner.strip_suffix(']'))
        .or_else(|| type_name.strip_suffix('?'))
        .map(normalize_type_name)
        .filter(|value| !value.is_empty())
}

fn union_type_parts(type_name: &str) -> Option<(Vec<String>, bool)> {
    let parts: Vec<String> = if let Some(inner) = type_name
        .strip_prefix("union[")
        .or_else(|| type_name.strip_prefix("typing.union["))
        .and_then(|inner| inner.strip_suffix(']'))
    {
        inner.split(',').map(normalize_type_name).collect()
    } else if type_name.contains('|') {
        type_name.split('|').map(normalize_type_name).collect()
    } else {
        return None;
    };

    let mut nullable = false;
    let mut variants = Vec::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if is_null_type_name(&part) {
            nullable = true;
        } else {
            variants.push(part);
        }
    }

    if variants.is_empty() {
        variants.push("null".to_string());
    }
    Some((variants, nullable))
}

fn is_null_type_name(value: &str) -> bool {
    matches!(value, "none" | "null" | "nonetype")
}

fn model_type_descriptor(value: MbValue) -> Option<TypeDescriptor> {
    unsafe { mb_unwrap_native_ref::<MbBaseModel>(value) }.map(MbBaseModel::type_descriptor)
}

fn normalize_type_name(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "")
}

fn read_string(value: MbValue) -> Option<String> {
    (mamba_registry::ops().str_read)(value)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn string_format_from_name(value: &str) -> Option<StringFormat> {
    match normalize_type_name(value).as_str() {
        "email" => Some(StringFormat::Email),
        "url" | "uri" => Some(StringFormat::Url),
        "uuid" => Some(StringFormat::Uuid),
        "datetime" | "date-time" => Some(StringFormat::DateTime),
        "date" => Some(StringFormat::Date),
        "time" => Some(StringFormat::Time),
        "ipv4" => Some(StringFormat::Ipv4),
        "ipv6" => Some(StringFormat::Ipv6),
        "hostname" => Some(StringFormat::Hostname),
        "fqdn" => Some(StringFormat::Fqdn),
        "phone" => Some(StringFormat::Phone),
        "base64" => Some(StringFormat::Base64),
        "slug" => Some(StringFormat::Slug),
        "json" => Some(StringFormat::Json),
        _ => None,
    }
}

fn float_to_i64(value: f64) -> Option<i64> {
    if value.is_finite() && value.fract() == 0.0 {
        Some(value as i64)
    } else {
        None
    }
}

fn compact_json(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .and_then(|value| serde_json::to_string(&value))
        .unwrap_or_else(|_| raw.to_string())
}
