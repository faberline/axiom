# Cclab Schema Mamba

## Brief

Cclab Schema Mamba is the Mamba native binding for `schema` dataclass and
model validation primitives.

It registers the primary `mambalibs.dataclasses` module plus the legacy
`cclab_schema_mamba` compatibility alias. The binding owns model definition
surfaces such as `BaseModel`, `DataClass`, `Field`, `create_model`, and bound
method getters, plus validation, JSON parsing, model dumping, JSON Schema
generation, alias/default/coercion handling, nested model support, and
framework-facing validation detail helpers.

## Capabilities

A promise with no gate under it is not claimed.

### Capability Index

| Capability | Root WI | Notes |
|---|---:|---|
| Mamba Dataclass Model Definition Binding | - | defines BaseModel/DataClass/Field/create_model surface for Mamba dataclass-style models |
| Mamba Model Validation Dump And Schema Binding | - | validates, normalizes, dumps, parses JSON, and emits JSON Schema through the Mamba binding |

### Mamba Dataclass Model Definition Binding

Cclab Schema Mamba exposes `schema` model definition primitives to Mamba
through the `mambalibs.dataclasses` module, with a compatibility alias,
BaseModel/DataClass constructors, Field descriptors, dynamic model creation,
field registration, and bound model method dispatch.

- Root WI: none; this capability predates the tracker.
- Surfaces: Mamba module: `mambalibs.dataclasses`; Compatibility module:
  `cclab_schema_mamba`; Native ABI: `BaseModel`, `DataClass`, `Field`,
  `create_model`, `add_field`, `add_fields`, bound method getters
- Gate — behavior: `cargo test -p schema-mamba`
- Gate: `cargo test -p schema-mamba`
- Source: `apps/mamba/crates/schema-mamba/src/lib.rs`,
  `apps/mamba/crates/schema-mamba/src/types.rs`,
  `apps/mamba/crates/schema-mamba/tests/test_binding.rs`
- Evidence: `cargo test -p schema-mamba`;
  apps/mamba/crates/schema-mamba/src/lib.rs; apps/mamba/crates/schema-mamba/src/types.rs;
  apps/mamba/crates/schema-mamba/tests/test_binding.rs

### Mamba Model Validation Dump And Schema Binding

Cclab Schema Mamba exposes validated model execution to Mamba scripts,
including boolean validation, normalized dict output, JSON input parsing,
compact JSON dumping, JSON Schema generation, aliases/defaults/coercion, nested
models, and framework-facing validation detail helpers.

- Root WI: none; this capability predates the tracker.
- Surfaces: Mamba module: `mambalibs.dataclasses`; Native ABI: `validate`,
  `model_validate`, `parse_obj`, `model_validate_json`, `parse_raw`,
  `model_dump`, `model_dump_json`, `to_json_schema`, `model_json_schema`,
  `field_validator`
- Gate — behavior: `cargo test -p schema-mamba`
- Gate: `cargo test -p schema-mamba`
- Source: `apps/mamba/crates/schema-mamba/src/methods.rs`,
  `apps/mamba/crates/schema-mamba/src/types.rs`,
  `apps/mamba/crates/schema-mamba/tests/test_binding.rs`
- Evidence: `cargo test -p schema-mamba`;
  apps/mamba/crates/schema-mamba/src/methods.rs;
  apps/mamba/crates/schema-mamba/src/types.rs;
  apps/mamba/crates/schema-mamba/tests/test_binding.rs
