// HANDWRITE-BEGIN gap="missing-generator:unit-test:vat-service-observability-e2e" tracker="pending-tracker" reason="Prove Lumen, Tape, Relay, and Defer compose with VAT capture and the Sift-owned collector through the shared stdout contract."
#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const SERVICES: [&str; 4] = ["lumen", "tape", "relay", "defer"];

fn debug_binary(name: &str) -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_sift"))
        .parent()
        .expect("Sift debug binary directory")
        .join(name)
}

fn require_binary(path: &Path) {
    assert!(
        path.is_file(),
        "missing current-workspace binary {}; run `cargo build -p vat -p lumen -p tape -p relay -p defer -p sift --bins`",
        path.display()
    );
}

fn jsonl(stdout: &[u8]) -> Vec<Value> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("VAT stdout JSONL"))
        .collect()
}

#[test]
fn applications_remain_sift_agnostic() {
    for manifest in [
        include_str!("../../../apps/tape/Cargo.toml"),
        include_str!("../../../apps/relay/Cargo.toml"),
        include_str!("../../../apps/defer/Cargo.toml"),
    ] {
        assert!(
            !manifest.lines().any(|line| {
                let line = line.trim_start();
                line.starts_with("sift =") || line.starts_with("sift=")
            }),
            "applications must emit standard telemetry without linking Sift"
        );
    }
}

#[test]
fn vat_managed_service_stdout_reaches_real_sift_query() {
    if std::env::var_os("VAT_SERVICE_OBSERVABILITY_PROBE").is_some() {
        return;
    }

    let vat = debug_binary("vat");
    let lumen = debug_binary("lumen");
    let tape = debug_binary("tape");
    let relay = debug_binary("relay");
    let defer = debug_binary("defer");
    let sift = debug_binary("sift");
    for binary in [&vat, &lumen, &tape, &relay, &defer, &sift] {
        require_binary(binary);
    }
    let probe = std::env::current_exe().expect("current E2E test executable");

    let project = tempfile::tempdir().unwrap();
    let vat_home = tempfile::tempdir().unwrap();
    let config = format!(
        r#"
version = 1
name = "service-sift-observability"

[workspace]
base = "."
workdir = "."
keep = "always"

[env]
LUMEN_AUTH = "off"
TAPE_AUTH = "off"
RELAY_AUTH = "off"
DEFER_AUTH = "off"
SIFT_AUTH = "off"
VAT_SERVICE_OBSERVABILITY_PROBE = "1"

[[services]]
id = "lumen"
cmd = [{lumen}, "serve", "--host", "127.0.0.1", "--port", "{{port}}", "--wal", "embedded", "--log-format", "json"]
ready_http = "http://127.0.0.1:{{port}}/readyz"
timeout_s = 30

[[services]]
id = "tape"
cmd = [{tape}, "serve", "--bind", "127.0.0.1:{{port}}", "--store", "tape.json", "--log-format", "json"]
ready_http = "http://127.0.0.1:{{port}}/readyz"
timeout_s = 30

[[services]]
id = "relay"
cmd = [{relay}, "--bind", "127.0.0.1:{{port}}", "--data-dir", "relay-data", "--log-format", "json"]
ready_http = "http://127.0.0.1:{{port}}/readyz"
timeout_s = 30

[[services]]
id = "defer"
cmd = [{defer}, "serve", "--bind", "127.0.0.1:{{port}}", "--data-dir", "defer-data", "--log-format", "json"]
ready_http = "http://127.0.0.1:{{port}}/readyz"
timeout_s = 30

[[services]]
id = "sift"
cmd = [{sift}, "serve", "--host", "127.0.0.1", "--port", "{{port}}", "--data-dir", "sift-data", "--log-level", "warn", "--log-format", "json"]
ready_http = "http://127.0.0.1:{{port}}/readyz"
export = {{ SIFT_URL = "http://{{host}}:{{port}}" }}
timeout_s = 30

[[runners]]
id = "observability"
requires = ["lumen", "tape", "relay", "defer", "sift"]
cmd = [{probe}, "--exact", "vat_runner_collects_every_service", "--nocapture"]
timeout_s = 60
artifacts = ["observability-proof.json"]
"#,
        lumen = serde_json::to_string(&lumen.to_string_lossy()).unwrap(),
        tape = serde_json::to_string(&tape.to_string_lossy()).unwrap(),
        relay = serde_json::to_string(&relay.to_string_lossy()).unwrap(),
        defer = serde_json::to_string(&defer.to_string_lossy()).unwrap(),
        sift = serde_json::to_string(&sift.to_string_lossy()).unwrap(),
        probe = serde_json::to_string(&probe.to_string_lossy()).unwrap(),
    );
    std::fs::write(project.path().join("vat.toml"), config).unwrap();

    let output = Command::new(&vat)
        .current_dir(project.path())
        .env("VAT_HOME", vat_home.path())
        .env_remove("RUST_LOG")
        .env_remove("LUMEN_OTLP_ENDPOINT")
        .env_remove("TAPE_OTLP_ENDPOINT")
        .env_remove("RELAY_OTLP_ENDPOINT")
        .env_remove("DEFER_OTLP_ENDPOINT")
        .env_remove("OTEL_EXPORTER_OTLP_ENDPOINT")
        .env_remove("SIFT_TOKEN")
        .args(["run", "observability"])
        .output()
        .expect("run real VAT service observability journey");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let events = jsonl(&output.stdout);
    let result = events
        .iter()
        .find(|event| event["type"] == "result")
        .expect("VAT result event");
    let vat_id = result["id"].as_str().expect("VAT id");
    if !output.status.success() {
        let logs = Command::new(&vat)
            .env("VAT_HOME", vat_home.path())
            .args(["logs", vat_id, "runner"])
            .output()
            .expect("read failed VAT runner logs");
        panic!(
            "VAT journey failed: stdout=\n{}\nstderr=\n{}\nrunner stdout=\n{}\nrunner stderr=\n{}",
            stdout,
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&logs.stdout),
            String::from_utf8_lossy(&logs.stderr)
        );
    }
    assert!(
        !stdout.contains("axiom.service.log.v1"),
        "VAT must not replay child telemetry into its own stdout: {stdout}"
    );
    assert_eq!(result["ok"], true);
    assert_eq!(result["state"], "kept");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vat_runner_collects_every_service() {
    if std::env::var_os("VAT_SERVICE_OBSERVABILITY_PROBE").is_none() {
        return;
    }

    let sift_url = std::env::var("SIFT_URL").expect("VAT Sift endpoint export");
    let logs_dir = PathBuf::from(std::env::var("VAT_LOGS_DIR").expect("VAT logs directory"));
    let mut sources = Vec::new();
    for service in SERVICES {
        let key = format!("VAT_SERVICE_{}_STDOUT_LOG", service.to_ascii_uppercase());
        let source = PathBuf::from(std::env::var(&key).unwrap_or_else(|_| panic!("missing {key}")));
        assert!(source.starts_with(&logs_dir));
        assert!(source.is_file());
        wait_for_structured_event(&source, service).await;
        sources.push((service, source));
    }

    let mut summaries = serde_json::Map::new();
    for (service, source) in &sources {
        let checkpoint = format!("{service}.collector.checkpoint.json");
        let quarantine = format!("{service}.collector.rejected.jsonl");
        let source_id = format!("vat:{service}:stdout");
        let collector = Command::new(env!("CARGO_BIN_EXE_sift"))
            .args([
                "collect",
                "--source",
                source.to_str().unwrap(),
                "--source-id",
                &source_id,
                "--endpoint",
                &sift_url,
                "--project",
                "local",
                "--environment",
                "test",
                "--checkpoint",
                &checkpoint,
                "--quarantine",
                &quarantine,
                "--max-retries",
                "2",
            ])
            .env_remove("SIFT_TOKEN")
            .output()
            .unwrap_or_else(|error| panic!("run Sift collector for {service}: {error}"));
        assert!(
            collector.status.success(),
            "collector for {service} failed: stdout={} stderr={}",
            String::from_utf8_lossy(&collector.stdout),
            String::from_utf8_lossy(&collector.stderr)
        );
        let summary: Value = serde_json::from_slice(&collector.stdout).unwrap();
        assert!(
            summary["accepted"].as_u64().unwrap_or(0) >= 1,
            "{summary:#}"
        );
        assert_eq!(summary["rejected"], 0, "{service}: {summary:#}");
        summaries.insert((*service).to_string(), summary);
    }

    let client = reqwest::Client::builder()
        .http1_only()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let records = wait_for_all_services(&client, &sift_url).await;
    let proof = json!({
        "schema": "sift.local-service-observability-proof.v1",
        "services": SERVICES,
        "collector": summaries,
        "records": records,
    });
    std::fs::write(
        "observability-proof.json",
        serde_json::to_vec_pretty(&proof).unwrap(),
    )
    .unwrap();
}

async fn wait_for_structured_event(path: &Path, service: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let source = std::fs::read_to_string(path).unwrap_or_default();
        if let Some(event) = source
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .find(|event| {
                event["schema"] == "axiom.service.log.v1" && event["service"]["name"] == service
            })
        {
            return event;
        }
        assert!(
            Instant::now() < deadline,
            "missing structured {service} event in {}: {source}",
            path.display()
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_for_all_services(
    client: &reqwest::Client,
    sift_url: &str,
) -> serde_json::Map<String, Value> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let response = client
            .post(format!("{sift_url}/api/v1/query"))
            .json(&json!({
                "version": 1,
                "project": "local",
                "environment": "test",
                "signal": {"kind": "logs"},
                "limit": 100
            }))
            .send()
            .await
            .expect("query real Sift logging API");
        assert!(
            response.status().is_success(),
            "Sift query failed: {response:?}"
        );
        let page: Value = response.json().await.unwrap();
        let mut found = serde_json::Map::new();
        if let Some(records) = page["data"]["records"].as_array() {
            for service in SERVICES {
                if let Some(record) = records
                    .iter()
                    .find(|record| record["resource"]["service.name"] == service)
                {
                    assert!(record["json_payload"].is_object());
                    assert!(record["body_text"]
                        .as_str()
                        .is_some_and(|body| !body.is_empty()));
                    found.insert(service.to_string(), record.clone());
                }
            }
        }
        if found.len() == SERVICES.len() {
            return found;
        }
        assert!(
            Instant::now() < deadline,
            "Sift query did not contain every service: {page:#}"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
// HANDWRITE-END
