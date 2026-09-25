#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ACCEPTANCE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(cd "$ACCEPTANCE_ROOT/../.." && pwd)"
source "$SCRIPT_DIR/process-tree.sh"
source "$SCRIPT_DIR/run-log.sh"
source "$SCRIPT_DIR/source-prefix.sh"
source "$SCRIPT_DIR/acceptance-lock.sh"
source "$SCRIPT_DIR/sift-candidate.sh"
source "$SCRIPT_DIR/kubernetes-ownership.sh"
container_role="${AXIOM_GCP_ACCEPTANCE_CONTAINER_ROLE:-}"
if [[ "${ACCEPTANCE_APPS:-lumen sift}" == "sift" \
  && -z "$container_role" ]]; then
  exec "$SCRIPT_DIR/run-sift-contained.sh" "$@"
fi

# The contained child can fail before it reaches the full cleanup trap below,
# including while the supervisor or candidate receipt is validated. Publish a
# fail-closed handoff now. A successful `exec` does not run this EXIT trap; the
# next run.sh process installs the same guard until the full trap replaces it.
contained_bootstrap_exit() {
  local ec=$?
  local evidence_dir temporary finished_at
  trap - EXIT INT TERM
  [[ "$ec" != "0" ]] || ec=1
  evidence_dir="${EVIDENCE_DIR:-}"
  if [[ "${ACCEPTANCE_APPS:-lumen sift}" == "sift" \
      && "$container_role" == "run" \
      && "$evidence_dir" == /* && ! -L "$evidence_dir" ]]; then
    mkdir -p "$evidence_dir" >/dev/null 2>&1 || true
    temporary="$evidence_dir/.run-exit.bootstrap.$$"
    finished_at="$(date -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || true)"
    if [[ -n "$finished_at" ]] \
        && printf '%s\n' \
          "{\"schema\":\"axiom.gcp.sift.contained-run-exit.v1\",\"exit_code\":${ec},\"completed\":false,\"cleanup_armed\":false,\"local_claim_released\":true,\"finished_at\":\"${finished_at}\"}" \
          > "$temporary" \
        && chmod 0600 "$temporary" \
        && mv "$temporary" "$evidence_dir/run-exit.json"; then
      :
    else
      rm -f "$temporary" >/dev/null 2>&1 || true
    fi
  fi
  exit "$ec"
}
if [[ "${ACCEPTANCE_APPS:-lumen sift}" == "sift" \
  && "$container_role" == "run" ]]; then
  trap contained_bootstrap_exit EXIT
  trap 'exit 130' INT TERM
fi
if [[ "${AXIOM_GCP_ACCEPTANCE_ISOLATED_SESSION:-0}" != "1" ]]; then
  command -v python3 >/dev/null 2>&1 || {
    echo "python3 is required to isolate the acceptance process group" >&2
    exit 1
  }
  if [[ "${ACCEPTANCE_APPS:-lumen sift}" == "sift" ]]; then
    supervisor_default_seconds=5400
    supervisor_preflight_default_seconds=1800
  else
    supervisor_default_seconds=2700
    supervisor_preflight_default_seconds=900
  fi
  supervisor_deadline_seconds="${MAX_CLOUD_SECONDS:-$supervisor_default_seconds}"
  supervisor_preflight_seconds="${MAX_PREFLIGHT_SECONDS:-$supervisor_preflight_default_seconds}"
  [[ "$supervisor_deadline_seconds" =~ ^[0-9]+$ \
    && "$supervisor_deadline_seconds" -gt 0 \
    && "$supervisor_deadline_seconds" -le "$supervisor_default_seconds" ]] || {
    echo "MAX_CLOUD_SECONDS must be a positive integer no greater than $supervisor_default_seconds for this mode" >&2
    exit 1
  }
  [[ "$supervisor_preflight_seconds" =~ ^[0-9]+$ \
    && "$supervisor_preflight_seconds" -gt 0 \
    && "$supervisor_preflight_seconds" -le "$supervisor_preflight_default_seconds" ]] || {
    echo "MAX_PREFLIGHT_SECONDS must be a positive integer no greater than $supervisor_preflight_default_seconds for this mode" >&2
    exit 1
  }
  supervisor_ready_dir="$(mktemp -d "${TMPDIR:-/tmp}/axiom-gcp-supervisor.XXXXXX")"
  chmod 0700 "$supervisor_ready_dir"
  supervisor_ready_path="$supervisor_ready_dir/cloud-ready.txt"
  supervisor_ready_token="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
  export AXIOM_GCP_ACCEPTANCE_SUPERVISOR_READY_PATH="$supervisor_ready_path"
  export AXIOM_GCP_ACCEPTANCE_SUPERVISOR_READY_TOKEN="$supervisor_ready_token"
  export AXIOM_GCP_ACCEPTANCE_ISOLATED_SESSION=1
  exec python3 "$SCRIPT_DIR/run-supervisor.py" \
    --preflight-deadline-seconds "$supervisor_preflight_seconds" \
    --deadline-seconds "$supervisor_deadline_seconds" \
    --shutdown-grace-seconds 900 \
    --ready-path "$supervisor_ready_path" \
    --ready-token "$supervisor_ready_token" \
    --cleanup-ready-directory "$supervisor_ready_dir" \
    -- "$SCRIPT_DIR/run.sh" "$@"
fi
run_process_group="$(process_group_id "$$")"
[[ "$run_process_group" == "$$" ]] || {
  echo "acceptance run is not the leader of its isolated process group" >&2
  exit 1
}
: "${AXIOM_GCP_ACCEPTANCE_SUPERVISOR_READY_PATH:?missing supervisor ready path}"
: "${AXIOM_GCP_ACCEPTANCE_SUPERVISOR_READY_TOKEN:?missing supervisor ready token}"
[[ "$AXIOM_GCP_ACCEPTANCE_SUPERVISOR_READY_PATH" == /* \
  && "$AXIOM_GCP_ACCEPTANCE_SUPERVISOR_READY_TOKEN" =~ ^[0-9a-f]{64}$ ]] || {
  echo "invalid acceptance supervisor ready contract" >&2
  exit 1
}
: "${PROJECT_ID:?Set PROJECT_ID explicitly to the disposable GCP billing project}"
REGION="${REGION:-asia-east1}"
GKE_ZONE="${GKE_ZONE:-asia-east1-a}"
PERSISTENT_CLUSTER_NAME="${PERSISTENT_CLUSTER_NAME:-axiom-operator-acceptance}"
ARTIFACT_REGISTRY_REPOSITORY="${ARTIFACT_REGISTRY_REPOSITORY:-courier}"
RUN_ID="${RUN_ID:-$(date -u +%m%d%H%M%S)}"
SIFT_CANDIDATE_DIR="${SIFT_CANDIDATE_DIR:-}"
contained_sift=0
if [[ "${ACCEPTANCE_APPS:-lumen sift}" == "sift" \
  && "$container_role" == "run" ]]; then
  contained_sift=1
  verify_sift_candidate_directory "$SIFT_CANDIDATE_DIR" || {
    echo "the contained Sift run has no valid immutable candidate" >&2
    exit 1
  }
  GIT_SHA="$(jq -er '.git_sha' "$SIFT_CANDIDATE_DIR/candidate.json")"
else
  GIT_SHA="$(git -c core.fsmonitor=false -C "$REPO_ROOT" rev-parse HEAD)"
fi
IMAGE_TAG="${IMAGE_TAG:-${GIT_SHA}-${RUN_ID}}"
REGISTRY="${REGION}-docker.pkg.dev/${PROJECT_ID}/${ARTIFACT_REGISTRY_REPOSITORY}"
ACCEPTANCE_APPS="${ACCEPTANCE_APPS:-lumen sift}"
INPUT_LUMEN_IMAGE="${LUMEN_IMAGE:-}"
INPUT_SIFT_IMAGE="${SIFT_IMAGE:-}"
INPUT_RIG_IMAGE="${RIG_IMAGE:-}"
INPUT_TAPE_IMAGE="${TAPE_IMAGE:-}"
INPUT_SIFT_CLI="${SIFT_CLI:-}"
INPUT_SIFT_BIN="${SIFT_BIN:-}"
LUMEN_PRIOR_ACCEPTANCE="${LUMEN_PRIOR_ACCEPTANCE:-}"
LUMEN_AUTH_ISSUER_GSA="${LUMEN_AUTH_ISSUER_GSA:-}"
# Pre-declared so `export` under `set -u` never fails; each mode branch below
# fills in only the names it owns. Sift-only acceptance rejects a caller CLI;
# the other modes keep their existing local CLI override behavior. The
# *_IMAGE runtime variables are reset because their caller inputs were already
# captured into INPUT_* above.
LUMEN_CLI="${LUMEN_CLI:-}"
SIFT_CLI=""
SIFT_BIN=""
TAPE_CLI="${TAPE_CLI:-}"
LUMEN_IMAGE=""
SIFT_IMAGE=""
TAPE_IMAGE=""
RIG_IMAGE=""
STATE_DIR="${STATE_DIR:-/tmp/axiom-gcp-operator-${RUN_ID}}"
EVIDENCE_DIR="${EVIDENCE_DIR:-/tmp/axiom-gcp-operator-evidence/${RUN_ID}}"
MANIFEST_DIR="${MANIFEST_DIR:-$STATE_DIR/manifests}"
TERRAFORM_ENVIRONMENT_DIR="${TERRAFORM_ENVIRONMENT_DIR:-$STATE_DIR/environment}"
GCS_SOURCE_PREFIX="${GCS_SOURCE_PREFIX:-gs://${PROJECT_ID}_cloudbuild/source/axiom-gcp-operator-${RUN_ID}}"
sift_cloud_cap=5400
if [[ "$ACCEPTANCE_APPS" == "sift" ]]; then
  default_cloud_seconds="$sift_cloud_cap"
else
  default_cloud_seconds=2700
fi
MAX_CLOUD_SECONDS="${MAX_CLOUD_SECONDS:-$default_cloud_seconds}"
KUBECONFIG="${KUBECONFIG:-$STATE_DIR/kubeconfig}"
ACCEPTANCE_LOCAL_CLAIM_ROOT="${ACCEPTANCE_LOCAL_CLAIM_ROOT:-${TMPDIR:-/tmp}/axiom-gcp-operator-claims}"
cleanup_armed=0
cleanup_started=0
acceptance_lock_acquisition_id=""
cleanup_handoff_nonce=""
local_run_claim=""
local_run_claim_owned=0
watchdog_pid=""
watchdog_token=""
watchdog_descendants="$STATE_DIR/watchdog-descendants.txt"
watchdog_pid_file="$STATE_DIR/watchdog-pid.txt"
watchdog_process_record="$STATE_DIR/watchdog-process.txt"
watchdog_ready_file="$STATE_DIR/watchdog-ready.txt"
process_scan_failure="$STATE_DIR/process-scan-unsafe.txt"
run_log_process_record="$STATE_DIR/run-log-process.txt"
run_log_pipe="$STATE_DIR/run-log.pipe"
CANDIDATE_SOURCE_ARCHIVE=""
CANDIDATE_SOURCE_SHA256=""
CANDIDATE_CLOUD_BUILD_ID=""
CANDIDATE_SOURCE_URI=""
# Completion sentinel: bash expansion errors (set -u unbound variable)
# abort the script WITHOUT updating $?, so the EXIT trap's `local ec=$?`
# reads the PREVIOUS command's 0 — a false-green exit (runs 0724151638
# and 0724153400 both died mid-run yet exited 0). The trap therefore
# refuses ec=0 unless the last line of the script body really ran.
run_completed=0

require() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "required command not found: $1" >&2
    exit 1
  }
}

require_empty_list() {
  local label="$1"
  shift
  local output
  if ! output="$("$@" 2>/dev/null)"; then
    echo "could not inventory existing $label; refusing a destructive run" >&2
    exit 1
  fi
  if [[ -n "$output" ]]; then
    echo "refusing to reuse RUN_ID=$RUN_ID; existing $label:" >&2
    echo "$output" >&2
    exit 1
  fi
}

assert_acceptance_run_lock() {
  local resource
  resource="$(kubectl get lease "$(acceptance_lock_name)" \
    --namespace "$(acceptance_lock_namespace)" -o json)" || return 1
  verify_acceptance_lock_receipt_owner \
    "$EVIDENCE_DIR/acceptance-lock.json" "$resource" \
    "$PROJECT_ID" "$RUN_ID" "$acceptance_mode" || return 1
  jq -e '
    (.metadata.annotations["axiom.axiom.dev/cleanup-session-id"] // "") == ""
  ' <<<"$resource" >/dev/null
}

require_acceptance_run_lock() {
  local operation="$1"
  assert_acceptance_run_lock || {
    echo "the shared GKE Lease changed before $operation; refusing the mutation" >&2
    exit 1
  }
}

cleanup() {
  local ec=$?
  local watchdog_was_started=0
  local process_scan_safe=1
  local shutdown_status=0
  local run_log_close_status=0
  local local_claim_released=0
  if [[ "$ec" -eq 0 && "$run_completed" != "1" ]]; then
    echo "run aborted before completion (likely an expansion error above) — forcing failure exit" >&2
    ec=1
  fi
  if [[ "$contained_sift" == "1" ]]; then
    trap - EXIT INT TERM
    if [[ "$cleanup_armed" == "0" ]]; then
      if [[ "$local_run_claim_owned" == "0" ]]; then
        local_claim_released=1
      elif verify_acceptance_run_owner \
          "$local_run_claim" "$PROJECT_ID" "$RUN_ID" "$acceptance_mode" \
          "$acceptance_lock_acquisition_id" "$STATE_DIR" "$EVIDENCE_DIR" \
          "$$" "$run_process_group" "$run_owner_start_token" \
          "$cleanup_handoff_digest" \
          && rm -f "$local_run_claim" \
          && [[ ! -e "$local_run_claim" ]]; then
        local_run_claim_owned=0
        local_claim_released=1
      else
        echo "could not release the local preflight run claim" >&2
        ec=1
      fi
    fi
    exit_receipt="$EVIDENCE_DIR/run-exit.json"
    if ! jq -n \
        --argjson exit_code "$ec" \
        --argjson completed "$run_completed" \
        --argjson cleanup_armed "$cleanup_armed" \
        --argjson local_claim_released "$local_claim_released" \
        --arg finished_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" '
          {
            schema:"axiom.gcp.sift.contained-run-exit.v1",
            exit_code:$exit_code,
            completed:($completed == 1),
            cleanup_armed:($cleanup_armed == 1),
            local_claim_released:($local_claim_released == 1),
            finished_at:$finished_at
          }
        ' > "${exit_receipt}.tmp" \
        || ! chmod 0600 "${exit_receipt}.tmp" \
        || ! mv "${exit_receipt}.tmp" "$exit_receipt"; then
      rm -f "${exit_receipt}.tmp"
      ec=1
    fi
    echo "evidence: $EVIDENCE_DIR"
    exit "$ec"
  fi
  # Leave the FIFO before any signal, scan, or destructive cleanup. From this
  # point onward, output goes to the regular run.log file.
  if [[ "$RUN_LOG_OUTPUT_ACTIVE" == "1" \
    || "$RUN_LOG_DIRECT_OUTPUT_ACTIVE" == "1" \
    || -n "$RUN_LOG_TEE_PID" ]]; then
    if ! detach_run_log_for_cleanup; then
      echo "acceptance log sink was not healthy at cleanup entry" >&2
      ec=1
    fi
  fi
  if [[ "$RUN_LOG_DIRECT_OUTPUT_ACTIVE" != "1" ]]; then
    echo "acceptance cleanup could not append to the durable run log" >&2
    ec=1
  fi
  trap - EXIT INT TERM
  if [[ -n "$watchdog_pid" ]]; then
    watchdog_was_started=1
    if stop_process_generation_bounded "$watchdog_pid" "$watchdog_token"; then
      rm -f "$watchdog_process_record" >/dev/null 2>&1 || {
        echo "could not remove the stopped watchdog process record" >&2
        process_scan_safe=0
        ec=1
      }
      watchdog_pid=""
      watchdog_token=""
    else
      echo "watchdog generation remained live or unverifiable after bounded shutdown" >&2
      printf '%s\t%s\n' "$watchdog_pid" \
        "${watchdog_token:-$PROCESS_TOKEN_UNVERIFIABLE}" \
        >> "$watchdog_descendants" || true
      process_scan_safe=0
      ec=1
    fi
  fi
  if [[ -e "$process_scan_failure" ]]; then
    process_scan_safe=0
    ec=1
  fi
  if [[ "$watchdog_was_started" == "1" || -f "$watchdog_descendants" ]]; then
    # The foreground command can exit as soon as TERM reaches it. That starts
    # this EXIT trap while the watchdog is still in its grace period. Rescan
    # the isolated group here after stopping the watchdog, so a TERM handler
    # cannot fork and reparent an unrecorded child between the two paths.
    if shutdown_process_group_members \
        "$run_process_group" "$$" "" "$watchdog_descendants" \
        "$run_log_process_record" 5 1; then
      :
    else
      shutdown_status=$?
      if [[ "$shutdown_status" == "2" ]]; then
        printf '%s\n' "process-group enumeration failed during EXIT cleanup" \
          > "${process_scan_failure}.tmp"
      else
        printf '%s\n' "a process appeared in the final EXIT scan; refusing cleanup" \
          > "${process_scan_failure}.tmp"
      fi
      mv "${process_scan_failure}.tmp" "$process_scan_failure"
      process_scan_safe=0
      ec=1
    fi
  fi
  # Descendants inherit the FIFO writer. Close the sink only after the final
  # group scan proves that those writers are gone. Status 1 means the exact sink
  # generation is gone but the log failed. Status 2 means it is still unsafe.
  if [[ -n "$RUN_LOG_TEE_PID" ]]; then
    if close_run_log_sink; then
      :
    else
      run_log_close_status=$?
      ec=1
    fi
  fi
  if [[ "$run_log_close_status" == "2" ]]; then
    if ! printf '%s\t%s\n' "$RUN_LOG_TEE_PID" "$RUN_LOG_TEE_TOKEN" \
        >> "$watchdog_descendants"; then
      printf '%s\n' "could not persist the unsafe run-log generation" \
        > "${process_scan_failure}.tmp" || true
      mv "${process_scan_failure}.tmp" "$process_scan_failure" \
        >/dev/null 2>&1 || true
    fi
    process_scan_safe=0
  else
    run_log_process_record=""
  fi
  if [[ "$process_scan_safe" != "1" ]]; then
    echo "process-group membership is incomplete; refusing destructive cleanup" >&2
  elif [[ "$cleanup_armed" == "1" && "$cleanup_started" == "0" ]]; then
    cleanup_started=1
    if ! PROJECT_ID="$PROJECT_ID" REGION="$REGION" GKE_ZONE="$GKE_ZONE" RUN_ID="$RUN_ID" \
      STATE_DIR="$STATE_DIR" ACCEPTANCE_ROOT="$ACCEPTANCE_ROOT" \
      TERRAFORM_ENVIRONMENT_DIR="$TERRAFORM_ENVIRONMENT_DIR" \
      REGISTRY="$REGISTRY" IMAGE_TAG="$IMAGE_TAG" \
      GCS_SOURCE_PREFIX="$GCS_SOURCE_PREFIX" EVIDENCE_DIR="$EVIDENCE_DIR" \
      ACCEPTANCE_LOCAL_CLAIM_ROOT="$ACCEPTANCE_LOCAL_CLAIM_ROOT" \
      ACCEPTANCE_RUN_OWNER_HANDOFF_NONCE="$cleanup_handoff_nonce" \
      ARTIFACT_REGISTRY_REPOSITORY="$ARTIFACT_REGISTRY_REPOSITORY" \
      PERSISTENT_CLUSTER_NAME="$PERSISTENT_CLUSTER_NAME" \
      ACCEPTANCE_APPS="$ACCEPTANCE_APPS" \
      "$SCRIPT_DIR/cleanup.sh"; then
      echo "cleanup failed; Terraform state remains at $STATE_DIR" >&2
      ec=1
    fi
  fi
  echo "evidence: $EVIDENCE_DIR"
  if ! finish_run_log; then
    echo "acceptance log sink failed before it recorded all cleanup output" >&2
    ec=1
  fi
  echo "evidence: $EVIDENCE_DIR"
  exit "$ec"
}
trap cleanup EXIT
trap 'exit 130' INT
if [[ "$ACCEPTANCE_APPS" == "sift" ]]; then
  trap 'echo "90-minute Sift MVP cloud acceptance cap reached" >&2; exit 124' TERM
else
  trap 'echo "45-minute cloud acceptance cap reached" >&2; exit 124' TERM
fi

if [[ "$contained_sift" == "1" ]]; then
  required_commands=(awk curl gcloud gzip jq kubectl ln openssl ps python3 sort terraform)
else
  required_commands=(awk cargo curl gcloud git gzip jq kubectl ln mkfifo openssl ps python3 sort tar terraform)
fi
for command in "${required_commands[@]}"; do
  require "$command"
done
python3 -c 'import jsonschema' >/dev/null 2>&1 || {
  echo "python package 'jsonschema' is required for Sift evidence validation" >&2
  exit 1
}
[[ "$RUN_ID" =~ ^[a-z0-9][a-z0-9-]{0,17}$ ]] || {
  echo "RUN_ID must be 1-18 lowercase letters, digits, or hyphens" >&2
  exit 1
}
[[ "$GKE_ZONE" =~ ^[a-z]+-[a-z]+[0-9]-[a-z]$ ]] || {
  echo "GKE_ZONE must be a zone such as asia-east1-a" >&2
  exit 1
}
[[ "$MAX_CLOUD_SECONDS" =~ ^[0-9]+$ \
  && "$MAX_CLOUD_SECONDS" -gt 0 \
  && "$MAX_CLOUD_SECONDS" -le "$default_cloud_seconds" ]] || {
  echo "MAX_CLOUD_SECONDS must be a positive integer no greater than $default_cloud_seconds for this mode" >&2
  exit 1
}
case "$ACCEPTANCE_APPS" in
  "lumen sift") acceptance_mode="lumen-sift" ;;
  "lumen auth") acceptance_mode="lumen-auth" ;;
  "sift") acceptance_mode="sift" ;;
  "tape") acceptance_mode="tape" ;;
  *)
    echo "ACCEPTANCE_APPS must be exactly 'lumen sift' (default), 'lumen auth', 'sift', or 'tape'" >&2
    exit 1
    ;;
esac
if [[ "$acceptance_mode" != "tape" && "$acceptance_mode" != "sift" ]]; then
  : "${LUMEN_AUTH_ISSUER_GSA:?LUMEN_AUTH_ISSUER_GSA is required for auth-running modes}"
fi
if [[ "$acceptance_mode" == "sift" ]]; then
  [[ "$contained_sift" == "1" ]] || {
    echo "ACCEPTANCE_APPS=sift must enter through run-sift-contained.sh" >&2
    exit 1
  }
  [[ -z "$INPUT_SIFT_IMAGE" && -z "$INPUT_RIG_IMAGE" \
    && -z "$INPUT_SIFT_CLI" && -z "$INPUT_SIFT_BIN" ]] || {
    echo "contained Sift acceptance reads its CLI and images only from the candidate image" >&2
    exit 1
  }
  candidate_receipt="$SIFT_CANDIDATE_DIR/candidate.json"
  [[ "$(jq -er '.project_id' "$candidate_receipt")" == "$PROJECT_ID" \
    && "$(jq -er '.region' "$candidate_receipt")" == "$REGION" \
    && "$(jq -er '.run_id' "$candidate_receipt")" == "$RUN_ID" \
    && "$(jq -er '.git_sha' "$candidate_receipt")" == "$GIT_SHA" ]] || {
    echo "the candidate identity does not match this contained run" >&2
    exit 1
  }
  ARTIFACT_REGISTRY_REPOSITORY="$(jq -er \
    '.artifact_registry_repository' "$candidate_receipt")"
  REGISTRY="$(jq -er '.registry' "$candidate_receipt")"
  IMAGE_TAG="$(jq -er '.image_tag' "$candidate_receipt")"
  GCS_SOURCE_PREFIX="$(jq -er '.source_prefix' "$candidate_receipt")"
  SIFT_IMAGE="$(jq -er '.sift_image' "$candidate_receipt")"
  RIG_IMAGE="$(jq -er '.rig_image' "$candidate_receipt")"
  ACCEPTANCE_RUNNER_IMAGE="$(jq -er \
    '.acceptance_runner_image' "$candidate_receipt")"
  CANDIDATE_SOURCE_SHA256="$(jq -er \
    '.source_bundle_sha256' "$candidate_receipt")"
  CANDIDATE_CLOUD_BUILD_ID="$(jq -er '.cloud_build_id' "$candidate_receipt")"
  CANDIDATE_SOURCE_URI="$(jq -er '.source_object_uri' "$candidate_receipt")"
  SIFT_CLI="/usr/local/bin/sift"
  IMAGE_PROVENANCE="prebuilt"
  [[ -z "$LUMEN_PRIOR_ACCEPTANCE" ]] || {
    echo "LUMEN_PRIOR_ACCEPTANCE is meaningless in ACCEPTANCE_APPS=sift mode" >&2
    exit 1
  }
elif [[ "$acceptance_mode" == "tape" ]]; then
  [[ -z "$INPUT_TAPE_IMAGE" || "$INPUT_TAPE_IMAGE" == *@sha256:* ]] || {
    echo "caller-supplied service images must be immutable @sha256 digest references" >&2
    exit 1
  }
  if [[ -n "$INPUT_TAPE_IMAGE" ]]; then
    IMAGE_PROVENANCE="prebuilt"
  else
    IMAGE_PROVENANCE="cloud-build"
  fi
  [[ -z "$LUMEN_PRIOR_ACCEPTANCE" ]] || {
    echo "LUMEN_PRIOR_ACCEPTANCE is meaningless in ACCEPTANCE_APPS=tape mode" >&2
    exit 1
  }
else
  if [[ "$acceptance_mode" == "lumen-auth" ]]; then
    [[ -z "$INPUT_LUMEN_IMAGE" || "$INPUT_LUMEN_IMAGE" == *@sha256:* ]] || {
      echo "caller-supplied service images must be immutable @sha256 digest references" >&2
      exit 1
    }
    IMAGE_PROVENANCE="prebuilt"
    [[ -n "$INPUT_LUMEN_IMAGE" ]] || IMAGE_PROVENANCE="cloud-build"
    [[ -z "$LUMEN_PRIOR_ACCEPTANCE" ]] || {
      echo "LUMEN_PRIOR_ACCEPTANCE is not used in ACCEPTANCE_APPS=lumen auth mode" >&2
      exit 1
    }
  elif [[ "$acceptance_mode" != "tape" ]]; then
  for input_image in "$INPUT_LUMEN_IMAGE" "$INPUT_SIFT_IMAGE"; do
    [[ -z "$input_image" || "$input_image" == *@sha256:* ]] || {
      echo "caller-supplied service images must be immutable @sha256 digest references" >&2
      exit 1
    }
  done
  if [[ -n "$INPUT_LUMEN_IMAGE" && -n "$INPUT_SIFT_IMAGE" ]]; then
    IMAGE_PROVENANCE="prebuilt"
  elif [[ -n "$INPUT_LUMEN_IMAGE" || -n "$INPUT_SIFT_IMAGE" ]]; then
    IMAGE_PROVENANCE="mixed"
  else
    IMAGE_PROVENANCE="cloud-build"
  fi
  fi
fi
if [[ -n "$LUMEN_PRIOR_ACCEPTANCE" ]]; then
  [[ -f "$LUMEN_PRIOR_ACCEPTANCE" ]] || {
    echo "LUMEN_PRIOR_ACCEPTANCE must name an existing Lumen acceptance JSON file" >&2
    exit 1
  }
  jq -e '
    .schema == "axiom.gcp.lumen.acceptance.v1"
    and .operator_reconcile_1x1 == "passed"
    and .pod_restart_data_retention == "passed"
    and .gcs_backup_before_split == "passed"
    and .auto_split_delta == 1
  ' "$LUMEN_PRIOR_ACCEPTANCE" >/dev/null || {
    echo "LUMEN_PRIOR_ACCEPTANCE is not a completed Lumen acceptance proof" >&2
    exit 1
  }
fi

if ! SOURCE_BUCKET="$(validated_source_bucket "$GCS_SOURCE_PREFIX" "$RUN_ID")"; then
  echo "GCS_SOURCE_PREFIX must be exactly gs://BUCKET/source/axiom-gcp-operator-RUN_ID" >&2
  exit 1
fi

for run_dir in "$STATE_DIR" "$EVIDENCE_DIR"; do
  if [[ -e "$run_dir" && -n "$(find "$run_dir" -mindepth 1 -print -quit 2>/dev/null)" ]]; then
    echo "refusing to reuse nonempty run directory: $run_dir" >&2
    exit 1
  fi
done

[[ "$STATE_DIR" != "$EVIDENCE_DIR" ]] || {
  echo "STATE_DIR and EVIDENCE_DIR must be different directories" >&2
  exit 1
}
acceptance_lock_acquisition_id="$(openssl rand -hex 16)"
if [[ "$contained_sift" == "1" ]]; then
  cleanup_handoff_digest="${ACCEPTANCE_CONTAINER_HANDOFF_DIGEST:-}"
else
  cleanup_handoff_nonce="$(openssl rand -hex 32)"
  cleanup_handoff_digest="$(
    printf '%s' "$cleanup_handoff_nonce" | openssl dgst -sha256 | awk '{print $NF}'
  )"
fi
run_owner_start_token="$(process_start_token "$$")"
[[ "$acceptance_lock_acquisition_id" =~ ^[0-9a-f]{32}$ \
  && ( "$contained_sift" == "1" || "$cleanup_handoff_nonce" =~ ^[0-9a-f]{64}$ ) \
  && "$cleanup_handoff_digest" =~ ^[0-9a-f]{64}$ \
  && -n "$run_owner_start_token" ]] || {
  echo "could not create the local acceptance run identity" >&2
  exit 1
}
mkdir -p "$ACCEPTANCE_LOCAL_CLAIM_ROOT" || exit 1
local_run_claim="$(acceptance_run_claim_path \
  "$ACCEPTANCE_LOCAL_CLAIM_ROOT" "$PROJECT_ID" "$RUN_ID" "$acceptance_mode")"
claim_candidate="$ACCEPTANCE_LOCAL_CLAIM_ROOT/.acceptance-run-${acceptance_lock_acquisition_id}.json"
write_acceptance_run_owner \
  "$claim_candidate" "$PROJECT_ID" "$RUN_ID" "$acceptance_mode" \
  "$acceptance_lock_acquisition_id" "$STATE_DIR" "$EVIDENCE_DIR" \
  "$$" "$run_process_group" "$run_owner_start_token" \
  "$cleanup_handoff_digest" || {
  echo "could not persist the local acceptance run identity" >&2
  exit 1
}
if ! ln "$claim_candidate" "$local_run_claim"; then
  rm -f "$claim_candidate"
  echo "another process already reserved PROJECT_ID=$PROJECT_ID RUN_ID=$RUN_ID mode=$acceptance_mode" >&2
  exit 1
fi
local_run_claim_owned=1
rm -f "$claim_candidate"
verify_acceptance_run_owner \
  "$local_run_claim" "$PROJECT_ID" "$RUN_ID" "$acceptance_mode" \
  "$acceptance_lock_acquisition_id" "$STATE_DIR" "$EVIDENCE_DIR" \
  "$$" "$run_process_group" "$run_owner_start_token" \
  "$cleanup_handoff_digest" || {
  echo "could not verify the atomic local acceptance claim" >&2
  exit 1
}
mkdir -p "$STATE_DIR" "$EVIDENCE_DIR" || exit 1
mkdir -p "$EVIDENCE_DIR/kubernetes" "$EVIDENCE_DIR/gcs"
export KUBECONFIG
if [[ "$contained_sift" == "1" ]]; then
  copy_sift_candidate_evidence "$SIFT_CANDIDATE_DIR" "$EVIDENCE_DIR" || {
    echo "could not copy the fixed candidate evidence" >&2
    exit 1
  }
  verify_sift_candidate_directory "$EVIDENCE_DIR" || {
    echo "the copied candidate evidence did not verify" >&2
    exit 1
  }
else
  start_run_log "$EVIDENCE_DIR/run.log" "$run_log_process_record" "$run_log_pipe" || {
    echo "could not start the durable acceptance log sink" >&2
    exit 1
  }
fi

required_apis=(
  artifactregistry.googleapis.com
  cloudbuild.googleapis.com
  compute.googleapis.com
  container.googleapis.com
  iam.googleapis.com
  iamcredentials.googleapis.com
  storage.googleapis.com
)
: > "$EVIDENCE_DIR/preexisting-apis.txt"
for api in "${required_apis[@]}"; do
  enabled="$(gcloud services list --enabled --project="$PROJECT_ID" \
    --filter="config.name=${api}" --format='value(config.name)')"
  if [[ "$enabled" != "$api" ]]; then
    echo "required API is not already enabled: $api" >&2
    echo "The harness never changes project API state; enable it explicitly before retrying." >&2
    exit 1
  fi
  printf '%s\n' "$api" >> "$EVIDENCE_DIR/preexisting-apis.txt"
done
artifact_registry_receipt="$EVIDENCE_DIR/preexisting-artifact-registry.json"
if [[ "$contained_sift" == "1" ]]; then
  # This name is part of the immutable candidate bundle. Keep the live
  # inventory separate so later cleanup can still verify that bundle.
  artifact_registry_receipt="$EVIDENCE_DIR/live-artifact-registry.json"
fi
gcloud artifacts repositories describe "$ARTIFACT_REGISTRY_REPOSITORY" \
  --project="$PROJECT_ID" --location="$REGION" --format=json \
  > "$artifact_registry_receipt"
if [[ "$acceptance_mode" != "lumen-auth" ]]; then
  require_empty_list "backup bucket" gcloud storage buckets list --project="$PROJECT_ID" \
    --filter="name=${PROJECT_ID}-axo-${RUN_ID}-backup" --format='value(name)'
  require_empty_list "backup service account" gcloud iam service-accounts list \
    --project="$PROJECT_ID" \
    --filter="email:axo-${RUN_ID}-backup@${PROJECT_ID}.iam.gserviceaccount.com" \
    --format='value(email)'
fi
if [[ "$IMAGE_PROVENANCE" != "prebuilt" ]]; then
  if ! gcloud storage buckets describe "gs://${SOURCE_BUCKET}" --project="$PROJECT_ID" \
    --format=json > "$EVIDENCE_DIR/preexisting-cloud-build-source-bucket.json"; then
    echo "Cloud Build source bucket must already exist; the harness will not create or leak one" >&2
    exit 1
  fi
  if ! gcloud storage ls --recursive "gs://${SOURCE_BUCKET}" \
    > "$EVIDENCE_DIR/preexisting-cloud-build-source-objects.txt"; then
    echo "could not inventory the pre-existing Cloud Build source bucket" >&2
    exit 1
  fi
  if rg -Fx "$GCS_SOURCE_PREFIX" "$EVIDENCE_DIR/preexisting-cloud-build-source-objects.txt" >/dev/null \
      || rg -F "${GCS_SOURCE_PREFIX}/" "$EVIDENCE_DIR/preexisting-cloud-build-source-objects.txt" >/dev/null; then
    echo "refusing to reuse Cloud Build source prefix: $GCS_SOURCE_PREFIX" >&2
    exit 1
  fi
  write_source_prefix_receipt \
    "$EVIDENCE_DIR/source-prefix.json" "$PROJECT_ID" "$RUN_ID" "$GCS_SOURCE_PREFIX"
fi

if [[ "$acceptance_mode" == "sift" ]]; then
  image_list=(sift rig sift-acceptance-runner)
elif [[ "$acceptance_mode" == "tape" ]]; then
  image_list=(tape)
elif [[ "$acceptance_mode" == "lumen-auth" ]]; then
  image_list=(lumen)
else
  image_list=(lumen sift)
fi
for image in "${image_list[@]}"; do
  [[ "$contained_sift" != "1" ]] || continue
  inventory="$EVIDENCE_DIR/preexisting-${image}-images.json"
  list_stderr="$STATE_DIR/preexisting-${image}-images.stderr"
  if gcloud artifacts docker images list "$REGISTRY/$image" \
    --project="$PROJECT_ID" --include-tags --format=json > "$inventory" 2>"$list_stderr"; then
    :
  elif [[ "$image" == "tape" || "$image" == "rig" ]] && rg -F "NOT_FOUND" "$list_stderr" >/dev/null; then
    # A test-only package may not exist in the registry on its first run.
    printf '[]' > "$inventory"
  else
    echo "could not inventory existing $image images; refusing a destructive run" >&2
    cat "$list_stderr" >&2
    exit 1
  fi
  if jq -e --arg tag "$IMAGE_TAG" \
    'any(.[]; ((.tags // []) | index($tag)) != null)' "$inventory" >/dev/null; then
    echo "refusing to overwrite existing image tag: $REGISTRY/$image:$IMAGE_TAG" >&2
    exit 1
  fi
done

if [[ "$contained_sift" == "1" ]]; then
  : > "$EVIDENCE_DIR/source-git-status.txt"
else
  git -c core.fsmonitor=false -C "$REPO_ROOT" status --porcelain=v1 \
    > "$EVIDENCE_DIR/source-git-status.txt"
  if [[ -s "$EVIDENCE_DIR/source-git-status.txt" ]]; then
    echo "refusing Cloud Build from a dirty tree; commit the exact source before GCP acceptance" >&2
    cat "$EVIDENCE_DIR/source-git-status.txt" >&2
    exit 1
  fi
fi

echo ">> local deployment CLI build and render-surface preflight"
if [[ "$acceptance_mode" == "sift" ]]; then
  [[ -x "$SIFT_CLI" ]] || {
    echo "the candidate controller does not contain the Sift CLI" >&2
    exit 1
  }
elif [[ "$acceptance_mode" == "tape" ]]; then
  cargo build --locked --manifest-path "$REPO_ROOT/Cargo.toml" \
    -p tape --bin tape --features "operator backup"
  TAPE_CLI="${TAPE_CLI:-$REPO_ROOT/target/debug/tape}"
else
  # `delegated-auth` alongside `operator` (#2879): the auth leg drives `lumen
  # query --client-sa`, whose TokenRequest minter is behind that feature. A CLI
  # built without it refuses the flag rather than minting, so the auth leg would
  # fail on a build flag instead of on the contract it exists to check.
  lumen_src="$REPO_ROOT/target/faberline/lumen"
  bash "$ACCEPTANCE_ROOT/scripts/faberline-source.sh" lumen "$lumen_src" >/dev/null
  cargo build --locked --manifest-path "$lumen_src/Cargo.toml" --target-dir "$REPO_ROOT/target" \
    -p lumen --bin lumen --features "operator delegated-auth"
  LUMEN_CLI="${LUMEN_CLI:-$REPO_ROOT/target/debug/lumen}"
  if [[ "$acceptance_mode" != "lumen-auth" ]]; then
    cargo build --locked --manifest-path "$REPO_ROOT/Cargo.toml" \
      -p sift --bin sift
    SIFT_CLI="${SIFT_CLI:-$REPO_ROOT/target/debug/sift}"
  fi
fi

echo ">> existing persistent Standard GKE ownership preflight"
if ! gcloud container clusters describe "$PERSISTENT_CLUSTER_NAME" \
    --project="$PROJECT_ID" --zone="$GKE_ZONE" --format=json \
    > "$EVIDENCE_DIR/persistent-cluster.json"; then
  echo "the shared acceptance cluster must exist before a run can claim shared names" >&2
  echo "run acceptance/gcp/scripts/bootstrap-cluster.sh explicitly, then retry" >&2
  exit 1
fi
printf '%s\n' "$PERSISTENT_CLUSTER_NAME" \
  > "$EVIDENCE_DIR/persistent-cluster-name.txt"
gcloud container clusters get-credentials "$PERSISTENT_CLUSTER_NAME" \
  --project="$PROJECT_ID" --zone="$GKE_ZONE"

# This create-only Lease is the first run-owned shared mutation. It protects
# the fixed namespaces, RUN_ID-derived image tag, Cloud Build tag and source
# prefix. A random acquisition ID separates two processes that reused one
# RUN_ID. If the create response is lost, GET can identify the accepted Lease.
acceptance_lock_intent="$EVIDENCE_DIR/acceptance-lock-intent.json"
write_acceptance_lock_intent \
  "$acceptance_lock_intent" "$PROJECT_ID" "$RUN_ID" \
  "$acceptance_mode" "$acceptance_lock_acquisition_id" || {
  echo "could not persist the GKE acceptance lock intent" >&2
  exit 1
}
# Arm cleanup before the create request. The intent lets cleanup recover an
# accepted Lease even if this process stops before it can save the API reply.
cleanup_armed=1
acceptance_lock_error="$STATE_DIR/acceptance-lock-create.stderr"
if ! acceptance_lock_resource="$(
    acceptance_lock_manifest \
      "$PROJECT_ID" "$RUN_ID" "$acceptance_mode" "$acceptance_lock_acquisition_id" \
      | kubectl create -f - -o json 2>"$acceptance_lock_error"
  )"; then
  if acceptance_lock_resource="$(kubectl get lease "$(acceptance_lock_name)" \
      --namespace "$(acceptance_lock_namespace)" -o json 2>>"$acceptance_lock_error")" \
      && verify_acceptance_lock_json \
        "$acceptance_lock_resource" "$PROJECT_ID" "$RUN_ID" \
        "$acceptance_mode" "$acceptance_lock_acquisition_id"; then
    echo "recovered the accepted shared GKE Lease after an uncertain create response" >&2
  else
    echo "another acceptance run owns the shared GKE acceptance lock" >&2
    cat "$acceptance_lock_error" >&2
    exit 1
  fi
fi
write_acceptance_lock_receipt \
  "$EVIDENCE_DIR/acceptance-lock.json" "$acceptance_lock_resource" \
  "$PROJECT_ID" "$RUN_ID" "$acceptance_mode" "$acceptance_lock_acquisition_id" || {
  echo "could not bind the GKE acceptance lock to this run" >&2
  exit 1
}
ACCEPTANCE_LOCK_ACQUISITION_ID="$acceptance_lock_acquisition_id"
export ACCEPTANCE_LOCK_ACQUISITION_ID

if [[ "$acceptance_mode" == "tape" ]]; then
  mode_namespaces=(tape tape-system)
elif [[ "$acceptance_mode" == "sift" ]]; then
  mode_namespaces=(sift sift-system sift-restore)
elif [[ "$acceptance_mode" == "lumen-auth" ]]; then
  mode_namespaces=(lumen lumen-system lumen-auth-client)
else
  mode_namespaces=(lumen lumen-system sift sift-system lumen-auth-client)
fi
for namespace in "${mode_namespaces[@]}"; do
  require_kubernetes_resource_absent namespace "$namespace" || exit 1
done
if [[ "$acceptance_mode" == "sift" || "$acceptance_mode" == "lumen-sift" ]]; then
  require_kubernetes_resource_absent \
    customresourcedefinition sifts.sift.axiom.dev || exit 1
fi
printf '%s\n' "$PERSISTENT_CLUSTER_NAME" > "$STATE_DIR/kube-context-ready.txt"

# The watchdog polls its parent between short sleeps and disarms itself the
# moment the parent is gone. A single long sleep followed by an unconditional
# `kill -TERM $$` outlives a parent that died without reaching cleanup(), and
# minutes later fires at whatever process RECYCLED the parent's pid — attempt
# 0723113842 was killed exactly that way by a prior run's leftover.
run_main_pid="$$"
(
  watchdog_self="$(wait_for_process_id_file "$watchdog_pid_file" "$run_main_pid")" \
    || exit 0
  waited=0
  if ! append_process_group_members_with_retry \
      "$run_process_group" "$run_main_pid" "$watchdog_self" "$watchdog_descendants" \
      "$run_log_process_record" 5; then
    report_process_scan_failure \
      "$process_scan_failure" "process-group enumeration failed in watchdog" \
      "$run_main_pid" "$run_owner_start_token" || true
    exit 1
  fi
  (
    umask 077
    printf 'complete\t%s\n' "$AXIOM_GCP_ACCEPTANCE_SUPERVISOR_READY_TOKEN" \
      > "${AXIOM_GCP_ACCEPTANCE_SUPERVISOR_READY_PATH}.tmp"
  )
  mv "${AXIOM_GCP_ACCEPTANCE_SUPERVISOR_READY_PATH}.tmp" \
    "$AXIOM_GCP_ACCEPTANCE_SUPERVISOR_READY_PATH"
  printf '%s\n' "complete" > "${watchdog_ready_file}.tmp"
  mv "${watchdog_ready_file}.tmp" "$watchdog_ready_file"
  while (( waited < MAX_CLOUD_SECONDS )); do
    sleep 10
    waited=$((waited + 10))
    if ! append_process_group_members_with_retry \
        "$run_process_group" "$run_main_pid" "$watchdog_self" "$watchdog_descendants" \
        "$run_log_process_record" 5; then
      report_process_scan_failure \
        "$process_scan_failure" "process-group enumeration failed in watchdog" \
        "$run_main_pid" "$run_owner_start_token" || true
      exit 1
    fi
    if process_generation_state "$run_main_pid" "$run_owner_start_token"; then
      :
    else
      parent_state=$?
      [[ "$parent_state" == "1" ]] && exit 0
      report_process_scan_failure \
        "$process_scan_failure" \
        "parent process generation became unverifiable in watchdog" \
        "$run_main_pid" "$run_owner_start_token" || true
      exit 1
    fi
  done
  signal_recorded_processes "$watchdog_descendants" TERM
  sleep 10
  if ! append_process_group_members_with_retry \
      "$run_process_group" "$run_main_pid" "$watchdog_self" "$watchdog_descendants" \
      "$run_log_process_record" 5; then
    report_process_scan_failure \
      "$process_scan_failure" "process-group enumeration failed in watchdog" \
      "$run_main_pid" "$run_owner_start_token" || true
    exit 1
  fi
  signal_recorded_processes "$watchdog_descendants" KILL
  signal_process_generation \
    "$run_main_pid" "$run_owner_start_token" TERM >/dev/null 2>&1 || true
) &
watchdog_pid="$!"
watchdog_token="$(process_start_token "$watchdog_pid")" || {
  echo "could not bind the watchdog to its process generation" >&2
  exit 1
}
printf '%s\t%s\n' "$watchdog_pid" "$watchdog_token" \
  > "${watchdog_process_record}.tmp"
mv "${watchdog_process_record}.tmp" "$watchdog_process_record"
printf '%s\n' "$watchdog_pid" > "${watchdog_pid_file}.tmp"
mv "${watchdog_pid_file}.tmp" "$watchdog_pid_file"
watchdog_ready=0
for watchdog_ready_attempt in 1 2 3 4 5 6 7 8 9 10; do
  if [[ -e "$process_scan_failure" ]]; then
    break
  fi
  if process_generation_state "$watchdog_pid" "$watchdog_token"; then
    if [[ -f "$watchdog_ready_file" && ! -L "$watchdog_ready_file" \
      && "$(<"$watchdog_ready_file")" == "complete" ]]; then
      watchdog_ready=1
      break
    fi
  else
    break
  fi
  sleep 1
done
[[ "$watchdog_ready" == "1" ]] || {
  echo "watchdog could not complete the initial process-group scan" >&2
  exit 1
}

resolve_digest() {
  local image="$1"
  local digest
  digest="$(gcloud artifacts docker images describe "$REGISTRY/$image:$IMAGE_TAG" \
    --project="$PROJECT_ID" --format='value(image_summary.digest)')"
  [[ "$digest" == sha256:* ]] || {
    echo "could not resolve immutable digest for $image:$IMAGE_TAG" >&2
    return 1
  }
  printf '%s@%s\n' "$REGISTRY/$image" "$digest"
}

if [[ "$IMAGE_PROVENANCE" == "prebuilt" ]]; then
  echo ">> using caller-supplied immutable release or candidate images"
  if [[ "$acceptance_mode" == "sift" && "$contained_sift" == "1" ]]; then
    :
  elif [[ "$acceptance_mode" == "sift" ]]; then
    SIFT_IMAGE="$INPUT_SIFT_IMAGE"
    RIG_IMAGE="$INPUT_RIG_IMAGE"
  elif [[ "$acceptance_mode" == "tape" ]]; then
    TAPE_IMAGE="$INPUT_TAPE_IMAGE"
  elif [[ "$acceptance_mode" == "lumen-auth" ]]; then
    LUMEN_IMAGE="$INPUT_LUMEN_IMAGE"
  else
    LUMEN_IMAGE="$INPUT_LUMEN_IMAGE"
    SIFT_IMAGE="$INPUT_SIFT_IMAGE"
  fi
else
  if [[ "$acceptance_mode" == "sift" ]]; then
    echo "Sift acceptance cannot build after the contained GKE run starts" >&2
    exit 1
  elif [[ "$acceptance_mode" == "tape" ]]; then
    CLOUD_BUILD_CONFIG="$ACCEPTANCE_ROOT/cloudbuild.tape.yaml"
  elif [[ "$acceptance_mode" == "lumen-auth" ]]; then
    CLOUD_BUILD_CONFIG="$ACCEPTANCE_ROOT/cloudbuild.lumen.yaml"
  else
    CLOUD_BUILD_CONFIG="$ACCEPTANCE_ROOT/cloudbuild.yaml"
    if [[ -n "$INPUT_LUMEN_IMAGE" ]]; then
      CLOUD_BUILD_CONFIG="$ACCEPTANCE_ROOT/cloudbuild.sift.yaml"
    elif [[ -n "$INPUT_SIFT_IMAGE" ]]; then
      CLOUD_BUILD_CONFIG="$ACCEPTANCE_ROOT/cloudbuild.lumen.yaml"
    fi
  fi
  cloud_build_substitutions="_REGISTRY=$REGISTRY,_TAG=$IMAGE_TAG,_RUN_ID=$RUN_ID"
  case "${CLOUD_BUILD_CONFIG##*/}" in
    cloudbuild.sift-mvp.yaml|cloudbuild.sift.yaml|cloudbuild.yaml)
      cloud_build_substitutions="${cloud_build_substitutions},_GIT_SHA=$GIT_SHA"
      ;;
  esac
  echo ">> Cloud Build: source candidate only for service image(s) not supplied by digest"
  require_acceptance_run_lock "Cloud Build submit"
  build_id="$(gcloud builds submit "$REPO_ROOT" \
    --async \
    --project="$PROJECT_ID" \
    --region="$REGION" \
    --config="$CLOUD_BUILD_CONFIG" \
    --gcs-source-staging-dir="$GCS_SOURCE_PREFIX" \
    --ignore-file="$ACCEPTANCE_ROOT/gcloudignore" \
    --substitutions="$cloud_build_substitutions" \
    --format='value(id)')"
  [[ -n "$build_id" && "$build_id" != "null" ]] || {
    echo "Cloud Build did not return a build id" >&2
    exit 1
  }
  printf '%s\n' "$build_id" > "$STATE_DIR/cloud-build-id.txt"
  gcloud builds describe "$build_id" --project="$PROJECT_ID" --region="$REGION" \
    --format=json > "$EVIDENCE_DIR/cloud-build-submit.json"
  source_object_bucket="$(jq -r '.source.storageSource.bucket' "$EVIDENCE_DIR/cloud-build-submit.json")"
  source_object_name="$(jq -r '.source.storageSource.object' "$EVIDENCE_DIR/cloud-build-submit.json")"
  [[ -n "$source_object_bucket" && "$source_object_bucket" != "null" && -n "$source_object_name" && "$source_object_name" != "null" ]] || {
    echo "Cloud Build did not expose its exact staged source object" >&2
    exit 1
  }
  source_object_uri="$(validated_source_object_uri \
    "$GCS_SOURCE_PREFIX" "$RUN_ID" "$source_object_bucket" "$source_object_name")" || {
    echo "Cloud Build staged source outside the run-scoped prefix" >&2
    exit 1
  }
  gcloud storage objects describe "$source_object_uri" \
    --format=json > "$EVIDENCE_DIR/cloud-build-source-object.json"
  while true; do
    build_status="$(gcloud builds describe "$build_id" --project="$PROJECT_ID" \
      --region="$REGION" --format='value(status)')"
    printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$build_status" \
      >> "$EVIDENCE_DIR/cloud-build-status.log"
    case "$build_status" in
      SUCCESS) break ;;
      FAILURE|INTERNAL_ERROR|TIMEOUT|CANCELLED|EXPIRED)
        echo "Cloud Build ended with $build_status" >&2
        exit 1
        ;;
    esac
    sleep 10
  done
  gcloud builds describe "$build_id" --project="$PROJECT_ID" --region="$REGION" \
    --format=json > "$EVIDENCE_DIR/cloud-build-final.json"
  if [[ "$acceptance_mode" == "tape" ]]; then
    TAPE_IMAGE="$(resolve_digest tape)"
  elif [[ "$acceptance_mode" == "lumen-auth" ]]; then
    LUMEN_IMAGE="$(resolve_digest lumen)"
  else
    if [[ -n "$INPUT_LUMEN_IMAGE" ]]; then
      LUMEN_IMAGE="$INPUT_LUMEN_IMAGE"
    else
      LUMEN_IMAGE="$(resolve_digest lumen)"
    fi
    if [[ -n "$INPUT_SIFT_IMAGE" ]]; then
      SIFT_IMAGE="$INPUT_SIFT_IMAGE"
    else
      SIFT_IMAGE="$(resolve_digest sift)"
    fi
  fi
fi
if [[ "$acceptance_mode" == "sift" ]]; then
  live_build="$EVIDENCE_DIR/cloud-build-live.json"
  gcloud builds describe "$CANDIDATE_CLOUD_BUILD_ID" \
    --project="$PROJECT_ID" --region="$REGION" --format=json > "$live_build"
  for build_receipt in "$EVIDENCE_DIR/cloud-build-final.json" "$live_build"; do
    verify_sift_candidate_build_receipt \
      "$EVIDENCE_DIR/candidate.json" "$build_receipt" || {
      echo "Cloud Build receipt is not bound to this candidate: $build_receipt" >&2
      exit 1
    }
  done
  for built_image in sift rig sift-acceptance-runner; do
    case "$built_image" in
      sift) digest_ref="$SIFT_IMAGE" ;;
      rig) digest_ref="$RIG_IMAGE" ;;
      sift-acceptance-runner) digest_ref="$ACCEPTANCE_RUNNER_IMAGE" ;;
    esac
    gcloud artifacts docker images describe "$digest_ref" \
      --project="$PROJECT_ID" --format=json \
      > "$EVIDENCE_DIR/live-${built_image}-image.json" || {
      echo "candidate image is no longer available: $digest_ref" >&2
      exit 1
    }
  done
fi
if [[ "$acceptance_mode" == "sift" ]]; then
  jq -e \
    --arg sift "$SIFT_IMAGE" \
    --arg rig "$RIG_IMAGE" \
    --arg acceptance_runner "$ACCEPTANCE_RUNNER_IMAGE" '
      keys == ["acceptance_runner","rig","sift"]
      and .sift == $sift
      and .rig == $rig
      and .acceptance_runner == $acceptance_runner
    ' "$EVIDENCE_DIR/images.json" >/dev/null || {
    echo "copied candidate image receipt changed before GKE" >&2
    exit 1
  }
elif [[ "$acceptance_mode" == "tape" ]]; then
  jq -n --arg tape "$TAPE_IMAGE" '{tape:$tape}' > "$EVIDENCE_DIR/images.json"
elif [[ "$acceptance_mode" == "lumen-auth" ]]; then
  jq -n --arg lumen "$LUMEN_IMAGE" '{lumen:$lumen}' > "$EVIDENCE_DIR/images.json"
else
  jq -n --arg lumen "$LUMEN_IMAGE" --arg sift "$SIFT_IMAGE" \
    '{lumen:$lumen,sift:$sift}' > "$EVIDENCE_DIR/images.json"
fi

# Resource names are deterministic Terraform values, so render and validate all
# app-owned Kubernetes layers before creating run-scoped resources.
if [[ "$acceptance_mode" != "lumen-auth" ]]; then
  BACKUP_BUCKET="${PROJECT_ID}-axo-${RUN_ID}-backup"
  BACKUP_GSA_EMAIL="axo-${RUN_ID}-backup@${PROJECT_ID}.iam.gserviceaccount.com"
  export BACKUP_BUCKET BACKUP_GSA_EMAIL
fi
# The cluster this run actually uses. It was `axo-${RUN_ID}-gke` — a name left
# over from the disposable-cluster era that has not existed since the run moved
# to the persistent cluster, so anything reading it (the Sift collector's
# `clusterName`, and now the placement leg's node-pool drift check) was reading
# a cluster that is not there. Line 491 already asserts the Terraform output
# equals PERSISTENT_CLUSTER_NAME, so this is the same value, named once.
GKE_CLUSTER_NAME="$PERSISTENT_CLUSTER_NAME"
export GKE_CLUSTER_NAME GKE_ZONE PROJECT_ID REGION
export RUN_ID MANIFEST_DIR ACCEPTANCE_APPS
# Export mode-specific CLIs and images only
if [[ "$acceptance_mode" == "sift" ]]; then
  CANDIDATE_GIT_SHA="$GIT_SHA"
  export SIFT_CLI SIFT_IMAGE RIG_IMAGE ACCEPTANCE_RUNNER_IMAGE CANDIDATE_GIT_SHA
  export CANDIDATE_SOURCE_SHA256 CANDIDATE_CLOUD_BUILD_ID CANDIDATE_SOURCE_URI
elif [[ "$acceptance_mode" == "tape" ]]; then
  export TAPE_CLI TAPE_IMAGE
elif [[ "$acceptance_mode" == "lumen-auth" ]]; then
  export LUMEN_CLI LUMEN_IMAGE LUMEN_AUTH_ISSUER_GSA
else
  export LUMEN_CLI LUMEN_IMAGE SIFT_CLI SIFT_IMAGE
fi
"$SCRIPT_DIR/render-manifests.sh" || {
  echo "manifest rendering failed" >&2
  exit 1
}

echo ">> persistent Standard GKE cluster bootstrap or reuse"
# All acceptance modes are existing-only. Only a separate direct bootstrap
# command may create shared infrastructure; API read failures stop this run.
PROJECT_ID="$PROJECT_ID" REGION="$REGION" GKE_ZONE="$GKE_ZONE" \
  PERSISTENT_CLUSTER_NAME="$PERSISTENT_CLUSTER_NAME" \
  "$SCRIPT_DIR/bootstrap-cluster.sh" --existing-only > "$EVIDENCE_DIR/persistent-cluster-name.txt"
bootstrapped_cluster="$(cat "$EVIDENCE_DIR/persistent-cluster-name.txt")"
[[ "$bootstrapped_cluster" == "$PERSISTENT_CLUSTER_NAME" ]] || {
  echo "bootstrap-cluster.sh must emit exactly '$PERSISTENT_CLUSTER_NAME' on stdout" >&2
  echo "got $(wc -l < "$EVIDENCE_DIR/persistent-cluster-name.txt" | tr -d ' ') line(s); first and last:" >&2
  sed -n '1p;$p' "$EVIDENCE_DIR/persistent-cluster-name.txt" >&2
  exit 1
}

jq -n \
  --arg schema "axiom.gcp.operator.run.v1" \
  --arg project_id "$PROJECT_ID" \
  --arg region "$REGION" \
  --arg gke_zone "$GKE_ZONE" \
  --arg run_id "$RUN_ID" \
  --arg git_sha "$GIT_SHA" \
  --arg image_tag "$IMAGE_TAG" \
  --arg registry "$REGISTRY" \
  --arg image_provenance "$IMAGE_PROVENANCE" \
  --arg cluster_name "$PERSISTENT_CLUSTER_NAME" \
  --arg started_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  '{schema:$schema, project_id:$project_id, region:$region, gke_zone:$gke_zone, persistent_cluster:$cluster_name, run_id:$run_id, git_sha:$git_sha, git_dirty:false, image_tag:$image_tag, registry:$registry, image_provenance:$image_provenance, started_at:$started_at}' \
  > "$EVIDENCE_DIR/run.json"

if [[ "$acceptance_mode" == "lumen-auth" ]]; then
  echo ">> Terraform: auth-only Lumen resources on persistent Standard GKE"
else
  echo ">> Terraform: run-scoped backup bucket and workload identity on persistent Standard GKE"
fi
mkdir -p "$TERRAFORM_ENVIRONMENT_DIR"
cp "$ACCEPTANCE_ROOT/environment"/*.tf "$TERRAFORM_ENVIRONMENT_DIR/"
TF_DATA_DIR="$STATE_DIR/.terraform-environment" terraform \
  -chdir="$TERRAFORM_ENVIRONMENT_DIR" init -input=false
# Pass acceptance_apps unconditionally rather than appending to an array only in
# tape mode.  Under macOS bash 3.2 (`set -u`), expanding an *empty* array as
# "${arr[@]}" is an unbound-variable error, so the lumen-sift path — the default
# — aborted the whole run at terraform apply while the tape path, which always
# populated the array, kept passing (run 0726052225 died exactly here).  Naming
# the value outright removes the empty-array case instead of quoting around it,
# and keeps apply symmetric with cleanup.sh's destroy args.
if [[ "$acceptance_mode" == "tape" ]]; then
  terraform_acceptance_apps=tape
elif [[ "$acceptance_mode" == "sift" ]]; then
  terraform_acceptance_apps=sift
elif [[ "$acceptance_mode" == "lumen-auth" ]]; then
  terraform_acceptance_apps=lumen-auth
else
  terraform_acceptance_apps=lumen-sift
fi
require_acceptance_run_lock "Terraform apply"
TF_DATA_DIR="$STATE_DIR/.terraform-environment" terraform \
  -chdir="$TERRAFORM_ENVIRONMENT_DIR" apply \
  -state="$STATE_DIR/environment.tfstate" \
  -auto-approve \
  -var="project_id=$PROJECT_ID" \
  -var="region=$REGION" \
  -var="gke_zone=$GKE_ZONE" \
  -var="cluster_name=$PERSISTENT_CLUSTER_NAME" \
  -var="run_id=$RUN_ID" \
  -var="artifact_registry_repository=$ARTIFACT_REGISTRY_REPOSITORY" \
  -var="image_tag=$IMAGE_TAG" \
  -var="acceptance_apps=$terraform_acceptance_apps"
TF_DATA_DIR="$STATE_DIR/.terraform-environment" terraform \
  -chdir="$TERRAFORM_ENVIRONMENT_DIR" output \
  -state="$STATE_DIR/environment.tfstate" -json > "$EVIDENCE_DIR/terraform-output.json"

cluster="$(jq -r '.cluster_name.value' "$EVIDENCE_DIR/terraform-output.json")"
test "$(jq -r '.gke_zone.value' "$EVIDENCE_DIR/terraform-output.json")" = "$GKE_ZONE"
test "$(jq -r '.cluster_name.value' "$EVIDENCE_DIR/terraform-output.json")" = "$PERSISTENT_CLUSTER_NAME"
if [[ "$acceptance_mode" != "lumen-auth" ]]; then
  test "$(jq -r '.backup_bucket.value' "$EVIDENCE_DIR/terraform-output.json")" = "$BACKUP_BUCKET"
  test "$(jq -r '.backup_gsa_email.value' "$EVIDENCE_DIR/terraform-output.json")" = "$BACKUP_GSA_EMAIL"
fi
if [[ "$acceptance_mode" == "sift" ]]; then
  SIFT_NODE_POOL="$(jq -r '.sift_node_pool.value' "$EVIDENCE_DIR/terraform-output.json")"
  [[ "$SIFT_NODE_POOL" == "axo-${RUN_ID}-sift" ]] || {
    echo "Terraform did not create the exact run-scoped Sift node pool" >&2
    exit 1
  }
  export SIFT_NODE_POOL
fi
export EVIDENCE_DIR
export PROJECT_ID REGION BACKUP_BUCKET

if [[ "$acceptance_mode" == "sift" ]]; then
  require_acceptance_run_lock "Sift deploy"
  "$SCRIPT_DIR/deploy.sh" sift
  require_acceptance_run_lock "Sift operator verification"
  "$SCRIPT_DIR/verify-operator-cell.sh" sift
  require_acceptance_run_lock "Sift MVP verification"
  "$SCRIPT_DIR/verify-sift-mvp.sh"
elif [[ "$acceptance_mode" == "tape" ]]; then
  # Tape-only acceptance mode: a single disposable domain-plane cell, no
  # Lumen/Sift phasing.
  require_acceptance_run_lock "Tape deploy"
  "$SCRIPT_DIR/deploy.sh" tape
  require_acceptance_run_lock "Tape operator verification"
  "$SCRIPT_DIR/verify-operator-cell.sh" tape
  require_acceptance_run_lock "Tape verification"
  "$SCRIPT_DIR/verify-tape.sh"
elif [[ "$acceptance_mode" == "lumen-auth" ]]; then
  require_acceptance_run_lock "Lumen deploy"
  "$SCRIPT_DIR/deploy.sh" lumen
  require_acceptance_run_lock "Lumen operator verification"
  "$SCRIPT_DIR/verify-operator-cell.sh" lumen
  export LUMEN_AUTH_ACCEPTANCE_EVIDENCE="$EVIDENCE_DIR/lumen-auth-acceptance.json"
  require_acceptance_run_lock "Lumen auth verification"
  "$SCRIPT_DIR/verify-lumen-auth.sh"
  "$SCRIPT_DIR/finalize-lumen-acceptance.sh" lumen-auth
else
  # Phase 1 is a hard gate: no Sift CRD/operator/instance/collector is applied
  # until Lumen has independently reconciled, recovered, backed up to GCS, and
  # completed its bounded disk-triggered split.
  require_acceptance_run_lock "Lumen deploy"
  "$SCRIPT_DIR/deploy.sh" lumen
  require_acceptance_run_lock "Lumen operator verification"
  "$SCRIPT_DIR/verify-operator-cell.sh" lumen
  if [[ -n "$LUMEN_PRIOR_ACCEPTANCE" ]]; then
    cp "$LUMEN_PRIOR_ACCEPTANCE" "$EVIDENCE_DIR/lumen-acceptance-prior.json"
    export LUMEN_ACCEPTANCE_EVIDENCE="$EVIDENCE_DIR/lumen-acceptance-prior.json"
    export LUMEN_ACCEPTANCE_PROVENANCE="prior-gke-proof"
    echo ">> current Lumen operator cell passed; reusing supplied prior persistence, backup, and split proof"
  else
    require_acceptance_run_lock "Lumen verification"
    "$SCRIPT_DIR/verify-lumen.sh"
    export LUMEN_ACCEPTANCE_EVIDENCE="$EVIDENCE_DIR/lumen-acceptance.json"
    export LUMEN_ACCEPTANCE_PROVENANCE="current-run"
  fi

  # Request authorization is its own proof and runs on every pass, including
  # one reusing a prior persistence/backup/split proof: those legs all opt out
  # of auth, so nothing they established says anything about who may call.
  export LUMEN_AUTH_ACCEPTANCE_EVIDENCE="$EVIDENCE_DIR/lumen-auth-acceptance.json"
  require_acceptance_run_lock "Lumen auth verification"
  "$SCRIPT_DIR/verify-lumen-auth.sh"

  # Only a successful Lumen phase starts the Sift data plane. The collector then
  # reads Lumen's structured stdout from Standard GKE node logs and the proof
  # queries the materialized Sift logging store.
  require_acceptance_run_lock "Sift deploy"
  "$SCRIPT_DIR/deploy.sh" sift
  require_acceptance_run_lock "Sift operator verification"
  "$SCRIPT_DIR/verify-operator-cell.sh" sift
  require_acceptance_run_lock "Sift collection verification"
  "$SCRIPT_DIR/verify-sift-collection.sh"
fi

echo ">> acceptance passed; mandatory cleanup runs on EXIT"
run_completed=1
