// SPEC-MANAGED: apps/defer/external-contracts/behavior/2215.md#defer-security-shared-redacted-audit
// CODEGEN-BEGIN
// AW-EC-BEGIN
// @ec defer-security-shared-redacted-audit
// @capability security-hardening
// @claim delayed-task-security-boundary
// @contract shared-credential-reload-and-credential-free-audit
// @category security
// @required_for_production false
// @command cargo test --locked --manifest-path "$(bash scripts/faberline-core-test.sh --root)/Cargo.toml" --target-dir "${CARGO_TARGET_DIR:-target}" -p service-auth -- --nocapture
// AW-EC-END

// Contract: The shared mechanism used by Defer executes valid rotation, invalid and unreadable replacement preservation, file-watcher adoption, and typed authorization audit cases with non-zero tests.
// Contract: Authentication, authorization, and reload events expose stable decision context and failure classes but their typed and serialized forms cannot contain bearer credential bytes.
#[test]
#[ignore = "AW EC gate: run via `aw health --verify-ec` or `cargo test -- --ignored`"]
fn defer_security_shared_redacted_audit() {
    let command = r#"cargo test --locked --manifest-path "$(bash scripts/faberline-core-test.sh --root)/Cargo.toml" --target-dir "${CARGO_TARGET_DIR:-target}" -p service-auth -- --nocapture"#;
    let id = "defer-security-shared-redacted-audit";
    let mut root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !root.join(".aw").is_dir() {
        assert!(
            root.pop(),
            "AW EC {id}: no .aw/ project root above {}",
            env!("CARGO_MANIFEST_DIR")
        );
    }
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(&root)
        .output()
        .unwrap_or_else(|e| panic!("AW EC {id}: failed to spawn `{command}`: {e}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success()
        && aw_ec_cargo_test_executed_count(command, &stdout, &stderr) == Some(0)
    {
        panic!("AW EC {id} FAILED: cargo test command passed but executed 0 tests: {command}\nstdout:\n{stdout}\nstderr:\n{stderr}");
    }
    assert!(
        output.status.success(),
        "AW EC {id} FAILED (exit {:?}): {command}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status.code()
    );
}

fn aw_ec_cargo_test_executed_count(command: &str, stdout: &str, stderr: &str) -> Option<usize> {
    if !command.contains("cargo test") {
        return None;
    }
    let mut total = 0usize;
    let mut saw_count = false;
    for line in stdout.lines().chain(stderr.lines()) {
        let Some(count) = aw_ec_parse_cargo_running_test_count(line) else {
            continue;
        };
        total = total.saturating_add(count);
        saw_count = true;
    }
    saw_count.then_some(total)
}

fn aw_ec_parse_cargo_running_test_count(line: &str) -> Option<usize> {
    let rest = line.trim().strip_prefix("running ")?;
    let number = rest
        .strip_suffix(" tests")
        .or_else(|| rest.strip_suffix(" test"))?;
    number.trim().parse().ok()
}
// CODEGEN-END
