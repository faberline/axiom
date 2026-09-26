# Mamba Libraries

`mambalibs` contains Mamba-native libraries that extend the CPython-compatible
stdlib surface without replacing it.

## Extension and Compatibility

Mamba libraries may add native methods, functions, classes, and module
namespaces when that improves DX. Compatibility means those extensions do not
change CPython stdlib syntax or behavior. Normal Python syntax remains the
carrier for metadata: decorators, class definitions, annotations, and default
values can feed Mamba-native contracts without changing how the stdlib works.

## Namespace Policy

Use stdlib-name-first naming when CPython already has a namespace for the
domain:

- CPython-compatible baseline behavior stays in `runtime/stdlib`.
- Mamba-native enhanced capabilities live under `mambalibs.<stdlib-name>`.
- Third-party ecosystems are compatibility shims layered on top of the stdlib
  anchor, not the primary abstraction.

Current anchors:

| Domain | Primary Mamba namespace | Internal crate area | Compatibility surfaces | CPython wheel |
| --- | --- | --- | --- | --- |
| HTTP clients, app routing, API helpers | `mambalibs.http` | `httpkit` | later `fastapi`, `requests`, `httpx` | — |
| Dependency injection and provider scopes | `mambalibs.di` | `dikit` | `mambalibs.http.Depends` adapter | — |
| Dataclass/schema models | `mambalibs.dataclasses` | `schema` + `schema-mamba` | `cclab_schema_mamba` compat; later `pydantic`, `marshmallow`, `jsonschema` | — |
| Logging | `mambalibs.logging` | pending | later `loguru`, `structlog` | — |
| Arrays and low-level vectors | `mambalibs.array` | `arraykit` | later NumPy-style shims if needed | `mambalibs-arraykit` (`cp312-abi3`) |
| Scientific computing | `mambalibs.sci` | `scikit` | later SciPy-style shims if needed | `mambalibs-scikit` (`cp312-abi3`) |
| Plotting and charts | `mambalibs.plot` | `plotkit` | later Matplotlib-style shims if needed | `mambalibs-plotkit` (`cp312-abi3`) |
| Queues and task routing | `mambalibs.queue` | `queuekit` | later Celery/RQ-style shims if needed | — |
| PostgreSQL | `mambalibs.pg` | `pgkit` | no CPython stdlib anchor | — |
| SQLite | `sqlite3` in stdlib | runtime stdlib | `mambalibs.db` only if a common DB contract proves useful | — |

Directory names may keep historical `*kit` crate names when they are useful as
Rust implementation boundaries. Runtime-facing Mamba imports should follow the
table above.

## CPython wheels

`mambalibs.array`, `mambalibs.sci`, and `mambalibs.plot` ship as PyO3 `abi3`
wheels tagged `cp312-abi3` — one wheel per platform, serving CPython 3.12 and
later. The namespace stays PEP 420: no wheel ships a `mambalibs/__init__.py`,
so all three install side by side in one `.venv`.

Three `[[test]]` targets exercise them:

- `mambalibs_array_wheel`
- `mambalibs_sci_wheel`
- `mambalibs_plot_wheel`

Run all three with:

```
cargo test -p mamba --test mambalibs_array_wheel --test mambalibs_sci_wheel --test mambalibs_plot_wheel
```

The host must provide `maturin` on `PATH` and a `python3.12` that can `import
pytest`; a missing tool fails the case rather than skipping it. In
`mamba@0.1.0` these cases drive `maturin build` themselves — `mamba package`
does not build these wheels.

## FastAPI-Class DX Target

The HTTP, DI, and dataclass/schema anchors are intended to compose into a
FastAPI + Pydantic + Uvicorn-class developer experience:

- `mambalibs.http` owns app, route, request/response, status, client, and host
  dispatch contracts.
- `mambalibs.di` owns provider registration, scopes, overrides, and dependency
  markers.
- `mambalibs.dataclasses` owns schema/dataclass model validation and JSON
  Schema generation.
- The shared endpoint contract records method/path, dependency keys, request
  model, response model, handler name, and status code so later decorator
  registration, validation, OpenAPI, and server hosting use one native model.

This is an extension target, not a stdlib replacement. Third-party-compatible
surfaces such as `fastapi`, `pydantic`, and `uvicorn` can layer on top of these
anchors after the native contracts are strong enough.
