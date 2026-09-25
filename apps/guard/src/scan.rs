// SPEC-MANAGED: apps/guard/tech-design/src/scan.py
// HANDWRITE-BEGIN gap="python-td-rust-body" tracker="#2823" reason="Guard scan domain behavior remains native Rust"
// @spec WI #2931: executable Python TD parity baseline.
use std::fs;
use std::path::Path;

use serde::Deserialize;

use compass::checker::{check_paths, LintConfig};
use compass::diagnostic::DiagnosticSeverity;
use compass::lint::detect_sql_injection;
use compass::syntax::Language;

use crate::evidence::{run_evidence_commands, EvidenceCommand};
use crate::policy::{include_diagnostic, map_severity, PolicyProfile};
use crate::report::{finding_id, Finding, GuardReport, Location};

#[derive(Debug, Clone)]
/// @spec apps/guard/tech-design/src/scan.py
pub struct ScanOptions {
    pub profile: PolicyProfile,
    pub languages: Vec<Language>,
    pub exclude_patterns: Vec<String>,
    pub evidence_commands: Vec<EvidenceCommand>,
}

/// @spec apps/guard/tech-design/src/scan.py
impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            profile: PolicyProfile::BaselineStatic,
            languages: default_languages(),
            exclude_patterns: vec![
                "__pycache__".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                ".venv".to_string(),
                ".guard".to_string(),
            ],
            evidence_commands: Vec::new(),
        }
    }
}

/// @spec apps/guard/tech-design/src/scan.py
pub fn default_languages() -> Vec<Language> {
    vec![
        Language::Python,
        Language::TypeScript,
        Language::Rust,
        Language::JavaScript,
        Language::Go,
        Language::Html,
        Language::Css,
        Language::Dockerfile,
        Language::Hcl,
        Language::Yaml,
        Language::Toml,
        Language::Sql,
        Language::GraphQL,
    ]
}

// A reviewed-and-accepted finding waiver, loaded from
// `<target>/.guard/waivers.json`. Opt-in: with no waiver file present, scanning
// behaves exactly as before. A waiver never invents a pass — it only suppresses
// a finding whose rule (and optional path) a maintainer has documented.
#[derive(Debug, Clone, Deserialize)]
/// @spec apps/guard/tech-design/src/scan.py
pub struct Waiver {
    pub rule: String,
    #[serde(default)]
    pub path_contains: Option<String>,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
/// @spec apps/guard/tech-design/src/scan.py
struct WaiverFile {
    #[serde(default)]
    waivers: Vec<Waiver>,
}

// Load `<dir>/.guard/waivers.json`. Fail-safe: a missing file or any parse error
// yields no waivers, so a broken waiver file keeps the scan strict rather than
// silently passing.
/// @spec apps/guard/tech-design/src/scan.py
fn load_waivers(target: &Path) -> Vec<Waiver> {
    let dir = if target.is_dir() {
        target.to_path_buf()
    } else {
        match target.parent() {
            Some(parent) => parent.to_path_buf(),
            None => return Vec::new(),
        }
    };
    let path = dir.join(".guard").join("waivers.json");
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<WaiverFile>(&text)
        .map(|file| file.waivers)
        .unwrap_or_default()
}

// `true` when `finding` matches a waiver: identical rule code, and — when the
// waiver pins `path_contains` — the finding path contains that substring.
/// @spec apps/guard/tech-design/src/scan.py
fn finding_is_waived(finding: &Finding, waivers: &[Waiver]) -> bool {
    waivers.iter().any(|waiver| {
        waiver.rule == finding.rule
            && waiver
                .path_contains
                .as_deref()
                .map(|needle| finding.location.path.contains(needle))
                .unwrap_or(true)
    })
}

/// @spec apps/guard/tech-design/src/scan.py
pub fn scan_path(path: impl AsRef<Path>) -> GuardReport {
    scan_path_with_options(path, ScanOptions::default())
}

/// @spec apps/guard/tech-design/src/scan.py
pub fn scan_path_with_options(path: impl AsRef<Path>, options: ScanOptions) -> GuardReport {
    let target = path.as_ref();
    let target_display = target.display().to_string();
    if !target.exists() {
        return GuardReport::tool_error("scan", target_display, 5, "scan target does not exist");
    }

    let config = LintConfig {
        languages: options.languages,
        exclude_patterns: options.exclude_patterns,
        min_severity: DiagnosticSeverity::Hint,
    };
    let paths = [target];
    let results = check_paths(&paths, &config);
    let diagnostics_scanned = results.iter().map(|r| r.diagnostics.len()).sum();
    let mut findings = Vec::new();

    for result in &results {
        let path = result.path.display().to_string();
        let mut diagnostics = result.diagnostics.clone();
        if let Some(language) = sql_injection_language(result.language) {
            if let Ok(source) = std::fs::read_to_string(&result.path) {
                diagnostics.extend(detect_sql_injection(&source, language));
            }
        }
        for diagnostic in &diagnostics {
            if !include_diagnostic(options.profile, diagnostic.category, &diagnostic.code) {
                continue;
            }
            let start_line = one_based(diagnostic.range.start.line);
            let end_line = one_based(diagnostic.range.end.line);
            let severity = map_severity(options.profile, diagnostic.severity);
            findings.push(Finding {
                id: finding_id(&diagnostic.code, &path, start_line),
                severity,
                rule: diagnostic.code.clone(),
                title: diagnostic.message.clone(),
                detail: format!(
                    "compass reported {} {} diagnostic {} at {}:{}",
                    diagnostic.severity.as_str(),
                    diagnostic.category.as_str(),
                    diagnostic.code,
                    path,
                    start_line
                ),
                remediation: remediation_for_rule(&diagnostic.code).to_string(),
                location: Location {
                    path: path.clone(),
                    start_line,
                    start_col: one_based(diagnostic.range.start.character),
                    end_line,
                    end_col: one_based(diagnostic.range.end.character),
                },
                evidence: serde_json::json!({
                    "source": "compass",
                    "compass_category": diagnostic.category.as_str(),
                    "compass_severity": diagnostic.severity.as_str(),
                    "language": result.language.as_str(),
                }),
            });
        }
    }

    let waivers = load_waivers(target);
    if !waivers.is_empty() {
        findings.retain(|finding| !finding_is_waived(finding, &waivers));
    }

    let evidence = run_evidence_commands(&options.evidence_commands);
    for item in &evidence {
        if let Some(finding) = item.to_guard_finding(&target_display) {
            findings.push(finding);
        }
    }

    GuardReport::from_scan_with_evidence(
        target_display,
        options.profile.as_str(),
        results.len(),
        diagnostics_scanned,
        findings,
        evidence,
    )
}

fn one_based(value: u32) -> u32 {
    value.saturating_add(1)
}

fn sql_injection_language(language: Language) -> Option<&'static str> {
    match language {
        Language::Python => Some("python"),
        Language::JavaScript => Some("javascript"),
        Language::TypeScript => Some("typescript"),
        Language::Go => Some("go"),
        _ => None,
    }
}

fn remediation_for_rule(rule: &str) -> &'static str {
    match rule {
        "JS004" | "JS006" | "PY301" | "PY302" => {
            "Remove dynamic code execution or replace it with a constrained parser/dispatcher."
        }
        "PY303" => "Avoid pickle/cPickle for untrusted data; use a safe serialization format.",
        "PY304" => "Avoid shell=True; pass an argv array and validate all untrusted inputs.",
        "PY305" => "Move secrets into environment variables or a secrets manager.",
        "RS201" => {
            "Document and audit the unsafe invariant; add a focused safety test when possible."
        }
        "SQL-INJ" => "Use parameterized queries and keep untrusted values out of SQL strings.",
        "DK002" => "Pin the image by version or digest to avoid supply-chain drift.",
        "JS007" => {
            "Avoid prototype mutation surfaces; use Object.getPrototypeOf or safe object creation."
        }
        "JS008" => "Remove `with`; it creates dynamic scope that is hard to audit.",
        _ => "Inspect the source, remove the risky pattern, or add a documented exception policy.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_javascript_eval_as_security_finding() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("unsafe.js");
        std::fs::write(&file, "eval('alert(1)');\n").expect("write fixture");

        let report = scan_path(dir.path());

        assert_eq!(report.summary.security_findings, 1);
        assert_eq!(report.findings[0].rule, "JS004");
        assert_eq!(report.exit_code, 1);
    }

    #[test]
    fn clean_javascript_file_is_clean() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("safe.js");
        std::fs::write(&file, "const answer = 42;\n").expect("write fixture");

        let report = scan_path(dir.path());

        assert_eq!(report.summary.security_findings, 0);
        assert_eq!(report.exit_code, 0);
    }

    #[test]
    fn security_lint_profile_upgrades_supply_chain_lint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("Dockerfile");
        std::fs::write(&file, "FROM ubuntu\n").expect("write fixture");

        let baseline = scan_path(dir.path());
        assert_eq!(baseline.summary.security_findings, 0);

        let mut options = ScanOptions::default();
        options.profile = PolicyProfile::SecurityLint;
        let report = scan_path_with_options(dir.path(), options);

        assert_eq!(report.summary.security_findings, 1);
        assert_eq!(report.findings[0].rule, "DK002");
        assert_eq!(report.policy_profile, "guard-security-lint/1");
    }

    #[test]
    fn sql_injection_helper_is_active_in_guard_scan() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("query.py");
        std::fs::write(
            &file,
            "uid = 1\nq = f\"SELECT * FROM users WHERE id = {uid}\"\n",
        )
        .expect("write fixture");

        let report = scan_path(dir.path());

        assert!(report
            .findings
            .iter()
            .any(|finding| finding.rule == "SQL-INJ"));
    }

    #[test]
    fn external_evidence_failure_becomes_guard_finding() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("safe.js");
        std::fs::write(&file, "const answer = 42;\n").expect("write fixture");

        let mut options = ScanOptions::default();
        options.evidence_commands.push(EvidenceCommand::shell(
            "rig",
            "exploit-smoke",
            "printf '%s' '{\"schema_version\":\"rig.report/1\",\"clean\":false,\"summary\":{\"total\":1},\"findings\":[{\"id\":\"exploit\"}]}'",
        ));
        let report = scan_path_with_options(dir.path(), options);

        assert_eq!(report.summary.evidence_count, 1);
        assert_eq!(report.summary.evidence_failed, 1);
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.rule == "RIG-EVIDENCE"));
    }

    #[test]
    fn waiver_file_suppresses_matching_finding() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("query.py"),
            "uid = 1\nq = f\"SELECT * FROM users WHERE id = {uid}\"\n",
        )
        .expect("write fixture");

        // Without a waiver the SQL-INJ finding is present and fails the scan.
        let strict = scan_path(dir.path());
        assert!(strict
            .findings
            .iter()
            .any(|finding| finding.rule == "SQL-INJ"));

        // A documented waiver for the rule clears it; nothing else is touched.
        std::fs::create_dir_all(dir.path().join(".guard")).expect("mkdir .guard");
        std::fs::write(
            dir.path().join(".guard").join("waivers.json"),
            r#"{"waivers":[{"rule":"SQL-INJ","reason":"benchmark-only query, not production"}]}"#,
        )
        .expect("write waivers");
        let waived = scan_path(dir.path());
        assert!(!waived
            .findings
            .iter()
            .any(|finding| finding.rule == "SQL-INJ"));
    }
}
// HANDWRITE-END
