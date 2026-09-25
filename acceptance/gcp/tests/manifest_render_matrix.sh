#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ACCEPTANCE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$ACCEPTANCE_ROOT/../.." && pwd)"
RENDER_SCRIPT="$ACCEPTANCE_ROOT/scripts/render-manifests.sh"

fail() {
  echo "manifest render matrix: $1" >&2
  exit 1
}

lumen_src="$REPO_ROOT/target/faberline/lumen"
bash "$ACCEPTANCE_ROOT/scripts/faberline-source.sh" lumen "$lumen_src" >/dev/null
cargo build --locked --manifest-path "$lumen_src/Cargo.toml" --target-dir "$REPO_ROOT/target" \
  -p lumen --bin lumen --features "operator delegated-auth" || fail "failed to build lumen binary"
LUMEN_CLI="$REPO_ROOT/target/debug/lumen"

cargo build --locked --manifest-path "$REPO_ROOT/Cargo.toml" \
  -p sift --bin sift || fail "failed to build sift binary"
SIFT_CLI="$REPO_ROOT/target/debug/sift"

cargo build --locked --manifest-path "$REPO_ROOT/Cargo.toml" \
  -p tape --bin tape --features "operator backup" || fail "failed to build tape binary"
TAPE_CLI="$REPO_ROOT/target/debug/tape"

[[ -x "$LUMEN_CLI" ]] || fail "lumen binary is not executable at $LUMEN_CLI"
[[ -x "$SIFT_CLI" ]] || fail "sift binary is not executable at $SIFT_CLI"
[[ -x "$TAPE_CLI" ]] || fail "tape binary is not executable at $TAPE_CLI"

scratch_dir="$(mktemp -d "${TMPDIR:-/tmp}/manifest-render-matrix.XXXXXX")"
cleanup_scratch() {
  rm -rf "$scratch_dir"
}
trap cleanup_scratch EXIT INT TERM

check_app_subtree() {
  local mode="$1"
  local app="$2"
  local manifest_dir="$3"
  local expected="$4"

  local crd_file="$manifest_dir/$app/crd.yaml"
  local operator_file="$manifest_dir/$app/operator.bundle.yaml"
  local instance_file="$manifest_dir/$app/instance.bundle.yaml"

  [[ -f "$crd_file" ]] || fail "mode '$mode' app '$app' missing crd.yaml"
  [[ -f "$operator_file" ]] || fail "mode '$mode' app '$app' missing operator.bundle.yaml"
  [[ -f "$instance_file" ]] || fail "mode '$mode' app '$app' missing instance.bundle.yaml"

  local parsed
  parsed="$(awk '
    BEGIN {
      kind = ""; in_meta = 0; meta_indent = -1; name = ""; namespace = "";
    }
    function end_doc() {
      if (kind == "Namespace" && name != "") {
        print "CREATED " name;
      }
      if (namespace != "") {
        print "REF " namespace;
      }
      kind = ""; in_meta = 0; meta_indent = -1; name = ""; namespace = "";
    }
    /^---/ {
      end_doc();
      next;
    }
    /^[ \t]*kind:[ \t]*/ {
      val = $0;
      sub(/^[ \t]*kind:[ \t]*/, "", val);
      gsub(/["\047]/, "", val);
      sub(/[ \t]*#.*/, "", val); sub(/[ \t]*$/, "", val);
      kind = val;
      next;
    }
    /^[ \t]*metadata:[ \t]*/ {
      in_meta = 1;
      match($0, /^[ \t]*/);
      meta_indent = RLENGTH;
      next;
    }
    in_meta {
      match($0, /^[ \t]*/);
      indent = RLENGTH;
      if (indent <= meta_indent && NF > 0) {
        in_meta = 0;
      } else {
        if ($0 ~ /^[ \t]*name:[ \t]*/) {
          val = $0;
          sub(/^[ \t]*name:[ \t]*/, "", val);
          gsub(/["\047]/, "", val);
          sub(/[ \t]*#.*/, "", val); sub(/[ \t]*$/, "", val);
          name = val;
        }
        if ($0 ~ /^[ \t]*namespace:[ \t]*/) {
          val = $0;
          sub(/^[ \t]*namespace:[ \t]*/, "", val);
          gsub(/["\047]/, "", val);
          sub(/[ \t]*#.*/, "", val); sub(/[ \t]*$/, "", val);
          namespace = val;
        }
      }
    }
    END {
      end_doc();
    }
  ' "$crd_file" "$operator_file" "$instance_file")"

  local created_namespaces
  created_namespaces="$(echo "$parsed" | grep '^CREATED ' | cut -d' ' -f2 | sort -u | tr '\n' ' ' | sed 's/ $//')"

  if [[ "$created_namespaces" != "$expected" ]]; then
    fail "mode '$mode' app '$app' created namespaces '$created_namespaces' does not match expected '$expected'"
  fi

  local ref_namespaces
  ref_namespaces="$(echo "$parsed" | grep '^REF ' | cut -d' ' -f2 | sort -u)"

  while IFS= read -r ref_ns; do
    [[ -z "$ref_ns" ]] && continue
    if ! echo " $created_namespaces " | grep -q " $ref_ns "; then
      fail "mode '$mode' app '$app' places object in namespace '$ref_ns' which is not created by any document in its applied manifests"
    fi
  done <<<"$ref_namespaces"
}

# --- Mode 1: lumen sift ---
(
  export ACCEPTANCE_APPS="lumen sift"
  export RUN_ID="matrix-test"
  export MANIFEST_DIR="$scratch_dir/lumen-sift/manifests"
  export GKE_CLUSTER_NAME="matrix-cluster"
  export GKE_ZONE="asia-east1-a"
  export PROJECT_ID="matrix-project"
  export LUMEN_CLI="$LUMEN_CLI"
  export SIFT_CLI="$SIFT_CLI"
  export LUMEN_IMAGE="example-registry/lumen@sha256:0000000000000000000000000000000000000000000000000000000000000000"
  export SIFT_IMAGE="example-registry/sift@sha256:0000000000000000000000000000000000000000000000000000000000000000"
  export BACKUP_BUCKET="matrix-backup-bucket"
  export BACKUP_GSA_EMAIL="matrix-backup@example.com"
  "$RENDER_SCRIPT"
) || fail "rendering failed for mode 'lumen sift'"

check_app_subtree "lumen sift" "lumen" "$scratch_dir/lumen-sift/manifests" "lumen lumen-system"
check_app_subtree "lumen sift" "sift" "$scratch_dir/lumen-sift/manifests" "sift sift-system"

# --- Mode 2: lumen auth ---
(
  unset BACKUP_BUCKET BACKUP_GSA_EMAIL
  export ACCEPTANCE_APPS="lumen auth"
  export RUN_ID="matrix-test"
  export MANIFEST_DIR="$scratch_dir/lumen-auth/manifests"
  export GKE_CLUSTER_NAME="matrix-cluster"
  export GKE_ZONE="asia-east1-a"
  export PROJECT_ID="matrix-project"
  export LUMEN_CLI="$LUMEN_CLI"
  export LUMEN_IMAGE="example-registry/lumen@sha256:0000000000000000000000000000000000000000000000000000000000000000"
  "$RENDER_SCRIPT"
) || fail "rendering failed for mode 'lumen auth'"

check_app_subtree "lumen auth" "lumen" "$scratch_dir/lumen-auth/manifests" "lumen lumen-system"

# Assert no lumen-backup ServiceAccount in lumen auth mode bundles
if grep -q 'lumen-backup' "$scratch_dir/lumen-auth/manifests/lumen/instance.bundle.yaml" "$scratch_dir/lumen-auth/manifests/lumen/operator.bundle.yaml"; then
  fail "mode 'lumen auth' unexpectedly rendered lumen-backup ServiceAccount"
fi

# --- Mode 3: standalone Sift MVP ---
sift_numeric_run_id="0903171714"
(
  export ACCEPTANCE_APPS="sift"
  export RUN_ID="$sift_numeric_run_id"
  export MANIFEST_DIR="$scratch_dir/sift/manifests"
  export GKE_CLUSTER_NAME="matrix-cluster"
  export GKE_ZONE="asia-east1-a"
  export PROJECT_ID="matrix-project"
  export SIFT_CLI="$SIFT_CLI"
  export SIFT_IMAGE="example-registry/sift@sha256:0000000000000000000000000000000000000000000000000000000000000000"
  export RIG_IMAGE="example-registry/rig@sha256:1111111111111111111111111111111111111111111111111111111111111111"
  export BACKUP_BUCKET="matrix-backup-bucket"
  export BACKUP_GSA_EMAIL="matrix-backup@example.com"
  "$RENDER_SCRIPT"
) || fail "rendering failed for mode 'sift'"

check_app_subtree "sift" "sift" "$scratch_dir/sift/manifests" "sift sift-system"

sift_cr_json="$(
  kubectl patch --local --type=merge \
    -f "$scratch_dir/sift/manifests/sift/instance.bundle.yaml" \
    -p '{}' -o json \
    | jq -c 'select(.kind == "Sift")'
)" || fail "could not parse the standalone Sift manifest as Kubernetes JSON"
jq -e --arg run_id "$sift_numeric_run_id" '
  .spec.placement.nodeSelector["axiom-run-id"] == $run_id
  and (.spec.placement.nodeSelector["axiom-run-id"] | type) == "string"
' <<<"$sift_cr_json" >/dev/null \
  || fail "standalone Sift coerced a numeric-looking run ID into a YAML number"

grep -q 'storeSize: 50Gi' "$scratch_dir/sift/manifests/sift/instance.bundle.yaml" \
  || fail "standalone Sift did not render its 50Gi store volume"
grep -q 'peerTlsSecret: sift-peer-tls' "$scratch_dir/sift/manifests/sift/instance.bundle.yaml" \
  || fail "standalone Sift did not bind the generated peer TLS Secret"
if grep -q 'REPLACE_ME__SIFT_PEER_TLS_SECRET' "$scratch_dir/sift/manifests/sift/instance.bundle.yaml"; then
  fail "standalone Sift left the peer TLS placeholder in the applied bundle"
fi
grep -q 'name: sift-rig' "$scratch_dir/sift/manifests/sift/instance.bundle.yaml" \
  || fail "standalone Sift did not render its isolated Rig service account"
if grep -q 'system:auth-delegator' "$scratch_dir/sift/manifests/sift/instance.bundle.yaml"; then
  fail "standalone Sift masks operator-managed auth delegation with a static ClusterRoleBinding"
fi

# --- Mode 4: tape ---
(
  export ACCEPTANCE_APPS="tape"
  export RUN_ID="matrix-test"
  export MANIFEST_DIR="$scratch_dir/tape/manifests"
  export GKE_CLUSTER_NAME="matrix-cluster"
  export GKE_ZONE="asia-east1-a"
  export PROJECT_ID="matrix-project"
  export TAPE_CLI="$TAPE_CLI"
  export TAPE_IMAGE="example-registry/tape@sha256:0000000000000000000000000000000000000000000000000000000000000000"
  export BACKUP_BUCKET="matrix-backup-bucket"
  export BACKUP_GSA_EMAIL="matrix-backup@example.com"
  "$RENDER_SCRIPT"
) || fail "rendering failed for mode 'tape'"

check_app_subtree "tape" "tape" "$scratch_dir/tape/manifests" "tape tape-system"

echo "manifest render matrix: ok"
