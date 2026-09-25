//! Dependency and explicit-test contracts for Sift and the faberline/core
//! base libraries it composes.

use std::{collections::BTreeSet, path::Path, process::Command};

const SHARED_PACKAGES: &[&str] = &[
    "index-text",
    "metrics-remote-write",
    "service-collector",
    "service-mcp",
    "service-projection",
    "storage-object",
    "storage-segment",
    "transport-otlp",
];

fn metadata() -> serde_json::Value {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(manifest)
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("decode cargo metadata")
}

#[test]
fn production_graph_points_from_sift_to_libs_only() {
    let metadata = metadata();
    let packages = metadata["packages"].as_array().unwrap();
    let sift = packages
        .iter()
        .find(|package| package["name"] == "sift")
        .expect("Sift package");
    let production = sift["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|dependency| dependency["kind"].as_str() != Some("dev"))
        .collect::<Vec<_>>();

    for dependency in &production {
        if let Some(path) = dependency["path"].as_str() {
            assert!(
                !path.contains("/apps/"),
                "Sift production dependency points at an app: {path}"
            );
        }
    }
    for expected in SHARED_PACKAGES {
        assert!(
            production
                .iter()
                .any(|dependency| dependency["name"] == *expected),
            "Sift does not compose shared package {expected}"
        );
    }

    // The shared packages are faberline/core git dependencies, so they cannot
    // reach back into apps/sift; their own graph is core's contract.
    for dependency in &production {
        if SHARED_PACKAGES.contains(&dependency["name"].as_str().unwrap_or_default()) {
            let source = dependency["source"].as_str().unwrap_or_default();
            assert!(
                source.starts_with("git+https://github.com/faberline/core"),
                "shared package {} is not a faberline/core git dependency: {source}",
                dependency["name"]
            );
        }
    }
}

#[test]
fn every_direct_e2e_source_is_an_explicit_cargo_target() {
    let metadata = metadata();
    // The shared packages' e2e targets are checked in faberline/core.
    let package = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "sift")
        .expect("Sift package");
    let manifest = Path::new(package["manifest_path"].as_str().unwrap());
    let e2e = manifest.parent().unwrap().join("e2e");
    let targets = package["targets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|target| {
            target["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind == "test"))
        })
        .filter_map(|target| target["src_path"].as_str())
        .map(|path| Path::new(path).to_path_buf())
        .collect::<BTreeSet<_>>();
    for entry in
        std::fs::read_dir(&e2e).unwrap_or_else(|error| panic!("read {}: {error}", e2e.display()))
    {
        let path = entry.unwrap().path();
        if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            assert!(
                targets.contains(&path),
                "{} is not an explicit Cargo test target",
                path.display()
            );
        }
    }
}
