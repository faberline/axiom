#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ACCEPTANCE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

for script in "$SCRIPT_DIR"/*.sh "$ACCEPTANCE_ROOT"/tests/*.sh; do
  bash -n "$script"
  [[ -x "$script" ]] || {
    echo "acceptance script is not executable: $script" >&2
    exit 1
  }
done
process_token_helper="$SCRIPT_DIR/process-start-token.py"
[[ -x "$process_token_helper" ]] || {
  echo "process start-token helper is not executable: $process_token_helper" >&2
  exit 1
}
process_token="$($process_token_helper "$$")"
[[ "$process_token" =~ ^darwin:[0-9]+:[0-9]+$ \
  || "$process_token" =~ ^linux:[0-9a-f-]+:[0-9]+$ ]] || {
  echo "process start-token helper returned an invalid token" >&2
  exit 1
}
run_log_sink_helper="$SCRIPT_DIR/run-log-sink.py"
[[ -f "$run_log_sink_helper" && ! -L "$run_log_sink_helper" ]] || {
  echo "run-log sink helper must be a regular file" >&2
  exit 1
}
python3 -c 'import ast,pathlib,sys; ast.parse(pathlib.Path(sys.argv[1]).read_text())' \
  "$run_log_sink_helper"
run_supervisor="$SCRIPT_DIR/run-supervisor.py"
[[ -f "$run_supervisor" && ! -L "$run_supervisor" ]] || {
  echo "run supervisor must be a regular file" >&2
  exit 1
}
python3 -c 'import ast,pathlib,sys; ast.parse(pathlib.Path(sys.argv[1]).read_text())' \
  "$run_supervisor"
jq empty "$ACCEPTANCE_ROOT/evidence/schema.json"
python3 "$SCRIPT_DIR/validate-sift-mvp-evidence.py" \
  --schema "$ACCEPTANCE_ROOT/evidence/schema.json" --schema-only

# The Lumen CRD defaults `spec.auth` to `required`, and a lumen built without
# `delegated-auth` refuses to start in that mode rather than serve
# unauthenticated. That is the right call, but it means an acceptance image
# missing the feature turns every auth-mode instance into a crash loop whose
# only symptom is a pod that never goes Ready -- twenty minutes and a cloud
# build away from the one line that explains it. Check the feature here, where
# it costs nothing.
#
# Every Dockerfile that builds the lumen binary, not a named one: there are two
# build paths (both images together, or one at a time when the other was
# supplied by digest), and the first version of this check named only the path
# that was not being taken. A grep for the build line finds whichever file
# grows next.
lumen_build_files="$(grep -rl -- '-p lumen --bin lumen' "$ACCEPTANCE_ROOT/images")"
[[ -n "$lumen_build_files" ]] || {
  echo "no acceptance image builds the lumen binary; this check is now looking \
in the wrong place" >&2
  exit 1
}
while IFS= read -r dockerfile; do
  grep -q -- '--features "[^"]*delegated-auth' "$dockerfile" || {
    echo "$dockerfile must build lumen with the delegated-auth feature; \
spec.auth defaults to required and the binary refuses to start without it" >&2
    exit 1
  }
done <<<"$lumen_build_files"
for terraform_dir in environment cluster; do
  terraform -chdir="$ACCEPTANCE_ROOT/$terraform_dir" fmt -check -recursive
done
bash "$ACCEPTANCE_ROOT/tests/acceptance_mode_selection.sh"
bash "$ACCEPTANCE_ROOT/tests/manifest_render_matrix.sh"
bash "$ACCEPTANCE_ROOT/tests/sift_mvp_evidence_finalization.sh"
bash "$ACCEPTANCE_ROOT/tests/acceptance_lock_scope.sh"
bash "$ACCEPTANCE_ROOT/tests/acceptance_lock_writer_failures.sh"
bash "$ACCEPTANCE_ROOT/tests/acceptance_atomic_run_claim.sh"
bash "$ACCEPTANCE_ROOT/tests/acceptance_run_handoff.sh"
bash "$ACCEPTANCE_ROOT/tests/kubernetes_ownership.sh"
bash "$ACCEPTANCE_ROOT/tests/process_tree_fail_closed.sh"
bash "$ACCEPTANCE_ROOT/tests/sift_candidate_receipt.sh"
bash "$ACCEPTANCE_ROOT/tests/sift_candidate_preflight.sh"
bash "$ACCEPTANCE_ROOT/tests/sift_candidate_cleanup.sh"
bash "$ACCEPTANCE_ROOT/tests/sift_container_boundary.sh"
bash "$ACCEPTANCE_ROOT/tests/sift_prometheus_range_oracle.sh"
bash "$ACCEPTANCE_ROOT/tests/sift_node_pool_cleanup_bound.sh"
bash "$ACCEPTANCE_ROOT/tests/sift_evidence_secret_cleanup.sh"
bash "$ACCEPTANCE_ROOT/tests/run_supervisor_deadline.sh"
bash "$ACCEPTANCE_ROOT/tests/run_log_cleanup.sh"
bash "$ACCEPTANCE_ROOT/tests/cleanup_session_fence.sh"
bash "$ACCEPTANCE_ROOT/tests/cleanup_recovers_session_receipt_gap.sh"
bash "$ACCEPTANCE_ROOT/tests/cleanup_releases_lock_after_early_failure.sh"
bash "$ACCEPTANCE_ROOT/tests/cleanup_continues_after_cloud_build_failure.sh"
bash "$ACCEPTANCE_ROOT/tests/cleanup_retains_artifacts_after_partial_build_failure.sh"
bash "$ACCEPTANCE_ROOT/tests/cleanup_rejects_failed_image_inventory.sh"
bash "$ACCEPTANCE_ROOT/tests/cleanup_sift_candidate_binding.sh"
bash "$ACCEPTANCE_ROOT/tests/cleanup_removes_owned_sift_binding_before_finalizer.sh"
bash "$ACCEPTANCE_ROOT/tests/source_prefix_scope.sh"

# Validate in a disposable copy so provider initialization never writes a
# lock/cache artifact into the source tree.
validate_root="$(mktemp -d "${TMPDIR:-/tmp}/axiom-gcp-operator-validate.XXXXXX")"
cleanup_validate() {
  find "$validate_root" -type f -delete
  find "$validate_root" -depth -type d -empty -delete
}
trap cleanup_validate EXIT INT TERM
for terraform_dir in environment cluster; do
  mkdir -p "$validate_root/$terraform_dir"
  cp "$ACCEPTANCE_ROOT/$terraform_dir"/*.tf "$validate_root/$terraform_dir/"
  TF_DATA_DIR="$validate_root/.terraform-$terraform_dir" terraform \
    -chdir="$validate_root/$terraform_dir" init -backend=false -input=false
  TF_DATA_DIR="$validate_root/.terraform-$terraform_dir" terraform \
    -chdir="$validate_root/$terraform_dir" validate
done

# A gate that says nothing when it passes is indistinguishable from one that
# did not run -- and this gate was silently red for several commits before
# anyone noticed. Say so out loud, and name what comes next.
echo "static acceptance gate: ok"
echo "next: PROJECT_ID=<project> acceptance/gcp/scripts/run.sh"
echo "Sift next: prepare-sift-candidate.sh, then run.sh with SIFT_CANDIDATE_DIR"
