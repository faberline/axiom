#!/usr/bin/env bash
set -euo pipefail

# HANDWRITE-BEGIN gap="missing-generator:unit-test:a87c6c67" tracker="2370" reason="The GKE harness needs a static regression oracle for acceptance-mode phase selection until shell-control-flow generation exists."
#
# This oracle replaces the LUMEN_ONLY one. That mode was deleted by the
# tape-mode refactor (ce6635f57a) and this file kept asserting it existed, so
# check.sh had been failing ever since -- and failing SILENTLY, because every
# assertion was a bare `rg ... >/dev/null` whose only output under `set -e` is
# the exit status. A gate nobody can read is a gate nobody runs. Every
# assertion below therefore names itself on failure.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ACCEPTANCE_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RUN_SCRIPT="$ACCEPTANCE_ROOT/scripts/run.sh"
RENDER_SCRIPT="$ACCEPTANCE_ROOT/scripts/render-manifests.sh"
CLEANUP_SCRIPT="$ACCEPTANCE_ROOT/scripts/cleanup.sh"
VERIFY_CLEAN_SCRIPT="$ACCEPTANCE_ROOT/scripts/verify-clean.sh"
CELL_SCRIPT="$ACCEPTANCE_ROOT/scripts/verify-operator-cell.sh"
DEPLOY_SCRIPT="$ACCEPTANCE_ROOT/scripts/deploy.sh"
BOOTSTRAP_SCRIPT="$ACCEPTANCE_ROOT/scripts/bootstrap-cluster.sh"
SIFT_VERIFY_SCRIPT="${SIFT_VERIFY_SCRIPT_OVERRIDE:-$ACCEPTANCE_ROOT/scripts/verify-sift-mvp.sh}"
SIFT_LOAD_DIGEST_SCRIPT="$ACCEPTANCE_ROOT/scripts/sift-load-digest.py"
SIFT_FINALIZER="$ACCEPTANCE_ROOT/scripts/finalize-sift-mvp-acceptance.sh"
SIFT_EVIDENCE_VALIDATOR="$ACCEPTANCE_ROOT/scripts/validate-sift-mvp-evidence.py"
SIFT_CANDIDATE_SCRIPT="$ACCEPTANCE_ROOT/scripts/sift-candidate.sh"
SIFT_PREPARE_SCRIPT="$ACCEPTANCE_ROOT/scripts/prepare-sift-candidate.sh"
SIFT_CANDIDATE_CLEANUP_SCRIPT="$ACCEPTANCE_ROOT/scripts/cleanup-sift-candidate.sh"
SIFT_CONTAINED_SCRIPT="$ACCEPTANCE_ROOT/scripts/run-sift-contained.sh"
SIFT_CONTAINER_BOUNDARY="$ACCEPTANCE_ROOT/scripts/sift-container-boundary.sh"
SIFT_ACCEPTANCE_RUNNER_IMAGE="${SIFT_ACCEPTANCE_RUNNER_IMAGE_OVERRIDE:-$ACCEPTANCE_ROOT/images/Dockerfile.sift-acceptance-runner}"
PROCESS_TREE_SCRIPT="$ACCEPTANCE_ROOT/scripts/process-tree.sh"
PROCESS_START_TOKEN_HELPER="$ACCEPTANCE_ROOT/scripts/process-start-token.py"
RUN_LOG_SCRIPT="$ACCEPTANCE_ROOT/scripts/run-log.sh"
RUN_SUPERVISOR="$ACCEPTANCE_ROOT/scripts/run-supervisor.py"
SOURCE_PREFIX_SCRIPT="$ACCEPTANCE_ROOT/scripts/source-prefix.sh"
ACCEPTANCE_LOCK_SCRIPT="$ACCEPTANCE_ROOT/scripts/acceptance-lock.sh"
KUBERNETES_OWNERSHIP_SCRIPT="$ACCEPTANCE_ROOT/scripts/kubernetes-ownership.sh"
SIFT_AUTH_DELEGATOR_FILTER="$ACCEPTANCE_ROOT/scripts/sift-auth-delegator.jq"
SIFT_ARCHIVE_FQDN_FILTER="$ACCEPTANCE_ROOT/scripts/sift-archive-fqdn-policy.jq"
ENV_VARIABLES="$ACCEPTANCE_ROOT/environment/variables.tf"
ENV_GKE="$ACCEPTANCE_ROOT/environment/gke.tf"
CLUSTER_TF="$ACCEPTANCE_ROOT/cluster/main.tf"
SCHEMA="$ACCEPTANCE_ROOT/evidence/schema.json"
SIFT_TEST_IMAGE_WORKFLOW="$ACCEPTANCE_ROOT/../../.github/workflows/sift-test-image.yml"
SIFT_COMPLIANCE_SCRIPT="$ACCEPTANCE_ROOT/../../apps/sift/e2e/prometheus_compliance.sh"
SIFT_CANDIDATE_ROOT_TEST="$ACCEPTANCE_ROOT/../../apps/sift/e2e/candidate_root.sh"

fail() {
  echo "acceptance-mode oracle: $1" >&2
  exit 1
}

# Both helpers ignore comment lines, and that is not pedantry: the first draft
# of this oracle used a bare substring match, so commenting out `trap cleanup
# EXIT` -- i.e. disarming the mandatory GCP teardown -- still satisfied it. An
# assertion a `#` can defeat is not an assertion.
uncommented() { # uncommented <pattern> <file>
  rg -F -- "$1" "$2" 2>/dev/null | rg -v '^\s*#' >/dev/null
}

present() { # present <label> <pattern> <file>
  uncommented "$2" "$3" || fail "$1 (expected live in ${3##*/}: $2)"
}

present_re() { # present_re <label> <anchored-regex> <file>
  rg -- "$2" "$3" 2>/dev/null | rg -v '^\s*#' >/dev/null \
    || fail "$1 (expected exact live code in ${3##*/}: $2)"
}

absent() { # absent <label> <pattern> <file>
  ! uncommented "$2" "$3" || fail "$1 (unexpectedly still live in ${3##*/}: $2)"
}

dockerfile_env_sets_sift_cli() { # dockerfile_env_sets_sift_cli <file>
  local result
  result="$(awk '
    function inspect() {
      if (tolower(logical) ~ /^[[:space:]]*env[[:space:]]/ && logical ~ /(^|[^[:alnum:]_])SIFT_CLI([^[:alnum:]_]|$)/) {
        found = 1
      }
      logical = ""
    }
    /^[[:space:]]*#/ { next }
    {
      current = $0
      continued = current ~ /\\[[:space:]]*$/
      sub(/\\[[:space:]]*$/, "", current)
      logical = logical " " current
      if (!continued) inspect()
    }
    END {
      if (length(logical) > 0) inspect()
      print found ? "yes" : "no"
    }
  ' "$1")" || fail "could not parse ${1##*/} ENV instructions"
  case "$result" in
    yes) return 0 ;;
    no) return 1 ;;
    *) fail "unexpected Dockerfile ENV scan result for ${1##*/}: $result" ;;
  esac
}

line_of() { # line_of <pattern> <file>
  rg -n -F -- "$1" "$2" | head -1 | cut -d: -f1
}

run_process_group_exit_race_child() {
  source "$PROCESS_TREE_SCRIPT"
  root_pid="$$"
  group_id=""
  local worker_pid
  watchdog_pid=""
  local watchdog_self
  scan_attempt=""
  records="${SIFT_PROCESS_GROUP_RACE_RECORDS:?}"
  watchdog_pid_file="${records}.watchdog-pid"
  local child_pid_file="${SIFT_PROCESS_GROUP_RACE_CHILD_PID:?}"
  group_id="$(process_group_id "$root_pid")"
  [[ "$group_id" == "$root_pid" ]] || return 1

  python3 -c '
import os
import signal
import subprocess
import time

def stop(_signum, _frame):
    child = subprocess.Popen(
        ["bash", "-c", "trap : TERM; while :; do sleep 30; done"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    with open(os.environ["SIFT_PROCESS_GROUP_RACE_CHILD_PID"], "w", encoding="utf-8") as output:
        output.write(str(child.pid))
    os._exit(0)

signal.signal(signal.SIGTERM, stop)
while True:
    time.sleep(30)
' >/dev/null 2>&1 &
  worker_pid="$!"

  process_group_race_cleanup() {
    trap - EXIT INT TERM
    kill "$watchdog_pid" >/dev/null 2>&1 || true
    wait "$watchdog_pid" >/dev/null 2>&1 || true
    for scan_attempt in 1 2 3 4 5; do
      append_process_group_members "$group_id" "$root_pid" "" "$records"
      signal_recorded_processes "$records" KILL
      [[ "$scan_attempt" == "5" ]] || sleep 1
    done
  }
  trap process_group_race_cleanup EXIT INT TERM

  (
    watchdog_self="$(wait_for_process_id_file "$watchdog_pid_file" "$root_pid")" \
      || exit 1
    record_process_group_members \
      "$group_id" "$root_pid" "$watchdog_self" "$records"
    signal_recorded_processes "$records" TERM
    sleep 10
    append_process_group_members \
      "$group_id" "$root_pid" "$watchdog_self" "$records"
    signal_recorded_processes "$records" KILL
  ) &
  watchdog_pid="$!"
  printf '%s\n' "$watchdog_pid" > "${watchdog_pid_file}.tmp"
  mv "${watchdog_pid_file}.tmp" "$watchdog_pid_file"

  wait "$worker_pid" || true
  [[ -s "$child_pid_file" ]]
}

if [[ "${SIFT_PROCESS_GROUP_EXIT_RACE_CHILD:-0}" == "1" ]]; then
  run_process_group_exit_race_child
  exit
fi

# Every script, by glob, not by name. This loop used to enumerate six of the
# twelve, which meant the three verify-*.sh scripts -- where the actual per-leg
# assertions live and where the file keeps growing -- were the only ones a
# syntax error could reach production in. A new script is covered the day it
# lands, not the day someone remembers to add it here.
shopt -s nullglob
acceptance_scripts=("$ACCEPTANCE_ROOT"/scripts/*.sh)
shopt -u nullglob
[[ ${#acceptance_scripts[@]} -ge 6 ]] || fail "found ${#acceptance_scripts[@]} acceptance scripts; the glob is not matching"
for script in "${acceptance_scripts[@]}"; do
  bash -n "$script" || fail "shell syntax error in ${script##*/}"
done
jq empty "$SCHEMA" || fail "evidence schema is not valid JSON"

# Start run.sh itself as a process-group leader. The outer wrapper must fork a
# new session child instead of calling setsid() in that leader and failing with
# EPERM. A missing PROJECT_ID is the first expected error after isolation.
if ! python3 -c '
import os
import subprocess
import sys

environment = os.environ.copy()
environment.pop("AXIOM_GCP_ACCEPTANCE_ISOLATED_SESSION", None)
environment.pop("PROJECT_ID", None)
result = subprocess.run(
    [sys.argv[1]],
    env=environment,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
    start_new_session=True,
)
if result.returncode == 0:
    raise SystemExit(1)
if "Set PROJECT_ID explicitly" not in result.stderr:
    sys.stderr.write(result.stderr)
    raise SystemExit(1)
' "$RUN_SCRIPT"; then
  fail "isolated-session wrapper failed when run.sh was already a process-group leader"
fi

# Prove the real EXIT-cleanup race. The watchdog sends TERM and starts its
# grace period. The foreground worker then exits, which starts main cleanup
# and kills the watchdog before its second scan. Cleanup must rescan the group
# itself and kill the reparented TERM-handler child.
if [[ "${SIFT_ORACLE_MUTATION_CHILD:-0}" != "1" ]] && ! (
  source "$PROCESS_TREE_SCRIPT"
  process_test_dir="$(mktemp -d "${TMPDIR:-/tmp}/sift-process-tree.XXXXXX")"
  process_test_records="$process_test_dir/records.txt"
  process_test_child_pid="$process_test_dir/child-pid.txt"
  process_test_alive() {
    local pid expected current
    while IFS=$'\t' read -r pid expected; do
      current="$(process_start_token "$pid")"
      [[ -n "$current" && "$current" == "$expected" ]] && return 0
    done < "$process_test_records"
    return 1
  }
  process_test_cleanup() {
    signal_recorded_processes "$process_test_records" KILL || true
    find "$process_test_dir" -type f -delete
    find "$process_test_dir" -depth -type d -empty -delete
  }
  trap process_test_cleanup EXIT INT TERM
  SIFT_PROCESS_GROUP_EXIT_RACE_CHILD=1 \
    SIFT_PROCESS_GROUP_RACE_RECORDS="$process_test_records" \
    SIFT_PROCESS_GROUP_RACE_CHILD_PID="$process_test_child_pid" \
    python3 -c '
import os
import subprocess
import sys

result = subprocess.run([sys.argv[1]], env=os.environ.copy(), start_new_session=True)
raise SystemExit(result.returncode)
' "$SCRIPT_DIR/acceptance_mode_selection.sh" || exit 1
  [[ -s "$process_test_child_pid" && -s "$process_test_records" ]] || exit 1
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    process_test_alive || exit 0
    sleep 1
  done
  exit 1
); then
  fail "process-group watchdog did not stop a reparented TERM-handler child"
fi

# --- the mode enum is closed, and both scripts close it the same way --------
# The two scripts branch independently. When they disagreed about which modes
# exist, the harness rendered one app's manifests and verified another's.
present "run.sh lost the lumen-sift mode"      '"lumen sift") acceptance_mode="lumen-sift" ;;' "$RUN_SCRIPT"
present "run.sh lost the lumen-auth mode"       '"lumen auth") acceptance_mode="lumen-auth" ;;' "$RUN_SCRIPT"
present "run.sh lost the tape mode"            '"tape") acceptance_mode="tape" ;;'             "$RUN_SCRIPT"
present "run.sh lost the sift-only MVP mode"   '"sift") acceptance_mode="sift" ;;'             "$RUN_SCRIPT"
present "run.sh accepts an unknown mode"       "lumen auth" "$RUN_SCRIPT"
present "render-manifests lost lumen modes"   '"lumen sift"|"lumen auth")' "$RENDER_SCRIPT"
present "render-manifests lost tape"           '"tape")'       "$RENDER_SCRIPT"
present "render-manifests lost sift-only mode" '"sift")'       "$RENDER_SCRIPT"
present "render-manifests accepts an unknown mode" "ACCEPTANCE_APPS must be 'lumen sift', 'lumen auth', 'sift', or 'tape'" "$RENDER_SCRIPT"
present "cleanup lost lumen-auth"              '"lumen auth") acceptance_mode="lumen-auth" ;;' "$CLEANUP_SCRIPT"
present "verify-clean lost lumen-auth"         '"lumen auth") acceptance_mode="lumen-auth" ;;' "$VERIFY_CLEAN_SCRIPT"
present "cleanup lost sift-only mode"          '"sift") acceptance_mode="sift" ;;' "$CLEANUP_SCRIPT"
present "verify-clean lost sift-only mode"     '"sift") acceptance_mode="sift" ;;' "$VERIFY_CLEAN_SCRIPT"
present "auth-only invokes finalizer"          'finalize-lumen-acceptance.sh" lumen-auth' "$RUN_SCRIPT"
absent  "the deleted LUMEN_ONLY mode came back in run.sh"    'LUMEN_ONLY' "$RUN_SCRIPT"
absent  "the deleted LUMEN_ONLY mode came back in cleanup"   'LUMEN_ONLY' "$CLEANUP_SCRIPT"

# --- cleanup is armed on every exit path -----------------------------------
# The standing requirement is that GCP resources are released whether the run
# passes or fails, so the trap and its completion sentinel are contract, not
# housekeeping. `run_completed` exists because a `set -u` expansion error
# aborts without updating $?, which made two real runs exit 0 mid-flight.
present "cleanup is no longer trapped on EXIT" 'trap cleanup EXIT'                  "$RUN_SCRIPT"
present "the interrupt trap is gone"           "trap 'exit 130' INT"                "$RUN_SCRIPT"
present "the cloud-time cap trap is gone"      '45-minute cloud acceptance cap reached' "$RUN_SCRIPT"
present "the acceptance run is no longer isolated in its own process group" \
  'start_new_session=True' "$RUN_SUPERVISOR"
present "the outer isolation wrapper no longer kills surviving group members" \
  'signal_group(signal.SIGKILL)' "$RUN_SUPERVISOR"
present "the outer isolation wrapper does not wait for cloud readiness" \
  'ready_state == "ready"' "$RUN_SUPERVISOR"
present "the outer isolation wrapper has no independent cloud deadline" \
  'run_deadline = now + args.deadline_seconds' "$RUN_SUPERVISOR"
present "the outer isolation wrapper has no bounded preflight" \
  'preflight_deadline = time.monotonic() + args.preflight_deadline_seconds' "$RUN_SUPERVISOR"
present "the outer isolation wrapper has no bounded cleanup grace" \
  'shutdown_deadline = now + args.shutdown_grace_seconds' "$RUN_SUPERVISOR"
present "run.sh no longer binds cloud readiness to a nonce" \
  'AXIOM_GCP_ACCEPTANCE_SUPERVISOR_READY_TOKEN' "$RUN_SCRIPT"
supervisor_ready_publish_line="$(line_of \
  'mv "${AXIOM_GCP_ACCEPTANCE_SUPERVISOR_READY_PATH}.tmp"' "$RUN_SCRIPT")"
watchdog_ready_publish_line="$(line_of \
  'mv "${watchdog_ready_file}.tmp" "$watchdog_ready_file"' "$RUN_SCRIPT")"
[[ -n "$supervisor_ready_publish_line" && -n "$watchdog_ready_publish_line" \
  && "$supervisor_ready_publish_line" -lt "$watchdog_ready_publish_line" ]] \
  || fail "inner watchdog readiness can publish before the outer nonce receipt"
watchdog_generation_check_line="$(line_of \
  'if process_generation_state "$watchdog_pid" "$watchdog_token"; then' "$RUN_SCRIPT")"
watchdog_ready_accept_line="$(line_of 'watchdog_ready=1' "$RUN_SCRIPT")"
[[ -n "$watchdog_generation_check_line" && -n "$watchdog_ready_accept_line" \
  && "$watchdog_generation_check_line" -lt "$watchdog_ready_accept_line" ]] \
  || fail "run.sh can accept readiness without a live exact watchdog generation"
present "run.sh no longer fixes the cleanup grace at fifteen minutes" \
  '--shutdown-grace-seconds 900' "$RUN_SCRIPT"
absent "the watchdog again depends on Bash 4 BASHPID" 'BASHPID' "$RUN_SCRIPT"
present "the cloud-time cap no longer stops descendant processes" \
  'signal_recorded_processes "$watchdog_descendants" KILL' "$RUN_SCRIPT"
[[ "$(rg -F -c 'append_process_group_members' "$RUN_SCRIPT")" -ge 3 ]] \
  || fail "the watchdog no longer refreshes process-generation records during the run and shutdown"
present "the cloud-time cap misses group members created during TERM grace" \
  'append_process_group_members' "$RUN_SCRIPT"
present "recovery no longer checks exact recorded process generations" \
  'recorded_processes_have_live_member' "$CLEANUP_SCRIPT"
present "recovery no longer checks the dedicated watchdog generation" \
  '"$STATE_DIR/watchdog-process.txt"' "$CLEANUP_SCRIPT"
present "recovery no longer checks the dedicated run-log generation" \
  '"$STATE_DIR/run-log-process.txt"' "$CLEANUP_SCRIPT"
present "process generations no longer use the high-resolution OS helper" \
  'PROCESS_START_TOKEN_HELPER' "$PROCESS_TREE_SCRIPT"
absent "process generations again use second-resolution ps output" \
  'ps -o lstart' "$PROCESS_TREE_SCRIPT"
present "an unreadable live process token no longer fails closed" \
  'cannot verify the live acceptance run process generation' "$CLEANUP_SCRIPT"
absent "recovery again trusts a bare numeric process group" \
  'process_group_has_live_member' "$CLEANUP_SCRIPT"
present "EXIT cleanup no longer rescans after stopping the watchdog" \
  'shutdown_process_group_members' "$RUN_SCRIPT"
present "EXIT cleanup no longer confirms each creator is stopped" \
  'wait_for_process_generation_stopped "$pid" "$token" 40' "$PROCESS_TREE_SCRIPT"
present "EXIT cleanup no longer reaches a frozen fixed point" \
  '[[ "$stable" == "1" ]] || return 1' "$PROCESS_TREE_SCRIPT"
present "EXIT cleanup no longer freezes historical escaped generations" \
  'done < "$output"' "$PROCESS_TREE_SCRIPT"
present "EXIT cleanup no longer verifies historical generations are gone" \
  '&& ! recorded_processes_have_live_member "$output"' "$PROCESS_TREE_SCRIPT"
present "a failed process scan no longer blocks destructive cleanup" \
  'process-group membership is incomplete; refusing destructive cleanup' "$RUN_SCRIPT"
present "cloud work can start before the first complete watchdog scan" \
  'watchdog could not complete the initial process-group scan' "$RUN_SCRIPT"
present "EXIT cleanup no longer kills the watchdog process tree" \
  'signal_recorded_processes "$watchdog_descendants" KILL' "$RUN_SCRIPT"
present "EXIT cleanup can hand off while a recorded child remains unverifiable" \
  'a process appeared in the final EXIT scan; refusing cleanup' "$RUN_SCRIPT"
present "the long-lived log reader is no longer excluded by exact process generation" \
  'process_record_is_excluded "$exclusion_record" "$pid" "$token"' "$PROCESS_TREE_SCRIPT"
present "the run no longer keeps tee alive through destructive cleanup" \
  'start_run_log "$EVIDENCE_DIR/run.log"' "$RUN_SCRIPT"
present "the shared run-log helper no longer records tee birth identity" \
  'RUN_LOG_TEE_TOKEN="$(process_start_token "$RUN_LOG_TEE_PID")"' "$RUN_LOG_SCRIPT"
present "the watchdog is no longer stopped by exact generation with a deadline" \
  'stop_process_generation_bounded "$watchdog_pid" "$watchdog_token"' "$RUN_SCRIPT"
present "the watchdog no longer records its birth token" \
  'watchdog_token="$(process_start_token "$watchdog_pid")"' "$RUN_SCRIPT"
absent "watchdog shutdown again waits without a deadline" \
  'wait "$watchdog_pid"' "$RUN_SCRIPT"
absent "watchdog shutdown again signals a bare PID" \
  'kill "$watchdog_pid"' "$RUN_SCRIPT"
present "the run-log sink no longer requires a nonce-bound completion receipt" \
  'RUN_LOG_RECEIPT_NONCE' "$RUN_LOG_SCRIPT"
absent "run-log shutdown again waits on a possibly reused bare PID" \
  'wait "$pid"' "$RUN_LOG_SCRIPT"
present "the run no longer closes and waits for tee after cleanup" \
  'finish_run_log' "$RUN_SCRIPT"
absent "run.sh again uses an untracked process-substitution tee" \
  'exec > >(tee -a "$EVIDENCE_DIR/run.log")' "$RUN_SCRIPT"
present "the false-green sentinel is gone"     'run_completed=1'                    "$RUN_SCRIPT"
present "cleanup no longer refuses ec=0 without the sentinel" 'run aborted before completion' "$RUN_SCRIPT"
present "the backup service account is no longer swept" 'wait_for_empty "backup service account"' "$VERIFY_CLEAN_SCRIPT"

# --- Sift MVP is independent, bounded, and evidence-complete --------------
present "sift-only mode does not invoke its verifier" 'verify-sift-mvp.sh"' "$RUN_SCRIPT"
present "sift-only mode lost the 90-minute cap" 'sift_cloud_cap=5400' "$RUN_SCRIPT"
present "sift-only mode no longer enters the contained controller" \
  'exec "$SCRIPT_DIR/run-sift-contained.sh" "$@"' "$RUN_SCRIPT"
present "the contained controller no longer requires one complete candidate" \
  'verify_sift_candidate_directory "$SIFT_CANDIDATE_DIR"' "$SIFT_CONTAINED_SCRIPT"
present "the contained controller can again mount the candidate directory writable through an overlapping run path" \
  'paths_overlap "$SIFT_CANDIDATE_DIR" "$writable_directory"' "$SIFT_CONTAINED_SCRIPT"
present "the contained controller no longer resolves prospective writable paths before creating them" \
  'canonicalize_without_creating "$STATE_DIR"' "$SIFT_CONTAINED_SCRIPT"
candidate_precreate_guard_line="$(line_of \
  'paths_overlap "$SIFT_CANDIDATE_DIR" "$planned_writable_directory"' \
  "$SIFT_CONTAINED_SCRIPT")"
writable_directory_create_line="$(line_of \
  'mkdir -p "$STATE_DIR" "$EVIDENCE_DIR" "$CONTAINMENT_DIR"' \
  "$SIFT_CONTAINED_SCRIPT")"
[[ "$candidate_precreate_guard_line" =~ ^[0-9]+$ \
  && "$writable_directory_create_line" =~ ^[0-9]+$ \
  && "$candidate_precreate_guard_line" -lt "$writable_directory_create_line" ]] \
  || fail "candidate overlap must be rejected before writable directories are created"
present "sift-only mode accepts a caller-supplied image or CLI again" \
  'contained Sift acceptance reads its CLI and images only from the candidate image' "$RUN_SCRIPT"
present "sift-only mode accepts a caller-supplied compliance binary again" \
  'INPUT_SIFT_BIN="${SIFT_BIN:-}"' "$RUN_SCRIPT"
present "candidate preparation no longer builds all three fixed images together" \
  'cloudbuild.sift-mvp.yaml' "$SIFT_PREPARE_SCRIPT"
present "Sift candidate source is no longer archived from the fixed Git commit" \
  'git -c core.fsmonitor=false -C "$REPO_ROOT" archive' "$SIFT_PREPARE_SCRIPT"
present "Sift Cloud Build again submits the mutable worktree" \
  'gcloud builds submit "$source_archive"' "$SIFT_PREPARE_SCRIPT"
present "the contained controller image no longer embeds the candidate Sift CLI" \
  'COPY --from=builder /out/sift /usr/local/bin/sift' \
  "$SIFT_ACCEPTANCE_RUNNER_IMAGE"
if dockerfile_env_sets_sift_cli "$SIFT_ACCEPTANCE_RUNNER_IMAGE"; then
  fail "the controller image exposes its embedded CLI as caller input"
fi
present "the contained run no longer selects the embedded CLI after candidate validation" \
  'SIFT_CLI="/usr/local/bin/sift"' "$RUN_SCRIPT"
input_sift_cli_line="$(line_of 'INPUT_SIFT_CLI="${SIFT_CLI:-}"' "$RUN_SCRIPT")"
candidate_identity_guard_line="$(line_of \
  'the candidate identity does not match this contained run' "$RUN_SCRIPT")"
fixed_sift_cli_line="$(line_of 'SIFT_CLI="/usr/local/bin/sift"' "$RUN_SCRIPT")"
[[ "$input_sift_cli_line" =~ ^[0-9]+$ \
  && "$candidate_identity_guard_line" =~ ^[0-9]+$ \
  && "$fixed_sift_cli_line" =~ ^[0-9]+$ \
  && "$input_sift_cli_line" -lt "$candidate_identity_guard_line" \
  && "$candidate_identity_guard_line" -lt "$fixed_sift_cli_line" ]] \
  || fail "embedded Sift CLI selection must follow caller capture and candidate validation"
present "Sift GCP acceptance no longer runs the complete fixed candidate gate" \
  'bash "$source_dir/apps/sift/test.sh" --candidate' "$SIFT_PREPARE_SCRIPT"
present "Sift GCP acceptance does not bind the gate to the extracted source root" \
  'SIFT_REPO_ROOT="$source_dir"' "$SIFT_PREPARE_SCRIPT"
present "Sift candidate compliance no longer verifies the built full SHA" \
  'SIFT_EXPECTED_SOURCE_REVISION="$candidate_revision"' \
  "$ACCEPTANCE_ROOT/../../apps/sift/test.sh"
present "Sift candidate compliance no longer compares the actual full SHA" \
  'if actual != expected:' "$SIFT_COMPLIANCE_SCRIPT"
present "the stale fixed-path candidate negative test is gone" \
  'Prometheus compliance accepted a stale fixed-path Sift binary' \
  "$SIFT_CANDIDATE_ROOT_TEST"
present "Sift GCP acceptance no longer records the candidate gate result" \
  'axiom.gcp.sift.candidate-gate.v1' "$SIFT_PREPARE_SCRIPT"
present "Sift verification no longer binds to the candidate gate receipt" \
  'candidate_gate_receipt="$EVIDENCE_DIR/candidate-gate.json"' "$SIFT_VERIFY_SCRIPT"
present "cleanup evidence no longer binds to the candidate gate receipt" \
  '--slurpfile gate "$candidate_component_dir/candidate-gate.json"' "$VERIFY_CLEAN_SCRIPT"
present "cleanup evidence no longer binds to the candidate source receipt" \
  '--slurpfile source "$candidate_component_dir/candidate-source.json"' "$VERIFY_CLEAN_SCRIPT"
present "cleanup evidence no longer binds to the candidate build receipt" \
  '--slurpfile build "$candidate_component_dir/cloud-build-source-binding.json"' "$VERIFY_CLEAN_SCRIPT"
present "cleanup evidence no longer binds to the candidate image receipt" \
  '--slurpfile images "$candidate_component_dir/images.json"' "$VERIFY_CLEAN_SCRIPT"
present "Sift Cloud Build no longer checks the uploaded source bytes" \
  'Cloud Build staged source does not match the candidate archive' "$SIFT_PREPARE_SCRIPT"
present "Sift Cloud Build receipts are no longer bound to the candidate SHA" \
  '.substitutions._GIT_SHA == $c.git_sha' "$SIFT_CANDIDATE_SCRIPT"
present "Sift Cloud Build receipts are no longer bound to the candidate source digest" \
  '.substitutions._SOURCE_BUNDLE_SHA256 == $c.source_bundle_sha256' "$SIFT_CANDIDATE_SCRIPT"
present "Sift Cloud Build receipts are no longer bound to the run" \
  '.substitutions._RUN_ID == $c.run_id' "$SIFT_CANDIDATE_SCRIPT"
present "Sift candidate image tags no longer include the random acquisition identity" \
  'IMAGE_TAG="${GIT_SHA}-${RUN_ID}-${acquisition_id}"' "$SIFT_PREPARE_SCRIPT"
present "Sift candidate validation no longer binds the image tag to the random acquisition identity" \
  '.image_tag == (.git_sha + "-" + .run_id + "-" + .acquisition_id)' \
  "$SIFT_CANDIDATE_SCRIPT"
present "candidate preparation no longer fails closed on source-bucket inventory errors" \
  'could not inventory the pre-existing Cloud Build source bucket' "$SIFT_PREPARE_SCRIPT"
present "candidate preparation no longer writes a run-scoped source-prefix receipt" \
  'write_source_prefix_receipt' "$SIFT_PREPARE_SCRIPT"
present "cleanup no longer verifies the source-prefix receipt before deletion" \
  'verify_source_prefix_receipt' "$CLEANUP_SCRIPT"
present "verify-clean no longer verifies the source-prefix receipt" \
  'verify_source_prefix_receipt' "$VERIFY_CLEAN_SCRIPT"
present "the source-prefix guard no longer requires the exact run path" \
  '/source/axiom-gcp-operator-' "$SOURCE_PREFIX_SCRIPT"
present "candidate preparation no longer binds the reported Cloud Build object to the exact prefix" \
  'validated_source_object_uri' "$SIFT_PREPARE_SCRIPT"
present "candidate failure no longer records whether submit returned a build ID" \
  'submit_response_received:$submit_response_received' "$SIFT_PREPARE_SCRIPT"
present "candidate failure no longer distinguishes a published submit intent from a started build" \
  'submit_intent_published:$submit_intent_published' "$SIFT_PREPARE_SCRIPT"
present "candidate failure no longer starts its receipt-driven recovery" \
  'bash "$SCRIPT_DIR/cleanup-sift-candidate.sh" "$recovery_dir"' \
  "$SIFT_PREPARE_SCRIPT"
present "candidate recovery no longer discovers a submit whose response was lost" \
  '--filter="tags=axiom-acquisition-${ACQUISITION_ID}"' \
  "$SIFT_CANDIDATE_CLEANUP_SCRIPT"
present "candidate recovery no longer verifies exact build substitutions" \
  '.substitutions._SOURCE_BUNDLE_SHA256 == $source_sha' \
  "$SIFT_CANDIDATE_CLEANUP_SCRIPT"
present "candidate recovery can delete a moved image tag again" \
  'candidate tag no longer matches its Cloud Build receipt' \
  "$SIFT_CANDIDATE_CLEANUP_SCRIPT"
present "candidate recovery no longer deletes exact source objects" \
  'gcloud storage rm "$source_uri" --quiet' \
  "$SIFT_CANDIDATE_CLEANUP_SCRIPT"
present "candidate recovery no longer deletes its reservation with a generation precondition" \
  '--if-generation-match="$reservation_generation"' \
  "$SIFT_CANDIDATE_CLEANUP_SCRIPT"
present "candidate recovery no longer deletes its submit intent with a generation precondition" \
  '--if-generation-match="$submit_intent_generation"' \
  "$SIFT_CANDIDATE_CLEANUP_SCRIPT"
present "cleanup no longer validates the exact Cloud Build source object" \
  'verify_cloud_build_source_evidence' "$CLEANUP_SCRIPT"
present "main cleanup no longer deletes the candidate reservation with a generation precondition" \
  '--if-generation-match="$reservation_generation"' "$CLEANUP_SCRIPT"
present "main cleanup no longer deletes the candidate submit intent with a generation precondition" \
  '--if-generation-match="$submit_intent_generation"' "$CLEANUP_SCRIPT"
present "the run no longer uses create-only Kubernetes Lease acquisition" \
  'kubectl create -f - -o json' "$RUN_SCRIPT"
present "the run no longer persists a provisional Lease intent" \
  'write_acceptance_lock_intent' "$RUN_SCRIPT"
present "the run no longer writes an exact shared GKE Lease receipt" \
  'write_acceptance_lock_receipt' "$RUN_SCRIPT"
present "the Sift preflight can again treat Kubernetes API failure as absence" \
  'require_kubernetes_resource_absent namespace "$namespace"' "$RUN_SCRIPT"
present "the Sift preflight no longer refuses an existing Sift CRD" \
  'customresourcedefinition sifts.sift.axiom.dev' "$RUN_SCRIPT"
present "Sift deploy no longer creates namespaces with ownership receipts" \
  'create_owned_namespace' "$DEPLOY_SCRIPT"
present "Sift deploy no longer creates its CRD with an ownership receipt" \
  'create_owned_kubernetes_resource' "$DEPLOY_SCRIPT"
present "fresh-PVC restore no longer owns its namespace by UID" \
  'create_owned_namespace' "$SIFT_VERIFY_SCRIPT"
present "cleanup no longer deletes Sift namespaces through UID receipts" \
  'delete_owned_kubernetes_resource' "$CLEANUP_SCRIPT"
present "Kubernetes ownership no longer persists intent before create" \
  'write_kubernetes_ownership_intent' "$KUBERNETES_OWNERSHIP_SCRIPT"
present "Kubernetes ownership no longer binds cleanup to the live UID" \
  'verify_kubernetes_ownership_receipt' "$KUBERNETES_OWNERSHIP_SCRIPT"
present "cleanup no longer verifies Sift ownership before deleting a CR" \
  'assert_owned_kubernetes_resource' "$CLEANUP_SCRIPT"
present "Kubernetes cleanup no longer uses UID/resourceVersion preconditions" \
  'preconditions:{uid:$uid,resourceVersion:$resource_version}' \
  "$KUBERNETES_OWNERSHIP_SCRIPT"
absent "Sift cleanup again deletes the fixed CRD without a UID precondition" \
  'kubectl delete customresourcedefinition sifts.sift.axiom.dev' "$CLEANUP_SCRIPT"
ownership_assert_line="$(line_of 'assert_owned_kubernetes_resource' "$CLEANUP_SCRIPT")"
sift_instance_delete_line="$(line_of 'delete_sift_instance sift sift' "$CLEANUP_SCRIPT")"
[[ "$ownership_assert_line" =~ ^[0-9]+$ \
  && "$sift_instance_delete_line" =~ ^[0-9]+$ \
  && "$ownership_assert_line" -lt "$sift_instance_delete_line" ]] \
  || fail "Sift CR deletion can run before namespace and CRD ownership checks"
present "the run no longer derives one host-wide claim for this project, run, and mode" \
  'acceptance_run_claim_path' "$RUN_SCRIPT"
present "the run no longer installs its complete local claim atomically" \
  'ln "$claim_candidate" "$local_run_claim"' "$RUN_SCRIPT"
present "the run no longer binds cleanup to its local owner identity" \
  'write_acceptance_run_owner' "$RUN_SCRIPT"
present "the EXIT trap no longer passes its one-time cleanup handoff nonce" \
  'ACCEPTANCE_RUN_OWNER_HANDOFF_NONCE="$cleanup_handoff_nonce"' "$RUN_SCRIPT"
present "the local owner receipt no longer stores only the handoff nonce digest" \
  '.cleanup_handoff_digest == $cleanup_handoff_digest' "$ACCEPTANCE_LOCK_SCRIPT"
absent "the public acquisition ID is again accepted as a cleanup handoff" \
  'ACCEPTANCE_RUN_OWNER_ACQUISITION_ID=' "$CLEANUP_SCRIPT"
present "cleanup no longer rejects a live run owner" \
  'authorize_acceptance_cleanup || exit 1' "$CLEANUP_SCRIPT"
present "cleanup no longer verifies the shared GKE Lease before Kubernetes deletion" \
  'verify_acceptance_lock_receipt' "$CLEANUP_SCRIPT"
present "cleanup no longer gates Kubernetes deletion on the verified shared GKE Lease" \
  'if [[ "$kubernetes_cleanup_authorized" == "1" ]]' "$CLEANUP_SCRIPT"
present "cleanup no longer deletes the shared Lease with UID and resourceVersion preconditions" \
  '"$raw_path" "$uid" "$resource_version"; then' "$CLEANUP_SCRIPT"
present "the shared GKE Lease receipt no longer binds the Kubernetes UID" \
  '.uid == $uid' "$ACCEPTANCE_LOCK_SCRIPT"
lock_create_line="$(line_of 'kubectl create -f - -o json' "$RUN_SCRIPT")"
lock_intent_line="$(line_of 'write_acceptance_lock_intent' "$RUN_SCRIPT")"
run_owner_line="$(line_of 'write_acceptance_run_owner' "$RUN_SCRIPT")"
atomic_claim_line="$(line_of 'ln "$claim_candidate" "$local_run_claim"' "$RUN_SCRIPT")"
cleanup_armed_line="$(line_of 'cleanup_armed=1' "$RUN_SCRIPT")"
cloud_submit_line="$(line_of 'build_id="$(gcloud builds submit' "$RUN_SCRIPT")"
terraform_apply_line="$(line_of '-chdir="$TERRAFORM_ENVIRONMENT_DIR" apply' "$RUN_SCRIPT")"
candidate_copy_line="$(line_of 'copy_sift_candidate_evidence' "$RUN_SCRIPT")"
[[ "$lock_create_line" =~ ^[0-9]+$ && "$lock_intent_line" =~ ^[0-9]+$ \
  && "$run_owner_line" =~ ^[0-9]+$ && "$atomic_claim_line" =~ ^[0-9]+$ \
  && "$cleanup_armed_line" =~ ^[0-9]+$ && "$cloud_submit_line" =~ ^[0-9]+$ \
  && "$terraform_apply_line" =~ ^[0-9]+$ && "$candidate_copy_line" =~ ^[0-9]+$ \
  && "$run_owner_line" -lt "$atomic_claim_line" \
  && "$atomic_claim_line" -lt "$lock_intent_line" \
  && "$candidate_copy_line" -lt "$lock_intent_line" \
  && "$lock_intent_line" -lt "$lock_create_line" \
  && "$cleanup_armed_line" -lt "$lock_create_line" \
  && "$lock_create_line" -lt "$cloud_submit_line" \
  && "$lock_create_line" -lt "$terraform_apply_line" ]] \
  || fail "the Lease intent and cleanup trap must precede acquisition, Cloud Build, and run Terraform"
present "contained Sift run no longer separates live registry inventory from immutable candidate evidence" \
  'artifact_registry_receipt="$EVIDENCE_DIR/live-artifact-registry.json"' "$RUN_SCRIPT"
present "cleanup no longer passes the immutable candidate directory to its verifier" \
  'SIFT_CANDIDATE_DIR="$source_candidate_directory"' "$CLEANUP_SCRIPT"
present "final cleanup no longer prefers the immutable candidate directory" \
  'sift_candidate_verification_dir="$SIFT_CANDIDATE_DIR"' "$VERIFY_CLEAN_SCRIPT"
prepare_gate_line="$(line_of 'bash "$source_dir/apps/sift/test.sh" --candidate' "$SIFT_PREPARE_SCRIPT")"
prepare_reservation_line="$(line_of '"$receipts/candidate-reservation.json" "$reservation_uri" reservation || exit 1' "$SIFT_PREPARE_SCRIPT")"
prepare_intent_line="$(line_of '"$receipts/candidate-submit-intent.json" "$submit_intent_uri" submit-intent' "$SIFT_PREPARE_SCRIPT")"
prepare_submit_line="$(line_of 'if ! gcloud builds submit "$source_archive"' "$SIFT_PREPARE_SCRIPT")"
[[ "$prepare_gate_line" =~ ^[0-9]+$ \
  && "$prepare_reservation_line" =~ ^[0-9]+$ \
  && "$prepare_intent_line" =~ ^[0-9]+$ \
  && "$prepare_submit_line" =~ ^[0-9]+$ \
  && "$prepare_gate_line" -lt "$prepare_reservation_line" \
  && "$prepare_reservation_line" -lt "$prepare_intent_line" \
  && "$prepare_intent_line" -lt "$prepare_submit_line" ]] \
  || fail "Sift candidate gate, reservation, submit intent, and Cloud Build are out of order"
present "candidate reservation and submit intent are no longer create-only" \
  '--if-generation-match=0' "$SIFT_PREPARE_SCRIPT"
present "candidate preparation no longer claims the final directory atomically" \
  'if ! mkdir "$CANDIDATE_DIR"; then' "$SIFT_PREPARE_SCRIPT"
absent "candidate preparation again moves receipts into an unclaimed final directory" \
  'mv "$receipts" "$CANDIDATE_DIR"' "$SIFT_PREPARE_SCRIPT"
cloud_guard_line="$(line_of 'require_acceptance_run_lock "Cloud Build submit"' "$RUN_SCRIPT")"
terraform_guard_line="$(line_of 'require_acceptance_run_lock "Terraform apply"' "$RUN_SCRIPT")"
sift_deploy_guard_line="$(line_of 'require_acceptance_run_lock "Sift deploy"' "$RUN_SCRIPT")"
sift_deploy_line="$(line_of '"$SCRIPT_DIR/deploy.sh" sift' "$RUN_SCRIPT")"
[[ "$cloud_guard_line" =~ ^[0-9]+$ && "$terraform_guard_line" =~ ^[0-9]+$ \
  && "$sift_deploy_guard_line" =~ ^[0-9]+$ && "$sift_deploy_line" =~ ^[0-9]+$ \
  && "$cloud_guard_line" -lt "$cloud_submit_line" \
  && "$terraform_guard_line" -lt "$terraform_apply_line" \
  && "$sift_deploy_guard_line" -lt "$sift_deploy_line" ]] \
  || fail "run mutations must revalidate the exact live Lease before Cloud Build, Terraform, and deploy"
present "the run cannot recover an accepted Lease after a lost create response" \
  'verify_acceptance_lock_json' "$RUN_SCRIPT"
cleanup_lock_line="$(line_of 'acceptance_lock_receipt="$EVIDENCE_DIR/acceptance-lock.json"' "$CLEANUP_SCRIPT")"
cleanup_session_line="$(line_of 'acceptance_cleanup_session_patch' "$CLEANUP_SCRIPT")"
cleanup_build_line="$(line_of '&& ! stop_run_cloud_builds; then' "$CLEANUP_SCRIPT")"
[[ "$cleanup_lock_line" =~ ^[0-9]+$ && "$cleanup_session_line" =~ ^[0-9]+$ \
  && "$cleanup_build_line" =~ ^[0-9]+$ \
  && "$cleanup_lock_line" -lt "$cleanup_session_line" \
  && "$cleanup_session_line" -lt "$cleanup_build_line" ]] \
  || fail "cleanup must claim a unique session before touching run-tagged Cloud Builds"
present "cleanup no longer rechecks its session before destructive phases" \
  'assert_acceptance_cleanup_session' "$CLEANUP_SCRIPT"
present "cleanup cannot recover the patch-before-receipt window from durable intent" \
  'verify_acceptance_cleanup_session_intent_identity' "$CLEANUP_SCRIPT"
present "cleanup no longer uses a resourceVersion CAS for dead-owner takeover" \
  'acceptance_cleanup_session_takeover_patch' "$CLEANUP_SCRIPT"
present "cleanup no longer reports a dead-owner session takeover" \
  'took over the cleanup session after its recorded owner stopped' "$CLEANUP_SCRIPT"
present "cleanup can again require a candidate receipt after an early failed build" \
  '&& -f "$EVIDENCE_DIR/sift-mvp-verification.json"' "$VERIFY_CLEAN_SCRIPT"
present "verify-clean no longer checks the exact Cloud Build source object" \
  'verify_cloud_build_source_evidence' "$VERIFY_CLEAN_SCRIPT"
present "Sift Cloud Build receipts no longer bind the deployed image digests" \
  'and .digest == ($c.sift_image | split("@")[-1])' "$SIFT_CANDIDATE_SCRIPT"
present "Sift test-image workflow no longer resolves one immutable commit" \
  'sha: ${{ steps.source.outputs.sha }}' "$SIFT_TEST_IMAGE_WORKFLOW"
present "Sift test-image workflow publishes without the candidate gate" \
  'run: bash apps/sift/test.sh --candidate' "$SIFT_TEST_IMAGE_WORKFLOW"
present "Sift test-image binaries no longer receive the immutable full SHA" \
  'SIFT_SOURCE_REVISION: ${{ needs.resolve.outputs.sha }}' "$SIFT_TEST_IMAGE_WORKFLOW"
# build-stamp's explicit archive source revision is build-stamp's own contract
# now, pinned by the faberline/core tag: scripts/faberline-core-test.sh build-stamp.
[[ "$(rg -F -c 'ref: ${{ needs.resolve.outputs.sha }}' "$SIFT_TEST_IMAGE_WORKFLOW")" == "3" ]] \
  || fail "Sift test-image candidate, build, and publish jobs do not use the same resolved commit"
candidate_digest_line="$(line_of 'SIFT_IMAGE="$(jq -er' "$RUN_SCRIPT")"
gke_bootstrap_line="$(line_of 'echo ">> persistent Standard GKE cluster bootstrap or reuse"' "$RUN_SCRIPT")"
[[ -n "$candidate_digest_line" && -n "$gke_bootstrap_line" \
  && "$candidate_digest_line" -lt "$gke_bootstrap_line" ]] \
  || fail "GKE starts before the candidate receipt fixes all image digests"
owner_receipt_line="$(line_of 'write_sift_container_owner' "$SIFT_CONTAINED_SCRIPT")"
run_start_line="$(line_of 'docker start --attach "$run_container_id"' "$SIFT_CONTAINED_SCRIPT")"
stopped_receipt_line="$(line_of 'write_sift_container_stopped_receipt' "$SIFT_CONTAINED_SCRIPT")"
cleanup_container_line="$(line_of 'cleanup_container_id="$(docker create' "$SIFT_CONTAINED_SCRIPT")"
[[ "$owner_receipt_line" =~ ^[0-9]+$ && "$run_start_line" =~ ^[0-9]+$ \
  && "$stopped_receipt_line" =~ ^[0-9]+$ && "$cleanup_container_line" =~ ^[0-9]+$ \
  && "$owner_receipt_line" -lt "$run_start_line" \
  && "$run_start_line" -lt "$stopped_receipt_line" \
  && "$stopped_receipt_line" -lt "$cleanup_container_line" ]] \
  || fail "Sift cleanup can start before the exact controller container is proven stopped"
present "the Sift controller is no longer read-only" '--read-only' "$SIFT_CONTAINED_SCRIPT"
present "the Sift controller keeps Linux capabilities" '--cap-drop=ALL' "$SIFT_CONTAINED_SCRIPT"
present "the Sift controller can gain privileges" '--security-opt=no-new-privileges' "$SIFT_CONTAINED_SCRIPT"
present "the Sift controller image again runs as root" 'USER 65532:65532' "$SIFT_ACCEPTANCE_RUNNER_IMAGE"
absent "the Sift controller can again control its Docker host" '/var/run/docker.sock' "$SIFT_CONTAINED_SCRIPT"
present "the Sift controller lost exact interrupted-run recovery" \
  'run-sift-contained.sh [--recover]' "$SIFT_CONTAINED_SCRIPT"
present "an interrupted cleanup container is no longer recorded before start" \
  'write_sift_cleanup_container_owner' "$SIFT_CONTAINED_SCRIPT"
present "cleanup-container recovery no longer validates the exact prior owner" \
  'verify_sift_cleanup_container_owner' "$SIFT_CONTAINED_SCRIPT"
present "cleanup no longer discovers every run-tagged Cloud Build" \
  '--filter="tags=axiom-run-${RUN_ID}"' "$CLEANUP_SCRIPT"
present "cleanup no longer binds Sift builds to the candidate acquisition" \
  '--filter="tags=axiom-acquisition-${candidate_acquisition_id}"' "$CLEANUP_SCRIPT"
present "cleanup no longer rechecks the live candidate control objects" \
  'verify_live_sift_candidate_control_objects' "$CLEANUP_SCRIPT"
present "cleanup no longer rechecks the full candidate build before source deletion" \
  'verify_current_sift_candidate_build_inventory' "$CLEANUP_SCRIPT"
present "cleanup no longer runs a receipt-free check before deleting candidate controls" \
  'VERIFY_CLEAN_WRITE_RECEIPT=0' "$CLEANUP_SCRIPT"
present "verify-clean can produce a receipt while candidate controls remain" \
  'candidate control objects cannot produce a clean receipt' "$VERIFY_CLEAN_SCRIPT"
present "cleanup no longer waits for Cloud Build cancellation" \
  'Cloud Build $build_id did not reach a terminal state after cancellation' "$CLEANUP_SCRIPT"
present "verify-clean no longer rejects active run-tagged Cloud Builds" \
  'a run-tagged Cloud Build is still active or has an unknown status' "$VERIFY_CLEAN_SCRIPT"
present "verify-clean no longer validates the final candidate build resource" \
  'verify_sift_candidate_build_receipt' "$VERIFY_CLEAN_SCRIPT"
present_re "the Sift verifier accepts a mutable Sift image" \
  '^\[\[ "\$SIFT_IMAGE" == \*@sha256:\* \]\] \|\| \{$' "$SIFT_VERIFY_SCRIPT"
present_re "the Sift verifier accepts a mutable Rig image" \
  '^\[\[ "\$RIG_IMAGE" == \*@sha256:\* \]\] \|\| \{$' "$SIFT_VERIFY_SCRIPT"
present_re "the Sift verifier accepts a mutable controller image" \
  '^\[\[ "\$ACCEPTANCE_RUNNER_IMAGE" == \*@sha256:\* \]\] \|\| \{$' "$SIFT_VERIFY_SCRIPT"
present "terminal candidate evidence lost its source bundle digest" \
  'source_bundle_sha256:$source_bundle_sha256' "$SIFT_VERIFY_SCRIPT"
present "terminal candidate evidence lost its Cloud Build id" \
  'cloud_build_id:$cloud_build_id' "$SIFT_VERIFY_SCRIPT"
present "terminal candidate evidence lost its staged source object" \
  'source_object_uri:$source_object_uri' "$SIFT_VERIFY_SCRIPT"
present "terminal candidate evidence lost the immutable controller image" \
  'acceptance_runner_image:$acceptance_runner_image' "$SIFT_VERIFY_SCRIPT"
present "the paired Sift build lost its source digest tag" \
  'axiom-source-${_SOURCE_BUNDLE_SHA256}' "$ACCEPTANCE_ROOT/cloudbuild.sift-mvp.yaml"
present "the paired Sift build lost its unique acquisition tag" \
  'axiom-acquisition-${_CANDIDATE_ACQUISITION_ID}' \
  "$ACCEPTANCE_ROOT/cloudbuild.sift-mvp.yaml"
present "the paired Sift build no longer publishes the controller image" \
  '${_REGISTRY}/sift-acceptance-runner:${_TAG}' "$ACCEPTANCE_ROOT/cloudbuild.sift-mvp.yaml"
present "cleanup no longer removes the run-scoped controller image" \
  'delete_run_image sift-acceptance-runner' "$CLEANUP_SCRIPT"
present "cleanup no longer deletes images by immutable digest" \
  'gcloud artifacts docker images delete "$REGISTRY/$image@$digest"' \
  "$CLEANUP_SCRIPT"
present "cleanup no longer removes only its exact run image tag first" \
  'gcloud artifacts docker tags delete' "$CLEANUP_SCRIPT"
absent "cleanup again deletes all tags attached to a digest" \
  '--delete-tags' "$CLEANUP_SCRIPT"
image_delete_line="$(line_of 'if ! delete_run_image sift; then' "$CLEANUP_SCRIPT")"
source_delete_line="$(line_of 'gcloud storage rm "$source_uri" --quiet' "$CLEANUP_SCRIPT")"
[[ "$image_delete_line" =~ ^[0-9]+$ && "$source_delete_line" =~ ^[0-9]+$ \
  && "$image_delete_line" -lt "$source_delete_line" ]] \
  || fail "staged source can be deleted before candidate image cleanup finishes"
present "verify-clean no longer rejects a leftover controller image" \
  'sift-acceptance-runner:$IMAGE_TAG' "$VERIFY_CLEAN_SCRIPT"
present "Terraform lost the sift-only enum" '"sift"' "$ENV_VARIABLES"
present "Terraform lost the run-scoped Sift node pool" 'resource "google_container_node_pool" "sift_mvp"' "$ENV_GKE"
rg -q '^\s*disk_type\s*=\s*"pd-standard"\s*$' "$ENV_GKE" \
  || fail "the Sift node pool boot disks consume the PVC SSD quota"
present_re "the Sift verifier lost its 30-minute load" \
  '^LOAD_SECONDS=1800$' "$SIFT_VERIFY_SCRIPT"
present_re "the Sift verifier lost the 10k/s rate" \
  '^ITEMS_PER_SECOND=10000$' "$SIFT_VERIFY_SCRIPT"
present_re "the Sift verifier lost its 18M exact count" \
  '^EXPECTED_ITEMS=18000000$' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer requires the run-scoped node-pool name" \
  'SIFT_NODE_POOL is required' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer validates the run-scoped node-pool shape" \
  'sift-node-pool.json' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer checks actual pod node placement" \
  'verify_pods_on_run_nodes() {' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer requires all eleven primary role pods" \
  "'app.kubernetes.io/name=sift' sift-topology 11" "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer requires all eleven restored role pods" \
  "'app.kubernetes.io/name=sift' sift-restore-topology 11" "$SIFT_VERIFY_SCRIPT"
absent "the gRPC probe escaped to the shared acceptance pool" \
  'cloud.google.com/gke-nodepool: acceptance-pool' "$SIFT_VERIFY_SCRIPT"
absent "the rendered Sift CR can coerce a numeric-looking run ID" \
  'axiom-run-id: ${RUN_ID}' "$RENDER_SCRIPT"
absent "a Sift verification manifest can coerce a numeric-looking run ID" \
  'axiom-run-id: ${RUN_ID}' "$SIFT_VERIFY_SCRIPT"
present "the rendered Sift CR quotes its run ID" \
  'axiom-run-id: "${RUN_ID}"' "$RENDER_SCRIPT"
present "Sift verification manifests quote their run IDs" \
  'axiom-run-id: "${RUN_ID}"' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier lost GCS outage testing" 'archive-iam-disabled' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier does not timestamp the real GCS outage" \
  'archive_outage_started_at=' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier does not restrict logs to the real outage" \
  '--since-time="$archive_outage_started_at"' "$SIFT_VERIFY_SCRIPT"
absent "the Sift verifier can reuse a stale archive failure" \
  '--since=7m' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier lost fresh-PVC restore" 'fresh-pvc-restore' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer requires archive V10" \
  '.format_version == 10' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer checks the paged archive catalog" \
  '.catalog_root.entry_count == (.segment_count + .blob_count + .dedupe_receipt_count)' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer requires archived segments" \
  '.segment_count > 0' "$SIFT_VERIFY_SCRIPT"
absent "the Sift verifier still expects the removed inline segment array" \
  '(.segments | length) > 0' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer compares every voter after failover" \
  'wait_for_store_convergence' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer checks bounded resident Raft logs" \
  '.resident_log_bytes <= 536870912' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer checks every restored voter" \
  'restore_voters_converged=true' "$SIFT_VERIFY_SCRIPT"
absent "the Sift verifier emits terminal evidence before cleanup" \
  'acceptance.json' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier lost pre-cleanup evidence" \
  'sift-mvp-verification.json' "$SIFT_VERIFY_SCRIPT"
present "cleanup no longer finalizes Sift evidence after verify-clean" \
  'sift-mvp-verification.json' "$CLEANUP_SCRIPT"
present "cleanup no longer requires a clean cleanup receipt" \
  '$cleanup[0].status != "clean"' "$SIFT_FINALIZER"
present "cleanup evidence is no longer bound to the verified candidate" \
  '$cleanup[0].candidate != .acceptance.sift.candidate' "$SIFT_FINALIZER"
present "verify-clean no longer reconstructs the immutable candidate receipt" \
  'candidate cleanup receipt inputs do not describe one immutable build' "$VERIFY_CLEAN_SCRIPT"
absent "cleanup again deletes every Sift auth-delegator binding in the shared cluster" \
  '-l app.kubernetes.io/component=auth-delegation,app.kubernetes.io/name=sift' "$CLEANUP_SCRIPT"
absent "verify-clean again requires every Sift auth-delegator binding to disappear" \
  '-l app.kubernetes.io/component=auth-delegation,app.kubernetes.io/name=sift' "$VERIFY_CLEAN_SCRIPT"
present "verify-clean lost the primary acceptance auth binding check" \
  'sift.sift.sift.auth-delegator' "$VERIFY_CLEAN_SCRIPT"
present "verify-clean lost the restore acceptance auth binding check" \
  'sift.sift-restore.sift-restore.auth-delegator' "$VERIFY_CLEAN_SCRIPT"
present "cleanup no longer invokes the Sift evidence finalizer" \
  'finalize-sift-mvp-acceptance.sh' "$CLEANUP_SCRIPT"
present "the finalizer no longer creates terminal Sift evidence" \
  'mv "$output_tmp" "$output"' "$SIFT_FINALIZER"
[[ -f "$SIFT_EVIDENCE_VALIDATOR" ]] \
  || fail "the full Sift evidence schema validator is missing"
present "the verifier no longer validates pre-cleanup evidence" \
  'validate-sift-mvp-evidence.py' "$SIFT_VERIFY_SCRIPT"
present "the finalizer no longer validates terminal evidence" \
  'validate-sift-mvp-evidence.py' "$SIFT_FINALIZER"
present "cleanup queries can again hide API failures as empty results" \
  'inventory_output() {' "$VERIFY_CLEAN_SCRIPT"
present "cleanup no longer verifies newly deleted image digests" \
  'deleted-image-*.txt' "$VERIFY_CLEAN_SCRIPT"
present "cleanup no longer verifies the live tag digest before deletion" \
  'live image tag does not match the immutable receipt' "$CLEANUP_SCRIPT"
present "cleanup no longer rechecks the tag immediately before deletion" \
  'run image tag changed immediately before deletion' "$CLEANUP_SCRIPT"
present "cleanup no longer detects a tag move after exact tag deletion" \
  'run image tag still exists after exact tag deletion' "$CLEANUP_SCRIPT"
present "the Sift verifier lost its authenticated non-2xx status helper" \
  'auth_curl_status() {' "$SIFT_VERIFY_SCRIPT"
present "Remote Write 2.0 rejection again aborts before its 415 assertion" \
  'remote_write_v2_status="$(auth_curl_status ' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer executes Prometheus query_range" \
  '/prometheus/api/v1/query_range' "$SIFT_VERIFY_SCRIPT"
present "terminal evidence no longer records Prometheus query_range" \
  'prometheus_query_range:"passed"' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier lost its same-ID retry probe" \
  'smoke-idempotency' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer proves retry count and digest stability" \
  'idempotency retry changed durable project identity' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier lost delayed idempotency retries" \
  'verify_idempotency_retry after-steady-load' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier lost failover idempotency retries" \
  'verify_idempotency_retry after-vm-failover' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer retries every signal" \
  'for signal in logs metrics traces; do' "$SIFT_VERIFY_SCRIPT"
present "the Sift load no longer rejects OTLP partial-success bodies" \
  '"$.partialSuccess" = "absent"' "$SIFT_VERIFY_SCRIPT"
present "the steady phase lost its independent event-ID digest" \
  'verify_load_digest steady "$LOAD_SECONDS"' "$SIFT_VERIFY_SCRIPT"
present "the failover phase lost its independent event-ID digest" \
  'verify_load_digest failover "$FAILOVER_SECONDS"' "$SIFT_VERIFY_SCRIPT"
present "steady and failover trace IDs no longer use separate prefixes" \
  'trace_prefix=57ead0000000' "$SIFT_VERIFY_SCRIPT"
present "failover trace IDs lost their phase prefix" \
  'trace_prefix=fa1100000000' "$SIFT_VERIFY_SCRIPT"
present "latency query samples no longer validate response bodies" \
  'query-response-${sample}.json' "$SIFT_VERIFY_SCRIPT"
present "latency trace samples no longer validate response bodies" \
  'trace-response-${sample}.json' "$SIFT_VERIFY_SCRIPT"
present "the verifier lost the authorized same-ID project proof" \
  'cross-project-same-id' "$SIFT_VERIFY_SCRIPT"
present "the acceptance Role no longer authorizes the second test project" \
  'resourceNames: ["sift-mvp", "sift-mvp-alt"]' "$RENDER_SCRIPT"
present_re "the GCS outage no longer invokes quorum recovery" \
  '^verify_outage_quorum_recovery "\$archive_leader" \\$' "$SIFT_VERIFY_SCRIPT"
present "the GCS outage no longer restarts the current leader" \
  'pod/sift-store-${stopped_leader}' "$SIFT_VERIFY_SCRIPT"
present_re "the failover leg no longer invokes its pod identity comparison" \
  '^assert_failover_restart_evidence [\\]$' "$SIFT_VERIFY_SCRIPT"
present_re "the peer mTLS negative gate is no longer invoked" \
  '^verify_peer_mtls_rejection$' "$SIFT_VERIFY_SCRIPT"
present "peer mTLS no longer rejects clients without a certificate" \
  'peer-mtls-no-client.stderr' "$SIFT_VERIFY_SCRIPT"
present "peer mTLS no longer rejects an untrusted CA" \
  'peer-mtls-wrong-ca.stderr' "$SIFT_VERIFY_SCRIPT"
for tool in sift_query sift_get_trace sift_correlate sift_list_services sift_tail_logs; do
  present_re "MCP no longer calls ${tool}" \
    "^  mcp_call [0-9]+ ${tool} " "$SIFT_VERIFY_SCRIPT"
done
present "MCP query no longer proves the known log content" \
  'MCP sift_query did not return the known smoke log' "$SIFT_VERIFY_SCRIPT"
present "MCP trace no longer proves the known span content" \
  'MCP sift_get_trace did not return the known smoke span' "$SIFT_VERIFY_SCRIPT"
present "MCP correlation no longer proves related signal content" \
  'MCP sift_correlate did not return known correlated signals' "$SIFT_VERIFY_SCRIPT"
present "MCP service listing no longer proves all three signals" \
  'MCP sift_list_services did not return the three-signal smoke service' "$SIFT_VERIFY_SCRIPT"
present "MCP tail no longer proves the known log content" \
  'MCP sift_tail_logs did not return the known smoke log' "$SIFT_VERIFY_SCRIPT"
present "MCP query no longer proves cross-project isolation" \
  'MCP sift_query leaked the smoke log into another project' "$SIFT_VERIFY_SCRIPT"
present "MCP bad-Origin probe no longer uses valid authentication" \
  'bad_origin_status="$(curl' "$SIFT_VERIFY_SCRIPT"
present "cross-project allowed-header/denied-body mismatch is no longer tested" \
  'cross-project-allowed-header-denied-body.json' "$SIFT_VERIFY_SCRIPT"
present "cross-project denied-header/allowed-body mismatch is no longer tested" \
  'cross-project-denied-header-allowed-body.json' "$SIFT_VERIFY_SCRIPT"
present "Prometheus acceptance can again pass with an empty vector" \
  'and (.data.result | length) == 1' "$SIFT_VERIFY_SCRIPT"
present "restore convergence no longer requires commit indexes" \
  'and (map(.commit_index) | unique | length == 1)' "$SIFT_VERIFY_SCRIPT"
present_re "candidate image provenance gate is no longer invoked" \
  '^verify_sift_image_provenance$' "$SIFT_VERIFY_SCRIPT"
[[ -f "$SIFT_LOAD_DIGEST_SCRIPT" ]] \
  || fail "the independent Sift load digest helper is missing"
digest_golden="$(python3 "$SIFT_LOAD_DIGEST_SCRIPT" phase \
  --name steady --duration 1 --batch-items 2)"
[[ "$digest_golden" == "201abc9eb7f9d2270d82d26bb334d11d0a7f435effe582d1474f8e59eebec57e" ]] \
  || fail "the independent Sift load digest helper changed its golden result"
present "the Sift verifier lost the real 180-day rollover" \
  'retention-rollover' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier no longer waits for bounded retention completion" \
  '.storage.archive.retention_scan_pending == false' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier lost the active receipt after telemetry expiration" \
  'active six-hour receipt did not win over telemetry retention' "$SIFT_VERIFY_SCRIPT"
present "the Sift verifier lost its MCP SSE decoder" \
  'extract_sse_json() {' "$SIFT_VERIFY_SCRIPT"
present "MCP initialize again sends raw SSE to jq" \
  'extract_sse_json "$init_sse" "$init_body"' "$SIFT_VERIFY_SCRIPT"
present "MCP tools/list again sends raw SSE to jq" \
  'extract_sse_json "$list_sse" "$list_body"' "$SIFT_VERIFY_SCRIPT"
present "nested load evidence directories are no longer prepared" \
  'mkdir -p "$(dirname "$output")"' "$SIFT_VERIFY_SCRIPT"
present "large Sift load ConfigMaps no longer use create" \
  'kubectl create -f "$EVIDENCE_DIR/load/${phase}/${signal}/configmap.yaml"' "$SIFT_VERIFY_SCRIPT"
absent "large Sift load ConfigMaps still use apply and overflow the annotation limit" \
  'kubectl apply -f "$EVIDENCE_DIR/load/${phase}/${signal}/configmap.yaml"' "$SIFT_VERIFY_SCRIPT"
present "Sift deployment no longer requires GKE FQDN policy enforcement" \
  'fqdnnetworkpolicies.networking.gke.io' "$DEPLOY_SCRIPT"
present "Sift verifier lost the operator-managed auth binding proof" \
  'auth-delegator-binding.json' "$SIFT_VERIFY_SCRIPT"
[[ -f "$SIFT_AUTH_DELEGATOR_FILTER" ]] \
  || fail "Sift auth-delegator contract filter is missing"
auth_binding="$(jq -n '{
  roleRef:{
    apiGroup:"rbac.authorization.k8s.io",
    kind:"ClusterRole",
    name:"system:auth-delegator"
  },
  subjects:[
    {kind:"ServiceAccount", name:"sift", namespace:"sift"},
    {kind:"ServiceAccount", name:"sift-store", namespace:"sift"}
  ],
  metadata:{labels:{
    "app.kubernetes.io/name":"sift",
    "app.kubernetes.io/instance":"sift",
    "sift.axiom.dev/owner-namespace":"sift",
    "service-k8s.axiom.dev/owner-uid":"owner-uid"
  }}
}')"
jq -e --arg namespace sift --arg instance sift \
  -f "$SIFT_AUTH_DELEGATOR_FILTER" >/dev/null <<<"$auth_binding" \
  || fail "Sift auth-delegator contract rejected the exact runtime and store subjects"
missing_store="$(jq '.subjects = [.subjects[0]]' <<<"$auth_binding")"
if jq -e --arg namespace sift --arg instance sift \
  -f "$SIFT_AUTH_DELEGATOR_FILTER" >/dev/null <<<"$missing_store"; then
  fail "Sift auth-delegator contract accepted a binding without the store subject"
fi
extra_backup="$(jq '.subjects += [{kind:"ServiceAccount", name:"sift-backup", namespace:"sift"}]' \
  <<<"$auth_binding")"
if jq -e --arg namespace sift --arg instance sift \
  -f "$SIFT_AUTH_DELEGATOR_FILTER" >/dev/null <<<"$extra_backup"; then
  fail "Sift auth-delegator contract accepted an extra backup subject"
fi
wrong_namespace="$(jq '.subjects[1].namespace = "other"' <<<"$auth_binding")"
if jq -e --arg namespace sift --arg instance sift \
  -f "$SIFT_AUTH_DELEGATOR_FILTER" >/dev/null <<<"$wrong_namespace"; then
  fail "Sift auth-delegator contract accepted the store subject from another namespace"
fi
[[ -f "$SIFT_ARCHIVE_FQDN_FILTER" ]] \
  || fail "Sift archive FQDN policy contract filter is missing"
archive_fqdn="$(jq -n '{items:[{
  metadata:{
    name:"sift-store-google-apis",
    namespace:"sift",
    labels:{
      "app.kubernetes.io/name":"sift",
      "app.kubernetes.io/instance":"sift",
      "app.kubernetes.io/component":"store"
    }
  },
  spec:{
    podSelector:{matchLabels:{
      "app.kubernetes.io/name":"sift",
      "app.kubernetes.io/instance":"sift",
      "app.kubernetes.io/component":"store",
      "sift.axiom.dev/role":"store"
    }},
    egress:[{
      matches:[{name:"storage.googleapis.com"}],
      ports:[{port:443,protocol:"TCP"}]
    }]
  }
}]}')"
jq -e --arg namespace sift --arg instance sift \
  -f "$SIFT_ARCHIVE_FQDN_FILTER" >/dev/null <<<"$archive_fqdn" \
  || fail "Sift archive FQDN contract rejected the exact store policy"
extra_backup_policy="$(jq '.items += [{metadata:{name:"sift-backup-google-apis"},spec:{}}]' \
  <<<"$archive_fqdn")"
if jq -e --arg namespace sift --arg instance sift \
  -f "$SIFT_ARCHIVE_FQDN_FILTER" >/dev/null <<<"$extra_backup_policy"; then
  fail "Sift archive FQDN contract accepted a policy for a disabled backup role"
fi
wrong_fqdn="$(jq '.items[0].spec.egress[0].matches[0].name = "*.googleapis.com"' \
  <<<"$archive_fqdn")"
if jq -e --arg namespace sift --arg instance sift \
  -f "$SIFT_ARCHIVE_FQDN_FILTER" >/dev/null <<<"$wrong_fqdn"; then
  fail "Sift archive FQDN contract accepted broad Google API egress"
fi
wrong_selector="$(jq '.items[0].spec.podSelector.matchLabels["app.kubernetes.io/instance"] = "other"' \
  <<<"$archive_fqdn")"
if jq -e --arg namespace sift --arg instance sift \
  -f "$SIFT_ARCHIVE_FQDN_FILTER" >/dev/null <<<"$wrong_selector"; then
  fail "Sift archive FQDN contract accepted another Sift instance"
fi
present "Sift verifier lost instance-scoped FQDN policy proof" \
  'fqdn-network-policies.json' "$SIFT_VERIFY_SCRIPT"
rg -q '^\s*datapath_provider\s*=\s*"ADVANCED_DATAPATH"\s*$' "$CLUSTER_TF" \
  || fail "persistent cluster lost Dataplane V2"
rg -q '^\s*enable_fqdn_network_policy\s*=\s*true\s*$' "$CLUSTER_TF" \
  || fail "persistent cluster lost FQDN Network Policy"
present "cluster reuse no longer rejects an incompatible dataplane" \
  'requires Dataplane V2 and FQDN Network Policy' "$BOOTSTRAP_SCRIPT"
present "cluster reuse reads Dataplane V2 from GKE networkConfig" \
  '.networkConfig.datapathProvider // ""' "$BOOTSTRAP_SCRIPT"
present "cluster reuse reads FQDN policy from GKE networkConfig" \
  '.networkConfig.enableFqdnNetworkPolicy // false' "$BOOTSTRAP_SCRIPT"

# --- phase ordering ---------------------------------------------------------
# Lumen's bundle must be materialized before the Sift branch consumes the
# shared manifest tree; a reordering here silently renders Sift against a
# half-built directory.
lumen_bundle_line="$(line_of 'kubectl kustomize "$MANIFEST_DIR/lumen/operator"' "$RENDER_SCRIPT")"
sift_manifest_line="$(line_of 'cat > "$MANIFEST_DIR/sift/operator/kustomization.yaml"' "$RENDER_SCRIPT")"
[[ "$lumen_bundle_line" =~ ^[0-9]+$ && "$sift_manifest_line" =~ ^[0-9]+$ ]] \
  || fail "could not locate the lumen/sift render phases"
(( lumen_bundle_line < sift_manifest_line )) \
  || fail "lumen must bundle before the sift branch (lumen@$lumen_bundle_line, sift@$sift_manifest_line)"

# --- the control-plane observability leg stays lumen-gated (#2621) ----------
# sift and tape render no metrics Service, so an ungated assertion would fail
# their cells on an endpoint that was never supposed to exist.
present "the observability leg lost its app guard" 'if [[ "$app" == "lumen" ]]; then' "$CELL_SCRIPT"
present "the leader gauge is no longer cross-checked against the lease" \
  'require_leader_gauge_agrees' "$CELL_SCRIPT"
# The port counter must stay file-backed: callers read these helpers through
# `$(...)`, so a plain variable increment is discarded and every scrape reuses
# one port -- two pods then return byte-identical metrics.
present "the metrics port counter stopped being file-backed" \
  'printf '"'"'%s\n'"'"' "$p" > "$metrics_port_state"' "$CELL_SCRIPT"

# --- bootstrap-cluster.sh's stdout is a one-line contract ------------------
# Its two branches are asymmetric: reuse prints the name and returns, create
# runs terraform first. When terraform's chatter went to stdout, the create
# branch wrote ~19KB of plan output into the file the caller asserts on, and
# the run died mute AFTER paying for the cluster. Every earlier run reused an
# existing cluster, so the create branch had never been exercised -- which is
# exactly the shape of bug a static oracle has to hold, because reproducing it
# costs ten minutes of GKE.
bootstrap_terraform_lines="$(rg -c -F 'terraform \' "$BOOTSTRAP_SCRIPT" || echo 0)"
(( bootstrap_terraform_lines == 2 )) \
  || fail "expected 2 terraform invocations in bootstrap-cluster.sh, found $bootstrap_terraform_lines"
redirected="$(rg -c -F '>&2' "$BOOTSTRAP_SCRIPT" || echo 0)"
(( redirected >= 2 )) \
  || fail "bootstrap-cluster.sh must send terraform output to stderr; only $redirected redirect(s) found — its stdout is the cluster name and nothing else"
present "the cluster-name check went back to a mute bare test" \
  "bootstrap-cluster.sh must emit exactly" "$RUN_SCRIPT"
absent "the cluster-name check is back to inspecting only line 1" \
  "test \"\$(sed -n '1p' \"\$EVIDENCE_DIR/persistent-cluster-name.txt\")\"" "$RUN_SCRIPT"

present "the reuse branch stopped warning about data-plane pool drift" \
  "node pool; the spec.placement leg will fail" "$BOOTSTRAP_SCRIPT"

# --- every namespace the run can create is swept, and checked (#2462) --------
# The fleet leg materializes data planes into namespaces of its own and tears
# them down itself on the happy path. A leg that fails midway does not, and
# this cluster is persistent -- so a leaked StatefulSet PVC is a Persistent
# Disk that bills indefinitely with nothing left pointing at it. Cleanup must
# name those namespaces even though a passing run never needs it to, and the
# no-leftovers gate must look for them, or the leak is invisible by design.
for fleet_ns in lumen-fleet-a lumen-fleet-b; do
  present "cleanup no longer sweeps the $fleet_ns data-plane namespace" \
    "$fleet_ns" "$CLEANUP_SCRIPT"
  present "the no-leftovers gate stopped checking $fleet_ns" \
    "$fleet_ns" "$VERIFY_CLEAN_SCRIPT"
done
present "cleanup no longer removes the cluster-scoped LumenFleet CRD" \
  "lumenfleets.lumen.dev" "$CLEANUP_SCRIPT"
present "the no-leftovers gate stopped checking the LumenFleet CRD" \
  "lumenfleets.lumen.dev" "$VERIFY_CLEAN_SCRIPT"
# Ordering, not just presence: the fleet controller reconciles cluster-wide, so
# its CRD must go before its target namespaces start terminating. Reversed, a
# reconcile pass landing between two deletes re-materializes a Lumen into a
# namespace on its way out and the gate trips on what cleanup just removed.
fleet_crd_line="$(line_of 'kubectl delete customresourcedefinition lumenfleets.lumen.dev' "$CLEANUP_SCRIPT")"
ns_delete_line="$(line_of 'kubectl delete namespace "$namespace" --ignore-not-found' "$CLEANUP_SCRIPT")"
[[ "$fleet_crd_line" =~ ^[0-9]+$ && "$ns_delete_line" =~ ^[0-9]+$ ]] \
  || fail "could not locate cleanup's fleet-CRD and namespace deletes"
(( fleet_crd_line < ns_delete_line )) \
  || fail "the LumenFleet CRD must be deleted before the namespace sweep (crd@$fleet_crd_line, namespaces@$ns_delete_line)"

# A Sift CR owns cluster-scoped children through an operator finalizer. Delete
# that CR while the operator namespace is still live. Otherwise the namespace
# and CRD stay Terminating with nobody left to remove the finalizer.
sift_cr_delete_line="$(line_of 'delete_sift_instance sift sift' "$CLEANUP_SCRIPT")"
[[ "$sift_cr_delete_line" =~ ^[0-9]+$ ]] \
  || fail "could not locate cleanup's ordered Sift CR delete"
(( sift_cr_delete_line < ns_delete_line )) \
  || fail "Sift CRs must be deleted before the namespace sweep (cr@$sift_cr_delete_line, namespaces@$ns_delete_line)"
present "cleanup lost the orphaned Sift finalizer fallback" \
  'patch sift.sift.axiom.dev "$name" --namespace "$namespace"' "$CLEANUP_SCRIPT"
present "cleanup no longer checks the binding owner UID before its finalizer fallback" \
  'service-k8s.axiom.dev/owner-uid' "$CLEANUP_SCRIPT"
present "cleanup no longer requires an accepted Sift CR deletion before fallback" \
  '.metadata.deletionTimestamp | strings | select(length > 0)' "$CLEANUP_SCRIPT"
present "cleanup uses the wrong Sift API version for its preconditioned delete" \
  '/apis/sift.axiom.dev/v1alpha1/namespaces/' "$CLEANUP_SCRIPT"
present "cleanup no longer uses Kubernetes delete preconditions" \
  'resourceVersion: $resource_version' "$CLEANUP_SCRIPT"
present "cleanup no longer applies an atomic finalizer patch" \
  '{op:"test", path:"/metadata/uid", value:$uid}' "$CLEANUP_SCRIPT"
binding_delete_line="$(line_of '"$binding_path" "$binding_uid" "$binding_resource_version"' "$CLEANUP_SCRIPT")"
binding_patch_line="$(line_of 'kubectl patch sift.sift.axiom.dev "$name"' "$CLEANUP_SCRIPT")"
[[ "$binding_delete_line" =~ ^[0-9]+$ && "$binding_patch_line" =~ ^[0-9]+$ \
  && "$binding_delete_line" -lt "$binding_patch_line" ]] \
  || fail "Sift cleanup must delete the exactly owned auth binding before removing the CR finalizer"

present "the lumen-sift pre-flight list names lumen-auth-client" \
  'mode_namespaces=(lumen lumen-system sift sift-system lumen-auth-client)' "$RUN_SCRIPT"

if [[ "${SIFT_ORACLE_MUTATION_CHILD:-0}" != "1" ]]; then
  mutation_dir="$(mktemp -d "${TMPDIR:-/tmp}/sift-acceptance-oracle.XXXXXX")"
  cleanup_mutations() {
    find "$mutation_dir" -type f -delete
    find "$mutation_dir" -depth -type d -empty -delete
  }
  trap cleanup_mutations EXIT INT TERM

  mutation_must_fail() {
    local label="$1"
    local fixture="$2"
    if SIFT_ORACLE_MUTATION_CHILD=1 SIFT_VERIFY_SCRIPT_OVERRIDE="$fixture" \
      bash "$0" >/dev/null 2>&1; then
      fail "the static oracle accepted mutation: $label"
    fi
  }

  dockerfile_mutation_must_fail() {
    local label="$1"
    local fixture="$2"
    if SIFT_ORACLE_MUTATION_CHILD=1 \
      SIFT_ACCEPTANCE_RUNNER_IMAGE_OVERRIDE="$fixture" \
      bash "$0" >/dev/null 2>&1; then
      fail "the static oracle accepted Dockerfile mutation: $label"
    fi
  }

  quoted_cli_env="$mutation_dir/quoted-cli-env.Dockerfile"
  cp "$SIFT_ACCEPTANCE_RUNNER_IMAGE" "$quoted_cli_env"
  printf '%s\n' 'ENV SIFT_CLI="/usr/local/bin/sift"' >> "$quoted_cli_env"
  dockerfile_mutation_must_fail "quoted SIFT_CLI ENV" "$quoted_cli_env"

  legacy_cli_env="$mutation_dir/legacy-cli-env.Dockerfile"
  cp "$SIFT_ACCEPTANCE_RUNNER_IMAGE" "$legacy_cli_env"
  printf '%s\n' 'ENV SIFT_CLI /usr/local/bin/sift' >> "$legacy_cli_env"
  dockerfile_mutation_must_fail "legacy SIFT_CLI ENV" "$legacy_cli_env"

  lowercase_cli_env="$mutation_dir/lowercase-cli-env.Dockerfile"
  cp "$SIFT_ACCEPTANCE_RUNNER_IMAGE" "$lowercase_cli_env"
  printf '%s\n' 'env SIFT_CLI=/usr/local/bin/sift' >> "$lowercase_cli_env"
  dockerfile_mutation_must_fail "lowercase SIFT_CLI ENV" "$lowercase_cli_env"

  continued_cli_env="$mutation_dir/continued-cli-env.Dockerfile"
  cp "$SIFT_ACCEPTANCE_RUNNER_IMAGE" "$continued_cli_env"
  printf '%s\n' 'ENV HOME=/tmp/sift-acceptance-home \' \
    '    SIFT_CLI=/usr/local/bin/sift' >> "$continued_cli_env"
  dockerfile_mutation_must_fail "continued SIFT_CLI ENV" "$continued_cli_env"

  quoted_load="$mutation_dir/quoted-load.sh"
  sed 's/^LOAD_SECONDS=1800$/LOAD_SECONDS=1799/' \
    "$SIFT_VERIFY_SCRIPT" > "$quoted_load"
  printf '%s\n' "echo 'LOAD_SECONDS=1800' >/dev/null" >> "$quoted_load"
  mutation_must_fail "quoted prose replaced the 30-minute load" "$quoted_load"

  divergent_outage="$mutation_dir/divergent-outage.sh"
  sed 's/^verify_outage_quorum_recovery /skip_outage_quorum_recovery /' \
    "$SIFT_VERIFY_SCRIPT" > "$divergent_outage"
  printf '%s\n' "echo 'verify_outage_quorum_recovery \"\$archive_leader\" \\' >/dev/null" \
    >> "$divergent_outage"
  mutation_must_fail "quoted prose hid a divergent outage invocation" "$divergent_outage"

  missing_failover_call="$mutation_dir/missing-failover-call.sh"
  awk '
    /^assert_failover_restart_evidence \\$/ { skip=3; next }
    skip > 0 { skip--; next }
    { print }
  ' "$SIFT_VERIFY_SCRIPT" > "$missing_failover_call"
  mutation_must_fail "the failover pod comparison was defined but never called" \
    "$missing_failover_call"

  mutable_pin="$mutation_dir/mutable-pin.sh"
  sed 's/== \*@sha256:\*/== *:*/' \
    "$SIFT_VERIFY_SCRIPT" > "$mutable_pin"
  rg -F '[[ "$SIFT_IMAGE" == *:* ]]' "$mutable_pin" >/dev/null \
    || fail "could not build the mutable-image mutation"
  printf '%s\n' "echo '[[ \"\$SIFT_IMAGE\" == *@sha256:* ]]' >/dev/null" >> "$mutable_pin"
  mutation_must_fail "quoted prose hid a mutable Sift image" "$mutable_pin"

  early_acceptance="$mutation_dir/early-acceptance.sh"
  cp "$SIFT_VERIFY_SCRIPT" "$early_acceptance"
  printf '%s\n' 'cp "$EVIDENCE_DIR/sift-mvp-verification.json" "$EVIDENCE_DIR/acceptance.json"' \
    >> "$early_acceptance"
  mutation_must_fail "the verifier wrote acceptance.json before cleanup" "$early_acceptance"
fi

echo "acceptance-mode oracle: ok"
# HANDWRITE-END
