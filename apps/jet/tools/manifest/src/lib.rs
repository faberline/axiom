// SPEC-MANAGED: .aw/tech-design/projects/jet/semantic/jet-tools-manifest-src.md#schema
// CODEGEN-BEGIN
//! `cclab parse-manifest` — loads jet.declare.d.ts files and prints
//! the merged ParsedManifest as YAML.
//!
//! @spec .aw/tech-design/projects/jet/wasm-renderer/binding-manifest.md
//!
//! Auto-registered via the `cli_std::registry` distributed slice.
//! Walks the ancestor chain from `--source-dir` up to `--workspace-root`,
//! collecting every `jet.declare.d.ts`, then overlay-merges with
//! defaults seeded from [`jet_wasm::manifest::DEFAULT_BINDINGS`].

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use cli_std::registry::{CliModule, CLI_MODULES};
use clap::{Arg, ArgMatches, Command};
use jet_wasm::manifest::{parse_manifest, ExportKind, JetImpl, ParsedManifest};
use linkme::distributed_slice;

/// GH #3699 — `execute` previously did
/// `source_dir.canonicalize().unwrap_or_else(|_| source_dir.clone())` (and
/// the same for `workspace_root`). When canonicalize succeeded for one
/// input but failed for the other (typo, missing symlink, EACCES on
/// parent), `parse_manifest` got a mismatched pair: canonical absolute on
/// one side, the non-canonical raw input on the other. `ancestor_chain`
/// in `jet_wasm::manifest::parser` then compared `cursor == workspace_root`
/// by `PathBuf` byte-string equality, the equality gate never fired, and
/// the walk escaped to `/`, picking up every `jet.declare.d.ts` from
/// `$HOME`, `/etc`, etc. on the way up.
/// @spec .aw/tech-design/projects/jet/semantic/jet-tools-manifest-src.md#schema
pub(crate) fn format_safe_manifest_canonicalize_err(
    label: &str,
    path: &Path,
    err: &std::io::Error,
) -> String {
    format!(
        "GH #3699 jet parse-manifest: failed to canonicalize {label} = {:?} \
         ({err}). Refusing to fall back to the non-canonical input — that \
         would leave the source/workspace pair mismatched, and \
         ancestor_chain compares them by byte-string equality. The walk \
         would then escape the workspace and silently collect \
         jet.declare.d.ts files from outside the project (e.g. $HOME, \
         /etc). Check that {label} exists, is readable, and the entire \
         parent chain is traversable.",
        path
    )
}

/// @spec .aw/tech-design/projects/jet/semantic/jet-tools-manifest-src.md#schema
pub(crate) fn safe_manifest_canonicalize(path: &Path, label: &str) -> Result<PathBuf, String> {
    match path.canonicalize() {
        Ok(p) => Ok(p),
        Err(err) => Err(format_safe_manifest_canonicalize_err(label, path, &err)),
    }
}

/// @spec .aw/tech-design/projects/jet/semantic/jet-tools-manifest-src.md#schema
pub struct ParseManifestModule;

/// @spec .aw/tech-design/projects/jet/semantic/jet-tools-manifest-src.md#schema
impl CliModule for ParseManifestModule {
    fn name(&self) -> &'static str {
        "parse-manifest"
    }

    fn command(&self) -> Command {
        Command::new("parse-manifest")
            .about(
                "Walk ancestor jet.declare.d.ts files from --source-dir up to \
                 --workspace-root and print the overlay-merged ParsedManifest \
                 as YAML. Default bindings are seeded first; user manifests \
                 override on per-module conflict.",
            )
            .arg(
                Arg::new("source_dir")
                    .short('s')
                    .long("source-dir")
                    .value_name("PATH")
                    .default_value(".")
                    .help("Starting directory for the ancestor walk."),
            )
            .arg(
                Arg::new("workspace_root")
                    .short('w')
                    .long("workspace-root")
                    .value_name("PATH")
                    .default_value(".")
                    .help("Workspace root directory; the walk stops here."),
            )
    }

    fn execute(&self, matches: &ArgMatches) -> Result<()> {
        let source_dir = matches
            .get_one::<String>("source_dir")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("missing --source-dir"))?;
        let workspace_root = matches
            .get_one::<String>("workspace_root")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("missing --workspace-root"))?;
        // GH #3699 — was `.canonicalize().unwrap_or_else(|_| .clone())` on
        // both inputs, which silently produced a mismatched pair when one
        // canonicalize call failed. `ancestor_chain` compares cursor and
        // workspace_root by byte-string equality, so the mismatch caused
        // the walk to escape the workspace.
        let canonical_source =
            safe_manifest_canonicalize(&source_dir, "--source-dir").map_err(|msg| anyhow!(msg))?;
        let canonical_root = safe_manifest_canonicalize(&workspace_root, "--workspace-root")
            .map_err(|msg| anyhow!(msg))?;

        match parse_manifest(&canonical_source, &canonical_root) {
            Ok(manifest) => {
                println!("{}", render_yaml(&manifest));
                Ok(())
            }
            Err(e) => {
                let code = e.code.as_str();
                let path_segment = e
                    .path
                    .as_ref()
                    .map(|p| format!(" at {}", p.display()))
                    .unwrap_or_default();
                let line_segment = e.line.map(|l| format!(":line {l}")).unwrap_or_default();
                eprintln!("error [{code}]{path_segment}{line_segment}: {}", e.message);
                anyhow::bail!("manifest parse failed ({code})");
            }
        }
    }
}

#[distributed_slice(CLI_MODULES)]
static PARSE_MANIFEST: &dyn CliModule = &ParseManifestModule;

/// Minimal YAML renderer for the resolved manifest. Avoids pulling in
/// `serde_yaml` for output-only formatting; the schema lives in
/// `binding-manifest.schema.json` and is the source of truth.
fn render_yaml(m: &ParsedManifest) -> String {
    let mut out = String::new();
    out.push_str("entries:\n");
    for entry in &m.entries {
        out.push_str(&format!("  - module_name: {:?}\n", entry.module_name));
        out.push_str("    exports:\n");
        for ex in &entry.exports {
            let kind = match ex.kind {
                ExportKind::Default => "default",
                ExportKind::Named => "named",
            };
            out.push_str(&format!("      - kind: {kind}\n"));
            out.push_str(&format!("        name: {:?}\n", ex.name));
            if let Some(sig) = &ex.signature {
                out.push_str(&format!("        signature: {:?}\n", sig));
            }
        }
        out.push_str("    jet_impl:\n");
        match &entry.jet_impl {
            JetImpl::Rust { symbol } => {
                out.push_str("      discriminant: rust\n");
                out.push_str(&format!("      symbol: {:?}\n", symbol));
            }
            JetImpl::Bridge { symbol } => {
                out.push_str("      discriminant: bridge\n");
                out.push_str(&format!("      symbol: {:?}\n", symbol));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use jet_wasm::manifest::{ManifestErrorCode, DEFAULT_BINDINGS};

    #[test]
    fn renders_defaults_yaml() {
        let manifest = ParsedManifest {
            entries: DEFAULT_BINDINGS.iter().cloned().collect(),
        };
        let yaml = render_yaml(&manifest);
        assert!(yaml.contains("module_name: \"fetch\""));
        assert!(yaml.contains("module_name: \"console\""));
        assert!(yaml.contains("module_name: \"localStorage\""));
        assert!(yaml.contains("module_name: \"JSON\""));
        assert!(yaml.contains("discriminant: bridge"));
        assert!(yaml.contains("discriminant: rust"));
    }

    #[test]
    fn manifest_error_code_strings_match_spec() {
        // Smoke-check that the stable codes are referenced from the
        // spec correctly. The CLI prints these verbatim on failure.
        assert_eq!(
            ManifestErrorCode::FileNotFound.as_str(),
            "MANIFEST_PARSE_001"
        );
        assert_eq!(ManifestErrorCode::ParseError.as_str(), "MANIFEST_PARSE_002");
        assert_eq!(
            ManifestErrorCode::MissingModuleName.as_str(),
            "MANIFEST_PARSE_003"
        );
        assert_eq!(
            ManifestErrorCode::UnknownImplDiscriminant.as_str(),
            "MANIFEST_PARSE_004"
        );
        assert_eq!(
            ManifestErrorCode::DuplicateModule.as_str(),
            "MANIFEST_PARSE_005"
        );
    }
}

#[cfg(test)]
mod gh3699_safe_manifest_canonicalize_tests {
    //! GH #3699 — `execute` used to do
    //! `.canonicalize().unwrap_or_else(|_| .clone())` on both `--source-dir`
    //! and `--workspace-root`. When canonicalize succeeded on one input
    //! but failed on the other, `parse_manifest` got a mismatched pair
    //! (canonical absolute vs. non-canonical raw). `ancestor_chain` compares
    //! cursor and workspace_root via byte-string equality, so the equality
    //! gate never fired and the walk escaped to `/`, sweeping up
    //! `jet.declare.d.ts` from `$HOME`, `/etc`, etc.
    use super::*;
    use std::path::Path;

    #[test]
    fn happy_existing_dir_canonicalizes() {
        let tmp = tempfile::tempdir().unwrap();
        let got = safe_manifest_canonicalize(tmp.path(), "--source-dir").unwrap();
        // canonicalize resolves symlinks (e.g. /var -> /private/var on
        // macOS), so we cannot assert byte-equality with the original;
        // we just assert that the result is itself canonical (idempotent).
        let again = got.canonicalize().unwrap();
        assert_eq!(got, again);
    }

    #[test]
    fn enoent_returns_tagged_err() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("definitely-does-not-exist-3699");
        let err = safe_manifest_canonicalize(&missing, "--source-dir")
            .expect_err("missing path must error");
        assert!(err.contains("GH #3699"), "err: {err}");
    }

    #[test]
    fn err_names_label_so_user_knows_which_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing-root-3699");
        let err = safe_manifest_canonicalize(&missing, "--workspace-root")
            .expect_err("missing path must error");
        assert!(err.contains("--workspace-root"), "err: {err}");
    }

    #[test]
    fn err_names_path_so_user_knows_what_to_fix() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing-source-3699");
        let err = safe_manifest_canonicalize(&missing, "--source-dir")
            .expect_err("missing path must error");
        assert!(err.contains("missing-source-3699"), "err: {err}");
    }

    #[test]
    fn err_names_downstream_symptom_workspace_escape() {
        let err = format_safe_manifest_canonicalize_err(
            "--workspace-root",
            Path::new("/no/such/path"),
            &std::io::Error::from(std::io::ErrorKind::NotFound),
        );
        assert!(
            err.contains("escape the workspace"),
            "warn must name the walk-escape symptom: {err}"
        );
    }

    #[test]
    fn err_points_at_byte_string_equality_root_cause() {
        let err = format_safe_manifest_canonicalize_err(
            "--source-dir",
            Path::new("/no/such/path"),
            &std::io::Error::from(std::io::ErrorKind::NotFound),
        );
        assert!(
            err.contains("byte-string equality") || err.contains("mismatched"),
            "warn must point at the root cause: {err}"
        );
    }

    #[test]
    fn err_round_trips_io_error_text() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "EACCES sample");
        let err =
            format_safe_manifest_canonicalize_err("--source-dir", Path::new("/blocked"), &io_err);
        assert!(err.contains("EACCES sample"), "err: {err}");
    }

    #[test]
    fn err_is_deterministic_for_fixed_inputs() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "fixed-msg");
        let a = format_safe_manifest_canonicalize_err("--source-dir", Path::new("/fixed"), &io_err);
        let b = format_safe_manifest_canonicalize_err("--source-dir", Path::new("/fixed"), &io_err);
        assert_eq!(a, b);
    }
}
// CODEGEN-END
