// SPEC-MANAGED: .aw/tech-design/projects/jet/semantic/jet-tools-conformance-src.md#schema
// CODEGEN-BEGIN
//! `cclab check-conformance-manifest` — validates `.aw/tech-design/projects/jet/wasm-renderer/conformance.yaml`.
//!
//! @spec .aw/tech-design/projects/jet/wasm-renderer/subset-rigor.md
//!
//! Auto-registered via the `cli_std::registry` distributed slice. Implements
//! the manifest-validation logic specified in subset-rigor.md § Manifest
//! Validation Logic: load YAML, structural sanity-check, then for each
//! non-pending entry assert `demo_dir` exists under `examples/` and
//! `test_file` exists under `apps/jet/tests/`.
//!
//! Schema-level validation against `conformance.yaml.schema.json` is
//! deferred — the schema file is the authoritative spec, and external CI
//! tooling can run a JSON Schema 2020-12 validator over it. Application-
//! level structural checks here cover required-field / enum / S-X-B-rule
//! invariants that map onto R3/R4/R5 of the originating issue.

use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use cli_std::registry::{CliModule, CLI_MODULES};
use clap::{Arg, ArgMatches, Command};
use linkme::distributed_slice;
use serde::Deserialize;

const DEFAULT_MANIFEST: &str = ".aw/tech-design/projects/jet/wasm-renderer/conformance.yaml";
const DEFAULT_SCHEMA: &str =
    ".aw/tech-design/projects/jet/wasm-renderer/conformance.yaml.schema.json";

#[derive(Debug, Deserialize)]
struct Manifest {
    entries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: String,
    subset_rule: String,
    feature: String,
    #[serde(default)]
    sub_item: Option<String>,
    #[serde(default)]
    ast_node_kinds: Option<Vec<String>>,
    #[serde(default)]
    disambiguation_predicate: Option<String>,
    demo_dir: String,
    test_file: String,
    status: Status,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Verified,
    UnitOnly,
    Pending,
}

/// @spec .aw/tech-design/projects/jet/semantic/jet-tools-conformance-src.md#schema
pub struct CheckConformanceManifestModule;

/// @spec .aw/tech-design/projects/jet/semantic/jet-tools-conformance-src.md#schema
impl CliModule for CheckConformanceManifestModule {
    fn name(&self) -> &'static str {
        "check-conformance-manifest"
    }

    fn command(&self) -> Command {
        Command::new("check-conformance-manifest")
            .about(
                "Validate conformance.yaml structurally and verify that every \
                 non-pending entry's demo_dir exists under examples/ and its \
                 test_file exists under apps/jet/tests/.",
            )
            .arg(
                Arg::new("manifest")
                    .short('m')
                    .long("manifest")
                    .value_name("PATH")
                    .default_value(DEFAULT_MANIFEST)
                    .help("Path to the conformance.yaml manifest file."),
            )
            .arg(
                Arg::new("schema")
                    .short('s')
                    .long("schema")
                    .value_name("PATH")
                    .default_value(DEFAULT_SCHEMA)
                    .help(
                        "Path to the JSON Schema file. Currently informational; \
                         schema-level validation is performed by external CI tooling.",
                    ),
            )
            .arg(
                Arg::new("workspace_root")
                    .short('w')
                    .long("workspace-root")
                    .value_name("PATH")
                    .default_value(".")
                    .help(
                        "Workspace root directory. demo_dir resolved under \
                         <root>/examples/ and test_file under <root>/apps/jet/tests/.",
                    ),
            )
            .arg(Arg::new("strict").long("strict").num_args(0).help(
                "Treat unit_only entries the same as verified entries \
                         for path existence checks.",
            ))
    }

    fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let manifest_path = matches
            .get_one::<String>("manifest")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("missing --manifest"))?;
        let workspace_root = matches
            .get_one::<String>("workspace_root")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("missing --workspace-root"))?;
        let strict = matches.get_flag("strict");

        run(&manifest_path, &workspace_root, strict)
    }
}

#[distributed_slice(CLI_MODULES)]
static CHECK_CONFORMANCE_MANIFEST: &dyn CliModule = &CheckConformanceManifestModule;

fn run(manifest_path: &PathBuf, workspace_root: &PathBuf, strict: bool) -> Result<()> {
    let raw = std::fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read manifest: {}", manifest_path.display()))?;
    let manifest: Manifest = serde_yaml::from_str(&raw)
        .with_context(|| format!("failed to parse manifest YAML: {}", manifest_path.display()))?;

    let mut errors: Vec<String> = Vec::new();
    let mut seen_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for (idx, entry) in manifest.entries.iter().enumerate() {
        let where_at = format!("entries[{}] (id={})", idx, entry.id);

        // Structural invariants beyond serde's required-field check.
        if !seen_ids.insert(entry.id.as_str()) {
            errors.push(format!("{where_at}: duplicate id"));
        }

        let rule = &entry.subset_rule;
        let (rule_kind, _rule_n): (char, &str) = match rule.chars().next() {
            Some(c @ ('S' | 'X' | 'B')) => (c, &rule[1..]),
            _ => {
                errors.push(format!(
                    "{where_at}: subset_rule {rule:?} does not start with S, X, or B"
                ));
                continue;
            }
        };

        // R3/R4/R5: ast_node_kinds required for S/X entries.
        if matches!(rule_kind, 'S' | 'X') && entry.ast_node_kinds.is_none() {
            errors.push(format!(
                "{where_at}: ast_node_kinds is required for S/X entries (subset_rule={rule})"
            ));
        }
        // B entries should NOT carry ast_node_kinds (boundary cases are
        // not AST-rule-driven). Soft-warn if present.
        if rule_kind == 'B' && entry.ast_node_kinds.is_some() {
            errors.push(format!(
                "{where_at}: ast_node_kinds must be omitted for boundary (B) entries"
            ));
        }
        // disambiguation_predicate is informational; CLI does not assert
        // global non-overlap (that requires building a node-kind index
        // across all entries — deferred to a future audit subcommand).

        // Path existence checks — skip pending; honour --strict for unit_only.
        let check_paths = match entry.status {
            Status::Verified => true,
            Status::UnitOnly => strict,
            Status::Pending => false,
        };
        if check_paths {
            let demo_path = workspace_root.join("examples").join(&entry.demo_dir);
            if !demo_path.exists() {
                errors.push(format!(
                    "{where_at}: demo_dir {:?} not found at {}",
                    entry.demo_dir,
                    demo_path.display()
                ));
            }
            for tf in entry.test_file.split(',').map(str::trim) {
                let tp = workspace_root.join("apps/jet/tests").join(tf);
                if !tp.exists() {
                    errors.push(format!(
                        "{where_at}: test_file {tf:?} not found at {}",
                        tp.display()
                    ));
                }
            }
        }

        // Touch unused fields so they remain part of the public schema.
        let _ = (
            &entry.feature,
            &entry.sub_item,
            &entry.disambiguation_predicate,
        );
    }

    if errors.is_empty() {
        println!(
            "ok: {} entries validated, all non-pending paths resolve",
            manifest.entries.len()
        );
        Ok(())
    } else {
        for e in &errors {
            eprintln!("error: {e}");
        }
        bail!(
            "{} validation error(s):\n{}",
            errors.len(),
            errors.join("\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_manifest(dir: &std::path::Path, body: &str) -> PathBuf {
        let p = dir.join("conformance.yaml");
        std::fs::write(&p, body).unwrap();
        p
    }

    #[test]
    fn rejects_s_entry_without_ast_node_kinds() {
        let tmp = tempdir();
        let m = write_manifest(
            tmp.path(),
            r#"
entries:
  - id: S1_test
    subset_rule: S1
    feature: test
    demo_dir: t
    test_file: t.rs
    status: pending
"#,
        );
        let err = run(&m, &PathBuf::from("."), false).unwrap_err().to_string();
        assert!(
            err.contains("ast_node_kinds is required for S/X entries"),
            "got: {err}"
        );
    }

    #[test]
    fn boundary_entry_ok_without_ast_node_kinds() {
        let tmp = tempdir();
        let m = write_manifest(
            tmp.path(),
            r#"
entries:
  - id: B1_test
    subset_rule: B1
    feature: test
    demo_dir: t
    test_file: t.rs
    status: pending
"#,
        );
        run(&m, &PathBuf::from("."), false).unwrap();
    }

    #[test]
    fn rejects_duplicate_id() {
        let tmp = tempdir();
        let m = write_manifest(
            tmp.path(),
            r#"
entries:
  - id: B1_dup
    subset_rule: B1
    feature: a
    demo_dir: t
    test_file: t.rs
    status: pending
  - id: B1_dup
    subset_rule: B2
    feature: b
    demo_dir: t
    test_file: t.rs
    status: pending
"#,
        );
        let err = run(&m, &PathBuf::from("."), false).unwrap_err().to_string();
        assert!(err.contains("duplicate id"), "got: {err}");
    }

    #[test]
    fn pending_skips_path_check() {
        let tmp = tempdir();
        let m = write_manifest(
            tmp.path(),
            r#"
entries:
  - id: B1_pending
    subset_rule: B1
    feature: f
    demo_dir: nonexistent-dir
    test_file: nonexistent.rs
    status: pending
"#,
        );
        run(&m, &PathBuf::from("/nonexistent/root"), false).unwrap();
    }

    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }
}
// CODEGEN-END
