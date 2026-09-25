// SPEC-MANAGED: apps/guard/tech-design/src/policy.py
// HANDWRITE-BEGIN gap="python-td-rust-body" tracker="#2866" reason="Guard policy domain behavior remains native Rust"
// @spec WI #2931: executable Python TD parity baseline.
use compass::diagnostic::{DiagnosticCategory, DiagnosticSeverity};

use crate::report::Severity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// @spec apps/guard/tech-design/src/policy.py
pub enum PolicyProfile {
    BaselineStatic,
    SecurityLint,
    Strict,
}

/// @spec apps/guard/tech-design/src/policy.py
impl PolicyProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            PolicyProfile::BaselineStatic => "guard-baseline-static/1",
            PolicyProfile::SecurityLint => "guard-security-lint/1",
            PolicyProfile::Strict => "guard-strict/1",
        }
    }
}

/// @spec apps/guard/tech-design/src/policy.py
pub(crate) fn include_diagnostic(
    profile: PolicyProfile,
    category: DiagnosticCategory,
    code: &str,
) -> bool {
    category == DiagnosticCategory::Security
        || matches!(profile, PolicyProfile::SecurityLint | PolicyProfile::Strict)
            && security_lint_rule(code)
}

fn security_lint_rule(code: &str) -> bool {
    matches!(code, "DK002" | "JS007" | "JS008" | "SQL-INJ" | "TS102")
}

/// @spec apps/guard/tech-design/src/policy.py
pub(crate) fn map_severity(profile: PolicyProfile, severity: DiagnosticSeverity) -> Severity {
    match (profile, severity) {
        (PolicyProfile::Strict, DiagnosticSeverity::Error)
        | (PolicyProfile::Strict, DiagnosticSeverity::Warning) => Severity::High,
        (PolicyProfile::Strict, DiagnosticSeverity::Information) => Severity::Medium,
        (PolicyProfile::Strict, DiagnosticSeverity::Hint) => Severity::Low,
        (_, severity) => match severity {
            DiagnosticSeverity::Error => Severity::High,
            DiagnosticSeverity::Warning => Severity::Medium,
            DiagnosticSeverity::Information => Severity::Low,
            DiagnosticSeverity::Hint => Severity::Info,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_excludes_security_lint_rules() {
        assert!(!include_diagnostic(
            PolicyProfile::BaselineStatic,
            DiagnosticCategory::Style,
            "DK002",
        ));
        assert!(include_diagnostic(
            PolicyProfile::SecurityLint,
            DiagnosticCategory::Style,
            "DK002",
        ));
    }

    #[test]
    fn security_diagnostics_are_included_by_every_profile() {
        for profile in [
            PolicyProfile::BaselineStatic,
            PolicyProfile::SecurityLint,
            PolicyProfile::Strict,
        ] {
            assert!(include_diagnostic(
                profile,
                DiagnosticCategory::Security,
                "JS004",
            ));
        }
    }

    #[test]
    fn strict_profile_promotes_hints_to_actionable_low_severity() {
        assert_eq!(
            map_severity(PolicyProfile::BaselineStatic, DiagnosticSeverity::Hint),
            Severity::Info,
        );
        assert_eq!(
            map_severity(PolicyProfile::Strict, DiagnosticSeverity::Hint),
            Severity::Low,
        );
        assert_eq!(
            map_severity(PolicyProfile::Strict, DiagnosticSeverity::Warning),
            Severity::High,
        );
    }
}
// HANDWRITE-END
