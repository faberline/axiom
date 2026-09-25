// HANDWRITE-BEGIN gap="missing-generator:unit-test:89dfd718" tracker="1887" reason="Lock Sift to the canonical shared-library package, crate, and path identities."
#[test]
fn manifest_uses_canonical_shared_library_names_without_aliases() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", manifest_path.display()));

    for expected in [
        r#"service-k8s = { git = "https://github.com/faberline/core", tag = ""#,
        r#"storage-durable = { git = "https://github.com/faberline/core", tag = ""#,
        r#"metrics-prometheus = { git = "https://github.com/faberline/core", tag = ""#,
        r#"raft-runtime = { git = "https://github.com/faberline/core", tag = ""#,
        r#"index-text = { git = "https://github.com/faberline/core", tag = ""#,
        r#"metrics-remote-write = { git = "https://github.com/faberline/core", tag = ""#,
        r#"service-collector = { git = "https://github.com/faberline/core", tag = ""#,
        r#"service-mcp = { git = "https://github.com/faberline/core", tag = ""#,
        r#"service-projection = { git = "https://github.com/faberline/core", tag = ""#,
        r#"storage-object = { git = "https://github.com/faberline/core", tag = ""#,
        r#"storage-segment = { git = "https://github.com/faberline/core", tag = ""#,
        r#"transport-otlp = { git = "https://github.com/faberline/core", tag = ""#,
    ] {
        assert!(
            manifest.contains(expected),
            "missing dependency alias: {expected}"
        );
    }

    for retired_alias in [
        "axiom-operator =",
        "service-durability =",
        "service-metrics =",
        "raft-host =",
    ] {
        assert!(
            !manifest.contains(retired_alias),
            "retired dependency alias remains: {retired_alias}"
        );
    }

    assert!(
        !manifest.contains("../../libs/"),
        "shared libraries are faberline/core git dependencies, not in-tree paths"
    );
}
// HANDWRITE-END
