// CODEGEN-BEGIN
//! AST-assisted instrumentation-point discovery (feature `ast`).
//!
//! meter's capture mode samples a running binary from the outside; that yields
//! address/symbol-level hot spots but no source structure, so attributing cost
//! to a specific function — or asking for "finer measurement *here*" — means
//! guessing at boundaries. This module turns a source file into *precise* probe
//! points: every function/method with its exact name and 1-based line span, so
//! an agent can request measurement at real source boundaries.
//!
//! Boundary: the AST/parsing work is **not** meter's job — it is delegated to
//! `compass::outline` (the workspace code-intelligence engine in `libs/compass`,
//! which owns the per-language tree-sitter knowledge). This module is the thin
//! *policy* layer: it maps compass's language-agnostic [`FunctionDef`] list onto
//! meter's probe model and decides what is instrumentable. Probe *injection* and
//! sample→point attribution build on top of these points.
//!
//! STATUS: hand-written foundation filling the README's "AST-assisted
//! instrumentation is planned but not implemented" gap. To restore meter's
//! regenerability invariant it must be folded into the tech-design + cb
//! (gap-blocker: meter AST-assisted instrumentation). Gated behind `ast` so the
//! default lean binary never pulls compass / tree-sitter grammars.
//!
//! [`FunctionDef`]: compass::FunctionDef

use std::path::Path;

use compass::syntax::Language;
use compass::FunctionKind;
use serde::{Deserialize, Serialize};

/// What kind of source construct a probe point sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeKind {
    /// Free function (Rust `fn`, Python `def`, TS/JS `function`, Go `func`).
    /// Rust methods are also reported as `Function` (see [`FunctionKind`]).
    Function,
    /// A method the grammar distinguishes from a free function
    /// (TS/JS `method_definition`, Go `method_declaration`).
    Method,
}

impl From<FunctionKind> for ProbeKind {
    fn from(kind: FunctionKind) -> Self {
        match kind {
            FunctionKind::Function => ProbeKind::Function,
            FunctionKind::Method => ProbeKind::Method,
        }
    }
}

/// A precise, AST-identified location where meter can place a probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbePoint {
    /// Function or method name (`<anonymous>` when the grammar node has no
    /// `name` field).
    pub name: String,
    /// Source file the point was found in (a caller-supplied label).
    pub file: String,
    /// Language the file was parsed as (compass `Language::as_str`).
    pub language: String,
    /// What construct the point sits on.
    pub kind: ProbeKind,
    /// 1-based first line of the construct (inclusive).
    pub start_line: usize,
    /// 1-based last line of the construct (inclusive).
    pub end_line: usize,
}

impl ProbePoint {
    /// Number of source lines the construct spans (>= 1).
    pub fn line_span(&self) -> usize {
        self.end_line.saturating_sub(self.start_line) + 1
    }
}

/// Errors from probe-point discovery.
#[derive(Debug, thiserror::Error)]
pub enum InstrumentError {
    /// The file extension is outside meter's AST instrumentation scope.
    #[error("unsupported file extension for AST instrumentation: {0:?}")]
    UnsupportedLanguage(String),
    /// The source file could not be read.
    #[error("failed to read {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    /// compass could not parse / outline the source.
    #[error("failed to outline {0}")]
    Outline(String),
}

/// Map a file extension to a compass [`Language`] meter knows how to instrument.
/// Returns `None` for extensions outside meter's instrumentation scope.
pub fn language_for_extension(ext: &str) -> Option<Language> {
    match ext {
        "rs" => Some(Language::Rust),
        "py" | "pyi" => Some(Language::Python),
        "ts" | "tsx" => Some(Language::TypeScript),
        "js" | "jsx" => Some(Language::JavaScript),
        "go" => Some(Language::Go),
        _ => None,
    }
}

/// Discover every probe point in a single source file.
///
/// Reads `path`, delegates parsing/enumeration to [`compass::outline`],
/// and returns one [`ProbePoint`] per function/method with an exact 1-based line
/// span, ordered by `start_line`.
pub fn discover_probe_points(path: impl AsRef<Path>) -> Result<Vec<ProbePoint>, InstrumentError> {
    let path = path.as_ref();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let language = language_for_extension(ext)
        .ok_or_else(|| InstrumentError::UnsupportedLanguage(ext.to_string()))?;

    let source = std::fs::read_to_string(path).map_err(|source| InstrumentError::Read {
        path: path.display().to_string(),
        source,
    })?;

    discover_probe_points_in_source(&source, language, &path.display().to_string())
}

/// Discover probe points in an in-memory source string (testable without a file).
///
/// `file_label` is recorded verbatim on each [`ProbePoint::file`].
pub fn discover_probe_points_in_source(
    source: &str,
    language: Language,
    file_label: &str,
) -> Result<Vec<ProbePoint>, InstrumentError> {
    let defs = compass::outline(source, language)
        .map_err(|e| InstrumentError::Outline(format!("{file_label}: {e}")))?;
    let lang_str = language.as_str();

    Ok(defs
        .into_iter()
        .map(|d| ProbePoint {
            name: d.name,
            file: file_label.to_string(),
            language: lang_str.to_string(),
            kind: d.kind.into(),
            start_line: d.start_line,
            end_line: d.end_line,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_functions_map_to_probe_points_with_spans() {
        let src = "fn alpha() -> i32 {\n    1\n}\n\nstruct S;\nimpl S {\n    fn beta(&self) -> i32 {\n        2\n    }\n}\n";
        let points = discover_probe_points_in_source(src, Language::Rust, "demo.rs").unwrap();

        let names: Vec<&str> = points.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["alpha", "beta"],
            "both fns found, ordered by line"
        );

        let alpha = &points[0];
        assert_eq!(alpha.start_line, 1);
        assert_eq!(alpha.end_line, 3);
        assert_eq!(alpha.line_span(), 3);
        assert_eq!(alpha.language, "rust");
        assert_eq!(alpha.kind, ProbeKind::Function);
        assert_eq!(points[1].start_line, 7);
    }

    #[test]
    fn python_functions_are_discovered() {
        let src = "def gamma():\n    return 1\n\ndef delta(x):\n    return x\n";
        let points = discover_probe_points_in_source(src, Language::Python, "demo.py").unwrap();
        let names: Vec<&str> = points.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["gamma", "delta"]);
    }

    #[test]
    fn unsupported_extension_is_rejected() {
        let err = discover_probe_points("/tmp/nope.txt").unwrap_err();
        assert!(matches!(err, InstrumentError::UnsupportedLanguage(_)));
    }
}
// CODEGEN-END
