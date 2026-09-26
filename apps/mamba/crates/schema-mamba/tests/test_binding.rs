//! Unit integration tests for `schema-mamba`.
//!
//! These tests exercise the trait implementations (`MambaModule`, `MbBaseModel`,
//! `field_from_kwargs`) and the FFI binding functions without bringing up the
//! full Mamba compiler stack.

use mamba_registry::convert::native_type_name;
use mamba_registry::{find_module, ops, MambaModule, MbValue, ModuleRegistrar};
use schema::types::Value as SchemaValue;
use schema_mamba::types::{field_from_kwargs, mb_to_schema_value, MbBaseModel};
use schema_mamba::{CclabSchemaMambaCompatModule, SchemaMambaModule};

fn wrap_str(s: &str) -> MbValue {
    mamba_registry::test_ops::init();
    (ops().str_new)(s)
}

fn dict(items: &[(&str, MbValue)]) -> MbValue {
    mamba_registry::test_ops::init();
    let ops = ops();
    let dict = (ops.dict_new)();
    for (key, value) in items {
        (ops.dict_insert_str)(dict, key, *value);
    }
    dict
}

fn item_model_with_defaults() -> MbValue {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
    };
    mamba_registry::test_ops::init();

    let model = unsafe { mb_schema_base_model_new([wrap_str("Item")].as_ptr(), 1) };
    let name = unsafe {
        mb_schema_field(
            [
                wrap_str("name"),
                dict(&[
                    ("type", wrap_str("str")),
                    ("min_length", MbValue::from_int(3)),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    let age = unsafe {
        mb_schema_field(
            [
                wrap_str("age"),
                dict(&[("type", wrap_str("int")), ("minimum", MbValue::from_int(1))]),
            ]
            .as_ptr(),
            2,
        )
    };
    let active = unsafe {
        mb_schema_field(
            [
                wrap_str("active"),
                dict(&[
                    ("type", wrap_str("bool")),
                    ("default", MbValue::from_bool(true)),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    let tags = unsafe {
        mb_schema_field(
            [
                wrap_str("tags"),
                dict(&[
                    ("type", wrap_str("list[str]")),
                    ("default", (ops().list_new)(vec![])),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };

    for field in [name, age, active, tags] {
        let _ = unsafe { mb_schema_model_add_field([model, field].as_ptr(), 2) };
    }

    model
}

fn user_model_with_nested_defaults() -> MbValue {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
    };
    mamba_registry::test_ops::init();

    let model = unsafe { mb_schema_base_model_new([wrap_str("User")].as_ptr(), 1) };
    let name = unsafe {
        mb_schema_field(
            [
                wrap_str("name"),
                dict(&[
                    ("type", wrap_str("str")),
                    ("min_length", MbValue::from_int(3)),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    let age = unsafe {
        mb_schema_field(
            [
                wrap_str("age"),
                dict(&[("type", wrap_str("int")), ("default", MbValue::from_int(1))]),
            ]
            .as_ptr(),
            2,
        )
    };
    let active = unsafe {
        mb_schema_field(
            [
                wrap_str("active"),
                dict(&[
                    ("type", wrap_str("bool")),
                    ("default", MbValue::from_bool(true)),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };

    for field in [name, age, active] {
        let _ = unsafe { mb_schema_model_add_field([model, field].as_ptr(), 2) };
    }

    model
}

// ── MambaModule trait ─────────────────────────────────────────────────────────

#[test]
fn module_name_is_correct() {
    assert_eq!(SchemaMambaModule.name(), "mambalibs.dataclasses");
}

#[test]
fn compat_module_name_is_preserved() {
    assert_eq!(CclabSchemaMambaCompatModule.name(), "cclab_schema_mamba");
}

#[test]
fn module_registers_schema_symbols() {
    let mut r = ModuleRegistrar::new();
    SchemaMambaModule.register(&mut r);
    assert_eq!(
        r.symbols().len(),
        27,
        "expected public schema symbols plus bound method dispatchers"
    );
}

#[test]
fn module_symbol_names_match_spec() {
    let mut r = ModuleRegistrar::new();
    SchemaMambaModule.register(&mut r);
    let names: Vec<&str> = r.symbols().iter().map(|s| s.name).collect();
    assert!(names.contains(&"BaseModel"));
    assert!(names.contains(&"DataClass"));
    assert!(names.contains(&"Field"));
    assert!(names.contains(&"create_model"));
    assert!(names.contains(&"add_field"));
    assert!(names.contains(&"add_fields"));
    assert!(names.contains(&"validate"));
    assert!(names.contains(&"model_validate"));
    assert!(names.contains(&"parse_obj"));
    assert!(names.contains(&"model_validate_json"));
    assert!(names.contains(&"parse_raw"));
    assert!(names.contains(&"model_dump"));
    assert!(names.contains(&"model_dump_json"));
    assert!(names.contains(&"field_validator"));
    assert!(names.contains(&"to_json_schema"));
    assert!(names.contains(&"model_json_schema"));
    assert!(names.contains(&"_schema_model_add_field_bound"));
    assert!(names.contains(&"_schema_model_add_fields_bound"));
    assert!(names.contains(&"_schema_model_validate_bound"));
    assert!(names.contains(&"_schema_model_model_validate_bound"));
    assert!(names.contains(&"_schema_model_parse_obj_bound"));
    assert!(names.contains(&"_schema_model_model_validate_json_bound"));
    assert!(names.contains(&"_schema_model_parse_raw_bound"));
    assert!(names.contains(&"_schema_model_model_dump_bound"));
    assert!(names.contains(&"_schema_model_model_dump_json_bound"));
    assert!(names.contains(&"_schema_model_to_json_schema_bound"));
    assert!(names.contains(&"_schema_model_model_json_schema_bound"));
}

#[test]
fn module_ffi_names_still_point_to_schema_functions() {
    let mut r = ModuleRegistrar::new();
    SchemaMambaModule.register(&mut r);
    let ffi_names: Vec<&str> = r.symbols().iter().map(|s| s.ffi_name).collect();
    assert!(ffi_names.contains(&"mb_schema_base_model_new"));
    assert!(ffi_names.contains(&"mb_schema_field"));
    assert!(ffi_names.contains(&"mb_schema_create_model"));
    assert!(ffi_names.contains(&"mb_schema_model_add_field"));
    assert!(ffi_names.contains(&"mb_schema_model_add_fields"));
    assert!(ffi_names.contains(&"mb_schema_validate"));
    assert!(ffi_names.contains(&"mb_schema_model_validate"));
    assert!(ffi_names.contains(&"mb_schema_parse_obj"));
    assert!(ffi_names.contains(&"mb_schema_model_validate_json"));
    assert!(ffi_names.contains(&"mb_schema_parse_raw"));
    assert!(ffi_names.contains(&"mb_schema_model_dump"));
    assert!(ffi_names.contains(&"mb_schema_model_dump_json"));
    assert!(ffi_names.contains(&"mb_schema_field_validator"));
    assert!(ffi_names.contains(&"mb_schema_to_json_schema"));
    assert!(ffi_names.contains(&"mb_schema_model_json_schema"));
    assert!(ffi_names.contains(&"mb_schema_model_add_field_bound"));
    assert!(ffi_names.contains(&"mb_schema_model_add_fields_bound"));
    assert!(ffi_names.contains(&"mb_schema_validate_bound"));
    assert!(ffi_names.contains(&"mb_schema_model_validate_bound"));
    assert!(ffi_names.contains(&"mb_schema_parse_obj_bound"));
    assert!(ffi_names.contains(&"mb_schema_model_validate_json_bound"));
    assert!(ffi_names.contains(&"mb_schema_parse_raw_bound"));
    assert!(ffi_names.contains(&"mb_schema_model_dump_bound"));
    assert!(ffi_names.contains(&"mb_schema_model_dump_json_bound"));
    assert!(ffi_names.contains(&"mb_schema_to_json_schema_bound"));
    assert!(ffi_names.contains(&"mb_schema_model_json_schema_bound"));
}

#[test]
fn linkme_registers_primary_and_compat_modules() {
    mamba_registry::test_ops::init();
    assert!(
        find_module("mambalibs.dataclasses").is_some(),
        "primary dataclasses namespace must register"
    );
    assert!(
        find_module("cclab_schema_mamba").is_some(),
        "legacy cclab_schema_mamba namespace must remain registered"
    );
}

// ── mb_to_schema_value ────────────────────────────────────────────────────────

#[test]
fn convert_none_to_null() {
    let sv = mb_to_schema_value(MbValue::none());
    assert_eq!(sv, Some(SchemaValue::Null));
}

#[test]
fn convert_int_to_schema_int() {
    let sv = mb_to_schema_value(MbValue::from_int(42));
    assert_eq!(sv, Some(SchemaValue::Int(42)));
}

#[test]
fn convert_float_to_schema_float() {
    let sv = mb_to_schema_value(MbValue::from_float(3.14));
    assert!(matches!(sv, Some(SchemaValue::Float(f)) if (f - 3.14).abs() < 1e-9));
}

#[test]
fn convert_bool_to_schema_bool() {
    assert_eq!(
        mb_to_schema_value(MbValue::from_bool(true)),
        Some(SchemaValue::Bool(true))
    );
    assert_eq!(
        mb_to_schema_value(MbValue::from_bool(false)),
        Some(SchemaValue::Bool(false))
    );
}

#[test]
fn convert_string_ptr_to_schema_string() {
    // Simulate Mamba's string allocation (MbObject → PTR).
    let v = wrap_str("hello");
    let sv = mb_to_schema_value(v);
    assert_eq!(sv, Some(SchemaValue::String("hello".to_string())));
}

// ── field_from_kwargs ─────────────────────────────────────────────────────────

#[test]
fn field_from_empty_kwargs() {
    let f = field_from_kwargs("username", MbValue::none());
    assert_eq!(f.name, "username");
    assert!(f.required);
    assert!(f.default.is_none());
    assert!(f.min_length.is_none());
}

#[test]
fn field_from_kwargs_with_min_length() {
    // Build a kwargs dict: {"min_length": 3}
    let kwargs = dict(&[("min_length", MbValue::from_int(3))]);

    let f = field_from_kwargs("username", kwargs);
    assert_eq!(f.min_length, Some(3));
    assert!(f.required, "no default → should remain required");
}

#[test]
fn field_from_kwargs_with_default_makes_optional() {
    // Simulate a string default value as a heap MbObject.
    let default_ptr = wrap_str("test");
    let kwargs = dict(&[("default", default_ptr)]);

    let f = field_from_kwargs("name", kwargs);
    // Having a default should flip `required` to false.
    assert!(!f.required, "field with a default should not be required");
    assert!(f.default.is_some());
}

#[test]
fn field_from_kwargs_with_typed_numeric_constraints() {
    use schema::types::TypeDescriptor;

    let kwargs = dict(&[
        ("type", wrap_str("int")),
        ("minimum", MbValue::from_int(1)),
        ("maximum", MbValue::from_int(10)),
    ]);

    let fd = field_from_kwargs("age", kwargs).into_field_descriptor();
    match fd.type_desc {
        TypeDescriptor::Int64(constraints) => {
            assert_eq!(constraints.minimum, Some(1));
            assert_eq!(constraints.maximum, Some(10));
        }
        other => panic!("expected int field descriptor, got {other:?}"),
    }
    assert!(fd.required);
}

#[test]
fn field_from_kwargs_with_aliases_regex_and_exclusive_constraints() {
    use schema::types::TypeDescriptor;

    let kwargs = dict(&[
        ("type", wrap_str("int")),
        ("alias", wrap_str("userId")),
        ("validation_alias", wrap_str("inputId")),
        ("serialization_alias", wrap_str("user_id")),
        ("gt", MbValue::from_int(0)),
        ("lt", MbValue::from_int(10)),
    ]);

    let fd = field_from_kwargs("id", kwargs).into_field_descriptor();
    assert_eq!(fd.alias.as_deref(), Some("userId"));
    assert_eq!(fd.validation_alias.as_deref(), Some("inputId"));
    assert_eq!(fd.serialization_alias.as_deref(), Some("user_id"));
    match fd.type_desc {
        TypeDescriptor::Int64(constraints) => {
            assert_eq!(constraints.exclusive_minimum, Some(0));
            assert_eq!(constraints.exclusive_maximum, Some(10));
        }
        other => panic!("expected int field descriptor, got {other:?}"),
    }

    let regex = dict(&[("type", wrap_str("str")), ("regex", wrap_str("^[A-Z]+$"))]);
    let fd = field_from_kwargs("code", regex).into_field_descriptor();
    match fd.type_desc {
        TypeDescriptor::String(constraints) => {
            assert_eq!(constraints.pattern.as_deref(), Some("^[A-Z]+$"));
        }
        other => panic!("expected string field descriptor, got {other:?}"),
    }
}

#[test]
fn field_from_kwargs_with_optional_nullable_and_default_none() {
    use schema::types::TypeDescriptor;

    let optional = field_from_kwargs("nickname", dict(&[("type", wrap_str("Optional[str]"))]))
        .into_field_descriptor();
    assert!(
        optional.required,
        "Optional without a default is still required"
    );
    match optional.type_desc {
        TypeDescriptor::Optional(inner) => {
            assert!(matches!(*inner, TypeDescriptor::String(_)));
        }
        other => panic!("expected Optional[str], got {other:?}"),
    }

    let nullable = field_from_kwargs(
        "score",
        dict(&[
            ("type", wrap_str("int")),
            ("nullable", MbValue::from_bool(true)),
        ]),
    )
    .into_field_descriptor();
    match nullable.type_desc {
        TypeDescriptor::Optional(inner) => {
            assert!(matches!(*inner, TypeDescriptor::Int64(_)));
        }
        other => panic!("expected nullable int, got {other:?}"),
    }

    let default_none = field_from_kwargs(
        "middle_name",
        dict(&[
            ("type", wrap_str("Optional[str]")),
            ("default", MbValue::none()),
        ]),
    )
    .into_field_descriptor();
    assert!(
        !default_none.required,
        "default=None should make the field non-required"
    );
    assert_eq!(default_none.default, Some(SchemaValue::Null));

    let list_optional =
        field_from_kwargs("scores", dict(&[("type", wrap_str("list[int | None]"))]))
            .into_field_descriptor();
    match list_optional.type_desc {
        TypeDescriptor::List { items, .. } => match *items {
            TypeDescriptor::Union { nullable, .. } => assert!(nullable),
            TypeDescriptor::Optional(_) => {}
            other => panic!("expected nullable list item type, got {other:?}"),
        },
        other => panic!("expected list type, got {other:?}"),
    }
}

#[test]
fn field_from_kwargs_with_nested_model_handles() {
    use schema::types::TypeDescriptor;
    mamba_registry::test_ops::init();

    let user = user_model_with_nested_defaults();
    let owner = field_from_kwargs("owner", dict(&[("model", user)])).into_field_descriptor();
    match owner.type_desc {
        TypeDescriptor::Object { fields, .. } => {
            assert!(fields.iter().any(|field| field.name == "name"));
            assert!(fields.iter().any(|field| field.name == "active"));
        }
        other => panic!("expected nested object model type, got {other:?}"),
    }

    let members = field_from_kwargs(
        "members",
        dict(&[("type", wrap_str("list")), ("items_model", user)]),
    )
    .into_field_descriptor();
    match members.type_desc {
        TypeDescriptor::List { items, .. } => match *items {
            TypeDescriptor::Object { fields, .. } => {
                assert!(fields.iter().any(|field| field.name == "name"));
                assert!(fields.iter().any(|field| field.name == "age"));
            }
            other => panic!("expected nested object item type, got {other:?}"),
        },
        other => panic!("expected list of nested model, got {other:?}"),
    }
}

#[test]
fn field_from_kwargs_with_list_default_and_item_type() {
    use schema::types::TypeDescriptor;
    mamba_registry::test_ops::init();

    let default = (ops().list_new)(vec![wrap_str("core")]);
    let kwargs = dict(&[
        ("type", wrap_str("list[str]")),
        ("default", default),
        ("min_items", MbValue::from_int(1)),
    ]);

    let fd = field_from_kwargs("tags", kwargs).into_field_descriptor();
    assert!(!fd.required);
    assert!(matches!(fd.default, Some(SchemaValue::List(ref items)) if items.len() == 1));
    match fd.type_desc {
        TypeDescriptor::List { items, constraints } => {
            assert_eq!(constraints.min_items, Some(1));
            assert!(matches!(*items, TypeDescriptor::String(_)));
        }
        other => panic!("expected list field descriptor, got {other:?}"),
    }
}

// ── MbBaseModel ───────────────────────────────────────────────────────────────

#[test]
fn base_model_new_and_add_field() {
    use schema::constraints::FieldDescriptor;
    use schema::constraints::StringConstraints;
    use schema::types::TypeDescriptor;

    let mut model = MbBaseModel::new("UserCreate");
    assert_eq!(model.name, "UserCreate");
    assert!(model.fields.is_empty());

    let fd = FieldDescriptor::new(
        "email",
        TypeDescriptor::String(StringConstraints::default()),
    );
    model.add_field(fd);
    assert_eq!(model.fields.len(), 1);
    assert!(model.fields.contains_key("email"));
}

// ── mb_schema_* FFI functions ─────────────────────────────────────────────────

#[test]
fn mb_schema_base_model_new_returns_ptr() {
    use schema_mamba::methods::mb_schema_base_model_new;

    // Simulate a Mamba string: MbObject string.
    let name_ptr = wrap_str("TestModel");

    let args = [name_ptr];
    let result = unsafe { mb_schema_base_model_new(args.as_ptr(), args.len()) };
    assert!(
        result.is_ptr(),
        "mb_schema_base_model_new must return an opaque PTR"
    );
    assert_eq!(native_type_name(result), Some("BaseModel"));
}

#[test]
fn mb_schema_field_returns_ptr() {
    use schema_mamba::methods::mb_schema_field;

    let name_ptr = wrap_str("username");
    let kwargs = MbValue::none();
    let args = [name_ptr, kwargs];
    let result = unsafe { mb_schema_field(args.as_ptr(), args.len()) };
    assert!(result.is_ptr(), "mb_schema_field must return an opaque PTR");
    assert_eq!(native_type_name(result), Some("Field"));
}

#[test]
fn mb_schema_model_add_field_registers_field() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
        mb_schema_to_json_schema,
    };
    mamba_registry::test_ops::init();

    let model = unsafe { mb_schema_base_model_new([wrap_str("User")].as_ptr(), 1) };
    let kwargs = dict(&[("min_length", MbValue::from_int(3))]);
    let field_args = [wrap_str("name"), kwargs];
    let field = unsafe { mb_schema_field(field_args.as_ptr(), field_args.len()) };

    let add_args = [model, field];
    let result = unsafe { mb_schema_model_add_field(add_args.as_ptr(), add_args.len()) };
    assert_eq!(result, model, "add_field should return the model handle");

    let schema = unsafe { mb_schema_to_json_schema([model].as_ptr(), 1) };
    let json = (ops().str_read)(schema).unwrap();
    assert!(
        json.contains("\"name\""),
        "registered field must appear in schema: {json}"
    );
    assert!(
        json.contains("\"minLength\":3"),
        "field kwargs must affect schema: {json}"
    );
}

#[test]
fn mb_schema_create_model_builds_from_flexible_field_map() {
    use schema_mamba::methods::{
        mb_schema_create_model, mb_schema_field, mb_schema_model_dump_json,
        mb_schema_model_json_schema,
    };
    mamba_registry::test_ops::init();

    let age_field = unsafe {
        mb_schema_field(
            [dict(&[
                ("type", wrap_str("int")),
                ("minimum", MbValue::from_int(1)),
            ])]
            .as_ptr(),
            1,
        )
    };
    let owner_model = user_model_with_nested_defaults();
    let fields = dict(&[
        (
            "name",
            dict(&[
                ("type", wrap_str("str")),
                ("min_length", MbValue::from_int(3)),
            ]),
        ),
        ("age", age_field),
        ("active", wrap_str("bool")),
        ("owner", owner_model),
    ]);
    let model = unsafe { mb_schema_create_model([wrap_str("ItemCreate"), fields].as_ptr(), 2) };
    assert_eq!(native_type_name(model), Some("BaseModel"));

    let owner = dict(&[("name", wrap_str("alice"))]);
    let data = dict(&[
        ("name", wrap_str("widget")),
        ("age", wrap_str("2")),
        ("active", MbValue::from_bool(true)),
        ("owner", owner),
    ]);
    let json = unsafe { mb_schema_model_dump_json([model, data].as_ptr(), 2) };
    let doc: serde_json::Value = serde_json::from_str(&(ops().str_read)(json).expect("dump json"))
        .expect("dump should parse");
    assert_eq!(doc["name"].as_str(), Some("widget"));
    assert_eq!(doc["age"].as_i64(), Some(2));
    assert_eq!(doc["active"].as_bool(), Some(true));
    assert_eq!(doc["owner"]["name"].as_str(), Some("alice"));
    assert_eq!(doc["owner"]["age"].as_i64(), Some(1));

    let schema = unsafe { mb_schema_model_json_schema([model].as_ptr(), 1) };
    let schema_doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(schema).expect("schema json"))
            .expect("schema should parse");
    assert_eq!(schema_doc["title"].as_str(), Some("ItemCreate"));
    assert_eq!(
        schema_doc["properties"]["name"]["minLength"].as_i64(),
        Some(3)
    );
    assert_eq!(schema_doc["properties"]["age"]["minimum"].as_i64(), Some(1));
    assert_eq!(
        schema_doc["properties"]["active"]["type"].as_str(),
        Some("boolean")
    );
    assert_eq!(
        schema_doc["properties"]["owner"]["properties"]["age"]["default"].as_i64(),
        Some(1)
    );
}

#[test]
fn mb_schema_model_add_fields_registers_batch_specs() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_fields,
        mb_schema_model_dump_json, mb_schema_model_json_schema,
    };
    mamba_registry::test_ops::init();

    let model = unsafe { mb_schema_base_model_new([wrap_str("BatchItem")].as_ptr(), 1) };
    let renamed = unsafe {
        mb_schema_field(
            [
                wrap_str("legacy_age"),
                dict(&[("type", wrap_str("int")), ("minimum", MbValue::from_int(1))]),
            ]
            .as_ptr(),
            2,
        )
    };
    let fields = dict(&[
        (
            "name",
            dict(&[
                ("type", wrap_str("str")),
                ("min_length", MbValue::from_int(3)),
            ]),
        ),
        ("age", renamed),
        (
            "tags",
            dict(&[
                ("type", wrap_str("list[str]")),
                ("default", (ops().list_new)(vec![])),
            ]),
        ),
    ]);
    let result = unsafe { mb_schema_model_add_fields([model, fields].as_ptr(), 2) };
    assert_eq!(result, model, "add_fields should return the model handle");

    let data = dict(&[("name", wrap_str("widget")), ("age", wrap_str("2"))]);
    let json = unsafe { mb_schema_model_dump_json([model, data].as_ptr(), 2) };
    let doc: serde_json::Value = serde_json::from_str(&(ops().str_read)(json).expect("dump json"))
        .expect("dump should parse");
    assert_eq!(doc["age"].as_i64(), Some(2));
    assert!(doc["tags"].as_array().is_some_and(|items| items.is_empty()));

    let schema = unsafe { mb_schema_model_json_schema([model].as_ptr(), 1) };
    let schema_doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(schema).expect("schema json"))
            .expect("schema should parse");
    assert!(schema_doc["properties"]["age"].is_object());
    assert!(
        schema_doc["properties"]["legacy_age"].is_null(),
        "field map keys should assign names for Field handles"
    );
}

#[test]
fn mb_schema_validate_reads_objectops_dict() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field, mb_schema_validate,
    };
    mamba_registry::test_ops::init();

    let model = unsafe { mb_schema_base_model_new([wrap_str("User")].as_ptr(), 1) };
    let field = unsafe { mb_schema_field([wrap_str("name"), MbValue::none()].as_ptr(), 2) };
    let _ = unsafe { mb_schema_model_add_field([model, field].as_ptr(), 2) };

    let data = dict(&[("name", wrap_str("alice"))]);
    let validate_args = [model, data];
    let result = unsafe { mb_schema_validate(validate_args.as_ptr(), validate_args.len()) };
    assert_eq!(result.as_bool(), Some(true));
}

#[test]
fn mb_schema_validate_rejects_typed_scalar_and_list_errors() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field, mb_schema_validate,
    };
    mamba_registry::test_ops::init();

    let model = unsafe { mb_schema_base_model_new([wrap_str("Item")].as_ptr(), 1) };
    let age = unsafe {
        mb_schema_field(
            [
                wrap_str("age"),
                dict(&[("type", wrap_str("int")), ("minimum", MbValue::from_int(1))]),
            ]
            .as_ptr(),
            2,
        )
    };
    let tags = unsafe {
        mb_schema_field(
            [wrap_str("tags"), dict(&[("type", wrap_str("list[str]"))])].as_ptr(),
            2,
        )
    };
    let _ = unsafe { mb_schema_model_add_field([model, age].as_ptr(), 2) };
    let _ = unsafe { mb_schema_model_add_field([model, tags].as_ptr(), 2) };

    let invalid = dict(&[
        ("age", wrap_str("two")),
        ("tags", (ops().list_new)(vec![MbValue::from_int(7)])),
    ]);
    let result = unsafe { mb_schema_validate([model, invalid].as_ptr(), 2) };
    let error = (ops().str_read)(result).expect("invalid data returns error string");
    assert!(error.contains("ValidationError"), "{error}");
}

#[test]
fn mb_schema_model_validate_returns_defaulted_coerced_dict() {
    use schema_mamba::methods::mb_schema_model_validate;
    mamba_registry::test_ops::init();

    let model = item_model_with_defaults();
    let data = dict(&[
        ("name", wrap_str("alice")),
        ("age", wrap_str("2")),
        ("tags", (ops().list_new)(vec![wrap_str("api")])),
        ("ignored", wrap_str("drop-me")),
    ]);

    let result = unsafe { mb_schema_model_validate([model, data].as_ptr(), 2) };
    assert_eq!(
        (ops().str_read)((ops().dict_get_str)(result, "name").unwrap()).as_deref(),
        Some("alice")
    );
    assert_eq!(
        (ops().dict_get_str)(result, "age").and_then(|v| v.as_int()),
        Some(2)
    );
    assert_eq!(
        (ops().dict_get_str)(result, "active").and_then(|v| v.as_bool()),
        Some(true)
    );
    let tags = (ops().dict_get_str)(result, "tags").expect("defaulted tags exist");
    assert_eq!((ops().list_len)(tags), Some(1));
    assert!(
        (ops().dict_get_str)(result, "ignored").is_none(),
        "model output should contain declared fields only"
    );
}

#[test]
fn mb_schema_model_validate_strict_rejects_lax_coercion() {
    use schema_mamba::methods::mb_schema_model_validate;
    mamba_registry::test_ops::init();

    let model = item_model_with_defaults();
    let data = dict(&[("name", wrap_str("alice")), ("age", wrap_str("2"))]);
    let options = dict(&[("strict", MbValue::from_bool(true))]);

    let result = unsafe { mb_schema_model_validate([model, data, options].as_ptr(), 3) };
    let error = (ops().str_read)(result).expect("strict mismatch returns error string");
    assert!(error.contains("ValidationError"), "{error}");
}

#[test]
fn mb_schema_parse_obj_and_model_dump_alias_model_validate() {
    use schema_mamba::methods::{mb_schema_model_dump, mb_schema_parse_obj};
    mamba_registry::test_ops::init();

    let model = item_model_with_defaults();
    let data = dict(&[("name", wrap_str("alice")), ("age", MbValue::from_int(2))]);
    let parsed = unsafe { mb_schema_parse_obj([model, data].as_ptr(), 2) };
    assert_eq!(
        (ops().dict_get_str)(parsed, "active").and_then(|v| v.as_bool()),
        Some(true)
    );

    let dump = unsafe { mb_schema_model_dump([model, data].as_ptr(), 2) };
    assert_eq!(
        (ops().dict_get_str)(dump, "age").and_then(|v| v.as_int()),
        Some(2)
    );
}

#[test]
fn mb_schema_model_validate_json_and_parse_raw_normalize_payloads() {
    use schema_mamba::methods::{mb_schema_model_validate_json, mb_schema_parse_raw};
    mamba_registry::test_ops::init();

    let model = item_model_with_defaults();
    let raw = wrap_str(r#"{"name":"alice","age":"2","tags":["api"],"ignored":"drop"}"#);
    let result = unsafe { mb_schema_model_validate_json([model, raw].as_ptr(), 2) };
    assert_eq!(
        (ops().str_read)((ops().dict_get_str)(result, "name").unwrap()).as_deref(),
        Some("alice")
    );
    assert_eq!(
        (ops().dict_get_str)(result, "age").and_then(|v| v.as_int()),
        Some(2)
    );
    assert_eq!(
        (ops().dict_get_str)(result, "active").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert!(
        (ops().dict_get_str)(result, "ignored").is_none(),
        "JSON validation should filter undeclared fields"
    );

    let raw = wrap_str(r#"{"name":"alice","age":"3"}"#);
    let parsed = unsafe { mb_schema_parse_raw([model, raw].as_ptr(), 2) };
    assert_eq!(
        (ops().dict_get_str)(parsed, "age").and_then(|v| v.as_int()),
        Some(3)
    );

    let invalid_json = wrap_str(r#"{"name":"alice","age":}"#);
    let error = unsafe { mb_schema_model_validate_json([model, invalid_json].as_ptr(), 2) };
    let message = (ops().str_read)(error).expect("invalid JSON returns error string");
    assert!(
        message.contains("ValidationError: invalid JSON"),
        "{message}"
    );
}

#[test]
fn mb_schema_model_dump_json_emits_normalized_payload() {
    use schema_mamba::methods::mb_schema_model_dump_json;
    mamba_registry::test_ops::init();

    let model = item_model_with_defaults();
    let data = dict(&[("name", wrap_str("alice")), ("age", wrap_str("2"))]);

    let result = unsafe { mb_schema_model_dump_json([model, data].as_ptr(), 2) };
    let json = (ops().str_read)(result).expect("model_dump_json returns string");
    let doc: serde_json::Value = serde_json::from_str(&json).expect("dump should be json");
    assert_eq!(doc["name"].as_str(), Some("alice"));
    assert_eq!(doc["age"].as_i64(), Some(2));
    assert_eq!(doc["active"].as_bool(), Some(true));
    assert!(doc["tags"].as_array().is_some_and(|items| items.is_empty()));
}

#[test]
fn mb_schema_model_validate_accepts_optional_nullable_defaults() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
        mb_schema_model_dump_json, mb_schema_model_json_schema, mb_schema_model_validate,
    };
    mamba_registry::test_ops::init();

    let model = unsafe { mb_schema_base_model_new([wrap_str("Profile")].as_ptr(), 1) };
    let nickname = unsafe {
        mb_schema_field(
            [
                wrap_str("nickname"),
                dict(&[("type", wrap_str("Optional[str]"))]),
            ]
            .as_ptr(),
            2,
        )
    };
    let score = unsafe {
        mb_schema_field(
            [
                wrap_str("score"),
                dict(&[
                    ("type", wrap_str("int")),
                    ("nullable", MbValue::from_bool(true)),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    let middle_name = unsafe {
        mb_schema_field(
            [
                wrap_str("middle_name"),
                dict(&[
                    ("type", wrap_str("Optional[str]")),
                    ("default", MbValue::none()),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    for field in [nickname, score, middle_name] {
        let _ = unsafe { mb_schema_model_add_field([model, field].as_ptr(), 2) };
    }

    let data = dict(&[("nickname", MbValue::none()), ("score", MbValue::none())]);
    let validated = unsafe { mb_schema_model_validate([model, data].as_ptr(), 2) };
    assert!(
        (ops().dict_get_str)(validated, "nickname").is_some_and(MbValue::is_none),
        "Optional[str] should preserve explicit null"
    );
    assert!(
        (ops().dict_get_str)(validated, "score").is_some_and(MbValue::is_none),
        "nullable int should preserve explicit null"
    );
    assert!(
        (ops().dict_get_str)(validated, "middle_name").is_some_and(MbValue::is_none),
        "default=None should materialize null"
    );

    let json = unsafe { mb_schema_model_dump_json([model, data].as_ptr(), 2) };
    let doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(json).unwrap()).expect("dump json");
    assert!(doc["nickname"].is_null());
    assert!(doc["score"].is_null());
    assert!(doc["middle_name"].is_null());

    let schema = unsafe { mb_schema_model_json_schema([model].as_ptr(), 1) };
    let schema_doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(schema).unwrap()).expect("schema json");
    assert_eq!(
        schema_doc["properties"]["nickname"]["type"],
        serde_json::json!(["string", "null"])
    );
    assert_eq!(
        schema_doc["properties"]["score"]["type"],
        serde_json::json!(["integer", "null"])
    );
    assert!(schema_doc["properties"]["middle_name"]["default"].is_null());
    let required = schema_doc["required"]
        .as_array()
        .expect("required array")
        .iter()
        .filter_map(|item| item.as_str())
        .collect::<Vec<_>>();
    assert_eq!(required, vec!["nickname", "score"]);
}

#[test]
fn mb_schema_model_validate_normalizes_nested_models() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
        mb_schema_model_dump_json, mb_schema_model_json_schema,
    };
    mamba_registry::test_ops::init();

    let user = user_model_with_nested_defaults();
    let team = unsafe { mb_schema_base_model_new([wrap_str("Team")].as_ptr(), 1) };
    let owner =
        unsafe { mb_schema_field([wrap_str("owner"), dict(&[("model", user)])].as_ptr(), 2) };
    let members = unsafe {
        mb_schema_field(
            [
                wrap_str("members"),
                dict(&[("type", wrap_str("list")), ("items_model", user)]),
            ]
            .as_ptr(),
            2,
        )
    };
    for field in [owner, members] {
        let _ = unsafe { mb_schema_model_add_field([team, field].as_ptr(), 2) };
    }

    let owner_payload = dict(&[("name", wrap_str("alice"))]);
    let member_payload = dict(&[("name", wrap_str("bob")), ("age", wrap_str("2"))]);
    let data = dict(&[
        ("owner", owner_payload),
        ("members", (ops().list_new)(vec![member_payload])),
    ]);
    let json = unsafe { mb_schema_model_dump_json([team, data].as_ptr(), 2) };
    let doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(json).unwrap()).expect("dump json");
    assert_eq!(doc["owner"]["name"].as_str(), Some("alice"));
    assert_eq!(doc["owner"]["age"].as_i64(), Some(1));
    assert_eq!(doc["owner"]["active"].as_bool(), Some(true));
    assert_eq!(doc["members"][0]["name"].as_str(), Some("bob"));
    assert_eq!(doc["members"][0]["age"].as_i64(), Some(2));
    assert_eq!(doc["members"][0]["active"].as_bool(), Some(true));

    let invalid = dict(&[
        ("owner", dict(&[])),
        (
            "members",
            (ops().list_new)(vec![dict(&[("name", wrap_str("bob"))])]),
        ),
    ]);
    let result = unsafe { mb_schema_model_dump_json([team, invalid].as_ptr(), 2) };
    let error = (ops().str_read)(result).expect("nested missing field returns error");
    assert!(error.contains("ValidationError"), "{error}");

    let schema = unsafe { mb_schema_model_json_schema([team].as_ptr(), 1) };
    let schema_doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(schema).unwrap()).expect("schema json");
    assert_eq!(
        schema_doc["properties"]["owner"]["properties"]["name"]["type"].as_str(),
        Some("string")
    );
    assert_eq!(
        schema_doc["properties"]["members"]["items"]["properties"]["age"]["type"].as_str(),
        Some("integer")
    );
    assert_eq!(
        schema_doc["properties"]["members"]["items"]["properties"]["active"]["default"].as_bool(),
        Some(true)
    );
}

#[test]
fn model_validation_detail_json_reports_fastapi_shape() {
    use schema_mamba::methods::model_validation_detail_json;
    mamba_registry::test_ops::init();

    let model = item_model_with_defaults();
    let data = dict(&[
        ("name", wrap_str("al")),
        ("age", wrap_str("two")),
        ("tags", (ops().list_new)(vec![MbValue::from_int(7)])),
    ]);

    let details = model_validation_detail_json(model, data, "body").expect("detail json");
    let doc: serde_json::Value = serde_json::from_str(&details).expect("detail should be json");
    let items = doc.as_array().expect("detail should be an array");
    assert!(
        items
            .iter()
            .any(|item| item["loc"] == serde_json::json!(["body", "age"])
                && item["type"].as_str() == Some("type_error")),
        "{doc}"
    );
    assert!(
        items
            .iter()
            .any(|item| item["loc"] == serde_json::json!(["body", "name"])
                && item["msg"]
                    .as_str()
                    .is_some_and(|msg| msg.contains("at least"))),
        "{doc}"
    );

    let valid = dict(&[("name", wrap_str("alice")), ("age", wrap_str("2"))]);
    let empty = model_validation_detail_json(model, valid, "body").expect("valid detail json");
    assert_eq!(empty, "[]");
}

#[test]
fn mb_schema_model_dump_supports_aliases_and_exclusive_constraints() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field, mb_schema_model_dump,
        mb_schema_model_dump_json, mb_schema_model_json_schema, mb_schema_model_validate,
    };
    mamba_registry::test_ops::init();

    let model = unsafe { mb_schema_base_model_new([wrap_str("Account")].as_ptr(), 1) };
    let id = unsafe {
        mb_schema_field(
            [
                wrap_str("id"),
                dict(&[
                    ("type", wrap_str("int")),
                    ("alias", wrap_str("userId")),
                    ("serialization_alias", wrap_str("user_id")),
                    ("gt", MbValue::from_int(0)),
                    ("lt", MbValue::from_int(10)),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    let code = unsafe {
        mb_schema_field(
            [
                wrap_str("code"),
                dict(&[
                    ("type", wrap_str("str")),
                    ("validation_alias", wrap_str("accountCode")),
                    ("regex", wrap_str("^[A-Z]{2}$")),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    for field in [id, code] {
        let _ = unsafe { mb_schema_model_add_field([model, field].as_ptr(), 2) };
    }

    let data = dict(&[("userId", wrap_str("2")), ("accountCode", wrap_str("AB"))]);
    let validated = unsafe { mb_schema_model_validate([model, data].as_ptr(), 2) };
    assert_eq!(
        (ops().dict_get_str)(validated, "id").and_then(|v| v.as_int()),
        Some(2)
    );
    assert!(
        (ops().dict_get_str)(validated, "user_id").is_none(),
        "model_validate should keep canonical names"
    );

    let by_alias = dict(&[("by_alias", MbValue::from_bool(true))]);
    let dumped = unsafe { mb_schema_model_dump([model, data, by_alias].as_ptr(), 3) };
    assert_eq!(
        (ops().dict_get_str)(dumped, "user_id").and_then(|v| v.as_int()),
        Some(2)
    );
    assert!(
        (ops().dict_get_str)(dumped, "id").is_none(),
        "by_alias dump should use serialization aliases"
    );

    let json = unsafe { mb_schema_model_dump_json([model, data, by_alias].as_ptr(), 3) };
    let doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(json).unwrap()).expect("dump json");
    assert_eq!(doc["user_id"].as_i64(), Some(2));
    assert_eq!(doc["code"].as_str(), Some("AB"));

    let invalid = dict(&[
        ("userId", MbValue::from_int(0)),
        ("accountCode", wrap_str("AB")),
    ]);
    let result = unsafe { mb_schema_model_dump([model, invalid].as_ptr(), 2) };
    let error = (ops().str_read)(result).expect("boundary failure should return error");
    assert!(error.contains("ValidationError"), "{error}");

    let schema = unsafe { mb_schema_model_json_schema([model].as_ptr(), 1) };
    let schema_doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(schema).unwrap()).expect("schema json");
    assert_eq!(
        schema_doc["properties"]["user_id"]["exclusiveMinimum"].as_i64(),
        Some(0)
    );
    assert_eq!(
        schema_doc["properties"]["user_id"]["exclusiveMaximum"].as_i64(),
        Some(10)
    );
    assert_eq!(
        schema_doc["properties"]["code"]["pattern"].as_str(),
        Some("^[A-Z]{2}$")
    );
}

#[test]
fn mb_schema_field_metadata_constraints_emit_json_schema() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
        mb_schema_model_dump_json, mb_schema_model_json_schema,
    };

    let model = unsafe { mb_schema_base_model_new([wrap_str("CatalogItem")].as_ptr(), 1) };
    let name = unsafe {
        mb_schema_field(
            [
                wrap_str("name"),
                dict(&[
                    ("type", wrap_str("str")),
                    ("title", wrap_str("Display Name")),
                    ("description", wrap_str("Public catalog name")),
                    (
                        "examples",
                        (ops().list_new)(vec![wrap_str("widget"), wrap_str("gadget")]),
                    ),
                    ("deprecated", MbValue::from_bool(true)),
                    ("read_only", MbValue::from_bool(true)),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    let quantity = unsafe {
        mb_schema_field(
            [
                wrap_str("quantity"),
                dict(&[
                    ("type", wrap_str("int")),
                    ("minimum", MbValue::from_int(0)),
                    ("multiple_of", MbValue::from_int(5)),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    let tags = unsafe {
        mb_schema_field(
            [
                wrap_str("tags"),
                dict(&[
                    ("type", wrap_str("list[str]")),
                    ("min_length", MbValue::from_int(1)),
                    ("max_length", MbValue::from_int(2)),
                    ("writeOnly", MbValue::from_bool(true)),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    for field in [name, quantity, tags] {
        let _ = unsafe { mb_schema_model_add_field([model, field].as_ptr(), 2) };
    }

    let schema = unsafe { mb_schema_model_json_schema([model].as_ptr(), 1) };
    let doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(schema).expect("schema json"))
            .expect("schema should parse");
    assert_eq!(
        doc["properties"]["name"]["title"].as_str(),
        Some("Display Name")
    );
    assert_eq!(
        doc["properties"]["name"]["description"].as_str(),
        Some("Public catalog name")
    );
    assert_eq!(
        doc["properties"]["name"]["deprecated"].as_bool(),
        Some(true)
    );
    assert_eq!(doc["properties"]["name"]["readOnly"].as_bool(), Some(true));
    assert_eq!(
        doc["properties"]["name"]["examples"]
            .as_array()
            .map(|items| items
                .iter()
                .filter_map(|item| item.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["widget", "gadget"])
    );
    assert_eq!(
        doc["properties"]["quantity"]["multipleOf"].as_i64(),
        Some(5)
    );
    assert_eq!(doc["properties"]["tags"]["minItems"].as_i64(), Some(1));
    assert_eq!(doc["properties"]["tags"]["maxItems"].as_i64(), Some(2));
    assert_eq!(doc["properties"]["tags"]["writeOnly"].as_bool(), Some(true));

    let valid = dict(&[
        ("name", wrap_str("widget")),
        ("quantity", MbValue::from_int(10)),
        ("tags", (ops().list_new)(vec![wrap_str("new")])),
    ]);
    let valid_json = unsafe { mb_schema_model_dump_json([model, valid].as_ptr(), 2) };
    let valid_doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(valid_json).expect("valid json"))
            .expect("valid dump should parse");
    assert_eq!(valid_doc["quantity"].as_i64(), Some(10));

    let invalid_multiple = dict(&[
        ("name", wrap_str("widget")),
        ("quantity", MbValue::from_int(7)),
        ("tags", (ops().list_new)(vec![wrap_str("new")])),
    ]);
    let invalid_multiple_result =
        unsafe { mb_schema_model_dump_json([model, invalid_multiple].as_ptr(), 2) };
    assert!((ops().str_read)(invalid_multiple_result)
        .is_some_and(|msg| msg.contains("ValidationError")));

    let invalid_tags = dict(&[
        ("name", wrap_str("widget")),
        ("quantity", MbValue::from_int(10)),
        ("tags", (ops().list_new)(vec![])),
    ]);
    let invalid_tags_result =
        unsafe { mb_schema_model_dump_json([model, invalid_tags].as_ptr(), 2) };
    assert!(
        (ops().str_read)(invalid_tags_result).is_some_and(|msg| msg.contains("ValidationError"))
    );
}

#[test]
fn mb_schema_model_json_schema_alias_matches_existing_schema() {
    use schema_mamba::methods::{mb_schema_model_json_schema, mb_schema_to_json_schema};
    mamba_registry::test_ops::init();

    let model = item_model_with_defaults();
    let schema = unsafe { mb_schema_to_json_schema([model].as_ptr(), 1) };
    let alias = unsafe { mb_schema_model_json_schema([model].as_ptr(), 1) };
    let schema_doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(schema).unwrap()).expect("schema is json");
    let alias_doc: serde_json::Value =
        serde_json::from_str(&(ops().str_read)(alias).unwrap()).expect("alias schema is json");
    assert_eq!(alias_doc, schema_doc);
}

#[test]
fn mb_schema_to_json_schema_includes_required_defaults_and_items() {
    use schema_mamba::methods::{
        mb_schema_base_model_new, mb_schema_field, mb_schema_model_add_field,
        mb_schema_to_json_schema,
    };
    mamba_registry::test_ops::init();

    let model = unsafe { mb_schema_base_model_new([wrap_str("ItemCreate")].as_ptr(), 1) };
    let name = unsafe {
        mb_schema_field(
            [
                wrap_str("name"),
                dict(&[
                    ("type", wrap_str("str")),
                    ("min_length", MbValue::from_int(3)),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    let age = unsafe {
        mb_schema_field(
            [
                wrap_str("age"),
                dict(&[("type", wrap_str("int")), ("minimum", MbValue::from_int(1))]),
            ]
            .as_ptr(),
            2,
        )
    };
    let active = unsafe {
        mb_schema_field(
            [
                wrap_str("active"),
                dict(&[
                    ("type", wrap_str("bool")),
                    ("default", MbValue::from_bool(true)),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    let tags = unsafe {
        mb_schema_field(
            [
                wrap_str("tags"),
                dict(&[
                    ("type", wrap_str("list[str]")),
                    ("default", (ops().list_new)(vec![])),
                ]),
            ]
            .as_ptr(),
            2,
        )
    };
    for field in [name, age, active, tags] {
        let _ = unsafe { mb_schema_model_add_field([model, field].as_ptr(), 2) };
    }

    let schema = unsafe { mb_schema_to_json_schema([model].as_ptr(), 1) };
    let json = (ops().str_read)(schema).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&json).expect("schema should be json");
    assert_eq!(doc["title"].as_str(), Some("ItemCreate"));
    assert_eq!(doc["properties"]["age"]["type"].as_str(), Some("integer"));
    assert_eq!(doc["properties"]["age"]["minimum"].as_i64(), Some(1));
    assert_eq!(
        doc["properties"]["active"]["type"].as_str(),
        Some("boolean")
    );
    assert_eq!(doc["properties"]["active"]["default"].as_bool(), Some(true));
    assert_eq!(doc["properties"]["tags"]["type"].as_str(), Some("array"));
    assert_eq!(
        doc["properties"]["tags"]["items"]["type"].as_str(),
        Some("string")
    );
    assert!(doc["properties"]["tags"]["default"]
        .as_array()
        .is_some_and(|items| items.is_empty()));
    assert_eq!(
        doc["required"].as_array().map(|items| items
            .iter()
            .filter_map(|item| item.as_str())
            .collect::<Vec<_>>()),
        Some(vec!["age", "name"])
    );
}

#[test]
fn mb_schema_to_json_schema_returns_ptr() {
    use schema_mamba::methods::{mb_schema_base_model_new, mb_schema_to_json_schema};

    let name_ptr = wrap_str("MyModel");
    let model_args = [name_ptr];
    let model = unsafe { mb_schema_base_model_new(model_args.as_ptr(), model_args.len()) };

    let schema_args = [model];
    let result = unsafe { mb_schema_to_json_schema(schema_args.as_ptr(), schema_args.len()) };
    assert!(
        result.is_ptr(),
        "mb_schema_to_json_schema must return a string PTR"
    );

    // Verify the output is a valid JSON string starting with '{'.
    let json = (ops().str_read)(result).unwrap();
    assert!(
        json.starts_with('{'),
        "JSON schema should start with '{{', got: {json}"
    );
}
