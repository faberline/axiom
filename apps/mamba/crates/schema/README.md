# Cclab Schema

## Brief

Unified validation library for the cclab framework - Pydantic-like validation with Rust performance.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Validation Type System | - | Rust validation API for descriptors, constraints, formats, nested structures, and accumulated errors |
| Mamba Dataclass Schema Surface | - | Mamba-facing schema helpers and binding parity are behavior verified through schema and schema-mamba |
| Serialization And Storage Features | - | optional serde, sonic, and BSON feature-backed data formats |

### Validation Type System

schema provides the Rust validation foundation for cclab: descriptors for
primitive, collection, special, and format types; string/numeric/list/object
constraints; nested structures; and accumulated validation errors.

- Root WI: none; this capability predates the tracker.
- Surfaces: Rust API: `schema::{TypeDescriptor, Value, validate}` -
  type-safe validation entrypoint; Rust API: `constraints::*` - string,
  numeric, list, and field descriptors; Rust API: `formats` - precompiled
  email, URL, UUID, date, datetime, and time validators
- Gate — behavior:
  `cargo test -p schema --test validation_tests --test error_paths_tests --test field_validator_tests --test model_validator_tests`
  - validation, constraints, formats, nested structures, validators, and error
  accumulation
- Gate — efficiency: `cargo test -p schema` - zero-copy validation
  remains covered by crate tests until a dedicated meter cube exists
- Gate: `cargo test -p schema`
- Source: `apps/mamba/crates/schema/tests/validation_tests.rs`,
  `apps/mamba/crates/schema/tests/error_paths_tests.rs`,
  `apps/mamba/crates/schema/tests/field_validator_tests.rs`,
  `apps/mamba/crates/schema/tests/model_validator_tests.rs`
- Evidence: `cargo test -p schema`

### Mamba Dataclass Schema Surface

Mamba exposes schema through additive schema helpers such as `BaseModel`,
`Field`, `model_validate`, nullable field options, and nested model field
metadata without changing CPython stdlib `dataclasses` behavior.

- Root WI: none; this capability predates the tracker.
- Surfaces: Python module: `mambalibs.dataclasses` - additive schema helpers;
  Rust API:
  `dataclass::{DataclassDefinition, FieldInfo, infer_type_from_annotation}` -
  type inference and field metadata surface
- Gate — behavior: `cargo test -p schema` - schema helper behavior in the
  Rust crate
- Gate — behavior: `cargo test -p schema-mamba` - Mamba binding parity
  for model definition, validation, dumping, JSON parsing, JSON Schema,
  aliases, defaults, coercion, nested models, and validation detail helpers
- Gate: `cargo test -p schema`
- Gate: `cargo test -p schema-mamba`
- Source: `apps/mamba/crates/schema/src/dataclass.rs`,
  `apps/mamba/crates/schema-mamba/tests/test_binding.rs`
- Evidence: `cargo test -p schema`; `cargo test -p schema-mamba`;
  apps/mamba/crates/schema/src/dataclass.rs;
  apps/mamba/crates/schema-mamba/tests/test_binding.rs

### Serialization And Storage Features

schema exposes optional feature-backed data formats for JSON and BSON,
including serde_json serialization, sonic-rs high-performance JSON, MongoDB
BSON types, field/model serializers, and computed values.

- Root WI: none; this capability predates the tracker.
- Surfaces: Cargo feature: `serde` - JSON serialization through serde_json;
  Cargo feature: `sonic` / `fast` - high-performance JSON through sonic-rs;
  Cargo feature: `bson` - MongoDB BSON type support; Rust API: `serializers::*`
  - field/model serializer and computed field surface
- Gate — behavior: `cargo test -p schema --test serializer_tests` - serializer,
  computed field, include/exclude, and round-trip behavior
- Gate — efficiency: `cargo test -p schema` - sonic-backed JSON path
  remains declared until a dedicated meter cube exists
- Gate: `cargo test -p schema`
- Source: `apps/mamba/crates/schema/Cargo.toml`,
  `apps/mamba/crates/schema/tests/serializer_tests.rs`
- Evidence: `cargo test -p schema`

## Overview

`cclab-shield` provides a comprehensive, type-safe validation system for Rust, serving as the validation foundation for the entire cclab ecosystem:

- **cclab-quasar**: HTTP request validation
- **cclab**: MongoDB/BSON validation
- **cclab-titan**: PostgreSQL identifier validation
- **cclab-sheet-***: Spreadsheet validation

Mamba exposes this foundation through the additive `mambalibs.dataclasses`
namespace. New schema helpers such as `BaseModel`, `Field`, `model_validate`,
nullable field options, and nested `Field(..., {"model": User})` schemas extend
Mamba's framework surface without changing CPython stdlib `dataclasses` syntax
or behavior.

## Architecture Vision

```text
cclab.orbit      == uvloop             (event loop)
cclab.quasar         == uvicorn + fastapi  (web framework)
cclab.shield  == pydantic + orjson  (validation + JSON) ⭐
```

## Features

- **Type-Safe Validation**: 23 built-in type descriptors covering primitives, collections, and special types
- **Constraint Validation**: String length, numeric ranges, list constraints, and more
- **Format Validation**: Pre-compiled regex validators for email, URL, UUID, DateTime, Date, Time
- **Nested Structures**: Full support for nested objects, arrays, and complex types
- **Error Accumulation**: Collects all validation errors instead of failing on first error
- **Zero-Copy Performance**: Efficient validation without unnecessary allocations
- **Feature Flags**:
  - `serde`: JSON serialization with serde_json
  - `sonic`: High-performance JSON with sonic-rs (3-7x faster)
  - `bson`: MongoDB BSON type support

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
cclab-shield = { path = "../cclab-shield" }

# With JSON serialization
cclab-shield = { path = "../cclab-shield", features = ["serde"] }

# With high-performance JSON
cclab-shield = { path = "../cclab-shield", features = ["sonic"] }

# With BSON support
cclab-shield = { path = "../cclab-shield", features = ["bson"] }
```

## Quick Start

### Basic Validation

```rust
use cclab_shield::{TypeDescriptor, Value, validate};

// Email validation
let email_type = TypeDescriptor::Email;
let value = Value::String("user@example.com".to_string());
assert!(validate(&value, &email_type).is_ok());

// Invalid email
let invalid = Value::String("not-an-email".to_string());
assert!(validate(&invalid, &email_type).is_err());
```

### String Validation with Constraints

```rust
use cclab_shield::{TypeDescriptor, Value, validate};
use cclab_shield::constraints::StringConstraints;

let constraints = StringConstraints {
    min_length: Some(5),
    max_length: Some(20),
    pattern: Some(r"^[a-zA-Z0-9_]+$".to_string()),
    format: None,
};

let type_desc = TypeDescriptor::String(constraints);
let value = Value::String("valid_user123".to_string());
assert!(validate(&value, &type_desc).is_ok());
```

### Numeric Validation

```rust
use cclab_shield::{TypeDescriptor, Value, validate};
use cclab_shield::constraints::NumericConstraints;

let constraints = NumericConstraints {
    minimum: Some(0),
    maximum: Some(100),
    exclusive_minimum: None,
    exclusive_maximum: None,
    multiple_of: Some(5),
};

let type_desc = TypeDescriptor::Int64(constraints);
assert!(validate(&Value::Int(50), &type_desc).is_ok());
assert!(validate(&Value::Int(101), &type_desc).is_err()); // Too large
assert!(validate(&Value::Int(7), &type_desc).is_err()); // Not multiple of 5
```

### Object Validation

```rust
use cclab_shield::{TypeDescriptor, Value, validate};
use cclab_shield::constraints::FieldDescriptor;

let user_type = TypeDescriptor::Object {
    fields: vec![
        FieldDescriptor {
            name: "name".to_string(),
            type_desc: TypeDescriptor::String(Default::default()),
            required: true,
            default: None,
            description: Some("User's full name".to_string()),
        },
        FieldDescriptor {
            name: "email".to_string(),
            type_desc: TypeDescriptor::Email,
            required: true,
            default: None,
            description: Some("User's email address".to_string()),
        },
        FieldDescriptor {
            name: "age".to_string(),
            type_desc: TypeDescriptor::Int64(Default::default()),
            required: false,
            default: Some(Value::Int(18)),
            description: Some("User's age".to_string()),
        },
    ],
    additional: None, // Don't allow extra fields
};

let user = Value::Object(vec![
    ("name".to_string(), Value::String("Alice".to_string())),
    ("email".to_string(), Value::String("alice@example.com".to_string())),
    ("age".to_string(), Value::Int(30)),
]);

assert!(validate(&user, &user_type).is_ok());
```

### List Validation

```rust
use cclab_shield::{TypeDescriptor, Value, validate};
use cclab_shield::constraints::ListConstraints;

let list_type = TypeDescriptor::List {
    items: Box::new(TypeDescriptor::Int64(Default::default())),
    constraints: ListConstraints {
        min_items: Some(1),
        max_items: Some(10),
        unique_items: true, // All items must be unique
    },
};

let valid_list = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
assert!(validate(&valid_list, &list_type).is_ok());

let duplicate_list = Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(1)]);
assert!(validate(&duplicate_list, &list_type).is_err()); // Has duplicates
```

### Union Types

```rust
use cclab_shield::{TypeDescriptor, Value, validate};

let union_type = TypeDescriptor::Union {
    variants: vec![
        TypeDescriptor::Int64(Default::default()),
        TypeDescriptor::String(Default::default()),
        TypeDescriptor::Email,
    ],
    nullable: true,
};

// All of these are valid
assert!(validate(&Value::Int(42), &union_type).is_ok());
assert!(validate(&Value::String("hello".to_string()), &union_type).is_ok());
assert!(validate(&Value::String("user@example.com".to_string()), &union_type).is_ok());
assert!(validate(&Value::Null, &union_type).is_ok());

// This is invalid (doesn't match any variant)
assert!(validate(&Value::Bool(true), &union_type).is_err());
```

### Optional Fields

```rust
use cclab_shield::{TypeDescriptor, Value, validate};

let optional_type = TypeDescriptor::Optional(
    Box::new(TypeDescriptor::Email)
);

assert!(validate(&Value::Null, &optional_type).is_ok());
assert!(validate(&Value::String("user@example.com".to_string()), &optional_type).is_ok());
assert!(validate(&Value::String("invalid".to_string()), &optional_type).is_err());
```

### Enum/Literal Types

```rust
use cclab_shield::{TypeDescriptor, Value, validate};

// Enum: value must match one of the allowed values
let color_type = TypeDescriptor::Enum {
    values: vec![
        Value::String("red".to_string()),
        Value::String("green".to_string()),
        Value::String("blue".to_string()),
    ],
};

assert!(validate(&Value::String("red".to_string()), &color_type).is_ok());
assert!(validate(&Value::String("yellow".to_string()), &color_type).is_err());
```

## Python Usage

When built with the `python` feature, the validation module is accessible from Python:

```python
from cclab.shield import validate

# Email validation
type_desc = {"type": "email"}
validate("user@example.com", type_desc)  # ✅ Success

# String with constraints
type_desc = {
    "type": "string",
    "constraints": {
        "min_length": 3,
        "max_length": 100,
        "pattern": "^[a-z]+$"
    }
}
validate("hello", type_desc)  # ✅ Success
validate("hi", type_desc)     # ❌ ValueError: String too short

# Object validation
type_desc = {
    "type": "object",
    "fields": [
        {
            "name": "email",
            "type": {"type": "email"},
            "required": True
        },
        {
            "name": "age",
            "type": {
                "type": "int64",
                "constraints": {"minimum": 0, "maximum": 150}
            },
            "required": False
        }
    ]
}
validate({"email": "user@example.com", "age": 25}, type_desc)  # ✅ Success
```

See [`examples/python_usage.py`](examples/python_usage.py) for comprehensive examples.

## Type System

### Primitive Types

- `String(StringConstraints)` - String with optional length, pattern, format constraints
- `Int64(NumericConstraints<i64>)` - Integer with optional min/max, multiple_of
- `Float64(NumericConstraints<f64>)` - Float with optional min/max, multiple_of
- `Bool` - Boolean value
- `Null` - Null value
- `Bytes` - Binary data

### Collection Types

- `List { items, constraints }` - Array with item type and constraints
- `Tuple { items }` - Fixed-length ordered collection
- `Set { items }` - Unique items only
- `Object { fields, additional }` - Named fields with types

### Special Types

- `Optional(inner)` - Nullable type
- `Union { variants, nullable }` - One of multiple types
- `Enum { values }` - Value must match one of the allowed values
- `Literal { values }` - Same as Enum
- `Any` - No validation

### Format Types

- `Email` - Email address (pre-compiled regex)
- `Url` - HTTP/HTTPS URL (pre-compiled regex)
- `Uuid` - UUID v4 (pre-compiled regex)
- `DateTime` - ISO 8601 DateTime (pre-compiled regex)
- `Date` - YYYY-MM-DD (pre-compiled regex)
- `Time` - HH:MM:SS with optional fractional seconds (pre-compiled regex)
- `Decimal(constraints)` - High precision decimal

### BSON Types (feature = "bson")

- `ObjectId` - MongoDB ObjectId
- `BsonDateTime` - BSON DateTime
- `BsonDecimal128` - BSON Decimal128
- `BsonBinary` - BSON Binary data

## Error Handling

The validation system accumulates all errors instead of failing on first error:

```rust
use cclab_shield::{TypeDescriptor, Value, validate};
use cclab_shield::constraints::*;

let type_desc = TypeDescriptor::String(StringConstraints {
    min_length: Some(5),
    max_length: Some(10),
    pattern: Some(r"^[a-z]+$".to_string()),
    format: None,
});

let value = Value::String("ABC".to_string());
match validate(&value, &type_desc) {
    Ok(_) => println!("Valid!"),
    Err(errors) => {
        println!("Validation failed with {} errors:", errors.len());
        for error in errors.as_slice() {
            println!("  - {}: {}", error.field, error.message);
        }
    }
}
```

## Performance

- **Pre-compiled Regex**: Format validators use lazy-initialized regex patterns
- **Zero-Copy**: Validation doesn't clone values unnecessarily
- **Early Returns**: Stops validating once a type mismatch is detected
- **Efficient Iteration**: Uses iterators instead of collecting intermediate results

## Comparison to Pydantic

| Feature | Pydantic | cclab-shield |
|---------|----------|----------------------|
| Language | Python | Rust |
| Type System | Python type hints | TypeDescriptor enum |
| Performance | Fast (with Rust extensions) | Native Rust speed |
| Error Handling | Immediate failure or accumulate | Always accumulate all errors |
| Nested Validation | ✅ | ✅ |
| Custom Validators | ✅ | Custom TypeDescriptor |
| JSON Schema | ✅ | Planned (Phase 4) |

## Development Status

- ✅ **Phase 1**: Foundation (Complete)
  - Crate structure, type system (23 variants), constraints, errors, format validators
- ✅ **Phase 2**: Core Validation Engine (Complete)
  - Full validation logic, comprehensive tests (58 tests passing - 32 integration + 18 unit + 8 doc)
- ✅ **Phase 3**: Migration (Complete)
  - ✅ Phase 3.1: cclab-quasar migrated (2,070 → 435 lines, 79% reduction)
  - ✅ Phase 3.2: cclab-nebula created (1,030 lines pure Rust)
  - All security features preserved (NoSQL injection prevention, 37 tests passing)
- 📋 **Phase 4**: Schema Export (future)
  - 📋 JSON Schema generation for downstream language bindings

See repository history for the pre-Mamba migration metrics.

## Contributing

This crate is part of the cclab framework. See the main repository for contribution guidelines.

## License

MIT OR Apache-2.0
