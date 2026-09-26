//! Audit the Mamba binding's dependency on `pg`.
//!
//! Phase A (initial gate, now narrowed): the binding used to be
//! forbidden from referencing `pg::orm::*` outright while the
//! ORM `Session` surface was being designed. Since #2088 the binding
//! mounts `Session` / `SessionModel` / `SessionQuery` directly, so
//! the literal-namespace ban has been replaced with a narrower one
//! that still catches drift into ORM internals (`query`, `schema`,
//! `validation`) — those modules belong to the typed surface, not
//! the binding's dyn surface.
//!
//! Phase B (future): once each layer is extracted into its own crate,
//! this grep test retires and the absence of a `cclab-pg-orm`
//! dependency in `Cargo.toml` becomes the enforcement.

use std::fs;
use std::path::{Path, PathBuf};

/// Internal ORM submodules the binding must not reach into. The
/// public `Session` surface is reached via `pg::blocking::`
/// (the top-level aggregator) or its re-exports — not via these
/// raw paths.
const FORBIDDEN_FROM_BINDING: &[&str] = &[
    "pg::orm::query::",
    "pg::orm::schema::",
    "pg::orm::validation::",
    "pg::orm::session::sealed",
];

#[test]
fn binding_must_not_reach_into_orm_internals() {
    let src = src_root();
    assert!(
        src.is_dir(),
        "expected src/ at {}: re-check workspace layout",
        src.display()
    );

    let mut offenders: Vec<(PathBuf, &'static str)> = Vec::new();
    walk(&src, &mut |path| {
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            return;
        }
        let src =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for needle in FORBIDDEN_FROM_BINDING {
            if src.contains(needle) {
                offenders.push((path.to_path_buf(), needle));
            }
        }
    });

    assert!(
        offenders.is_empty(),
        "binding/src/ must reach the ORM only through the public \
         `Session` / `SessionModel` surface — not internal submodules. \
         Offenders:\n{}",
        offenders
            .iter()
            .map(|(p, n)| format!("  - {} contains '{}'", p.display(), n))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn binding_execute_must_delegate_to_driver_connection() {
    let methods = src_root().join("methods.rs");
    let src =
        fs::read_to_string(&methods).unwrap_or_else(|e| panic!("read {}: {e}", methods.display()));
    let body = fn_body(&src, "mb_pg_execute");

    assert!(
        body.contains("conn.inner.execute(&sql)"),
        "mb_pg_execute must route through the driver Connection facade"
    );
    assert!(
        !body.contains("sqlx::query"),
        "mb_pg_execute must not bypass the driver layer with direct sqlx::query"
    );
    assert!(
        !body.contains(".pool()") && !body.contains(".runtime()"),
        "mb_pg_execute should not manually drive the pool/runtime"
    );
}

#[test]
fn binding_execute_params_must_delegate_to_driver_connection() {
    let methods = src_root().join("methods.rs");
    let src =
        fs::read_to_string(&methods).unwrap_or_else(|e| panic!("read {}: {e}", methods.display()));
    let body = fn_body(&src, "mb_pg_execute_params");

    assert!(
        body.contains("conn.inner.execute_params(&sql, &params)"),
        "mb_pg_execute_params must route through the driver Connection facade"
    );
    assert!(
        !body.contains("sqlx::query"),
        "mb_pg_execute_params must not bypass the driver layer with direct sqlx::query"
    );
    assert!(
        !body.contains(".pool()") && !body.contains(".runtime()"),
        "mb_pg_execute_params should not manually drive the pool/runtime"
    );
}

#[test]
fn binding_query_all_must_delegate_to_driver_connection() {
    let methods = src_root().join("methods.rs");
    let src =
        fs::read_to_string(&methods).unwrap_or_else(|e| panic!("read {}: {e}", methods.display()));
    let body = fn_body(&src, "mb_pg_query_all");

    assert!(
        body.contains("conn.inner.fetch_rows(&sql, &params)"),
        "mb_pg_query_all must route through the driver Connection facade"
    );
    assert!(
        !body.contains("sqlx::query"),
        "mb_pg_query_all must not bypass the driver layer with direct sqlx::query"
    );
    assert!(
        !body.contains(".pool()") && !body.contains(".runtime()"),
        "mb_pg_query_all should not manually drive the pool/runtime"
    );
}

#[test]
fn binding_query_one_must_delegate_to_driver_connection() {
    let methods = src_root().join("methods.rs");
    let src =
        fs::read_to_string(&methods).unwrap_or_else(|e| panic!("read {}: {e}", methods.display()));
    let body = fn_body(&src, "mb_pg_query_one");

    assert!(
        body.contains("conn.inner.fetch_optional_row(&sql, &params)"),
        "mb_pg_query_one must route through the driver Connection facade"
    );
    assert!(
        !body.contains("sqlx::query"),
        "mb_pg_query_one must not bypass the driver layer with direct sqlx::query"
    );
    assert!(
        !body.contains(".pool()") && !body.contains(".runtime()"),
        "mb_pg_query_one should not manually drive the pool/runtime"
    );
}

fn src_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn fn_body<'a>(src: &'a str, name: &str) -> &'a str {
    let marker = format!("fn {name}");
    let start = src
        .find(&marker)
        .unwrap_or_else(|| panic!("missing function {name}"));
    let open = start
        + src[start..]
            .find('{')
            .unwrap_or_else(|| panic!("missing function body for {name}"));

    let mut depth = 0i32;
    for (idx, ch) in src[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[open..open + idx + 1];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated function body for {name}");
}

fn walk(dir: &Path, visit: &mut dyn FnMut(&Path)) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, visit);
        } else {
            visit(&path);
        }
    }
}
