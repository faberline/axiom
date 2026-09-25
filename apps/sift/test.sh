#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script_root="$(cd "$script_dir/../.." && pwd -P)"
if [[ -n "${SIFT_REPO_ROOT:-}" ]]; then
  if ! supplied_repo_root="$(cd "$SIFT_REPO_ROOT" 2>/dev/null && pwd -P)"; then
    echo "SIFT_REPO_ROOT does not name a readable directory" >&2
    exit 1
  fi
  if [[ "$supplied_repo_root" != "$script_root" ]]; then
    echo "SIFT_REPO_ROOT must match the repository that contains apps/sift/test.sh" >&2
    exit 1
  fi
fi
repo_root="$script_root"
export SIFT_REPO_ROOT="$repo_root"
cd "$repo_root"

if [[ "${1:-}" == "--print-repo-root" ]]; then
  printf '%s\n' "$repo_root"
  exit 0
fi

if [[ "${1:-}" == "--candidate" ]]; then
  shift
  if [[ -n "${SIFT_BIN:-}" ]]; then
    echo "Sift candidate mode does not accept a caller-supplied SIFT_BIN" >&2
    exit 1
  fi
  candidate_revision="${SIFT_SOURCE_REVISION:-}"
  if [[ -z "$candidate_revision" ]]; then
    candidate_revision="$(
      git -c core.fsmonitor=false -C "$repo_root" rev-parse HEAD 2>/dev/null
    )" || {
      echo "SIFT_SOURCE_REVISION is required outside a Git checkout" >&2
      exit 1
    }
  fi
  candidate_revision="$(printf '%s' "$candidate_revision" | tr '[:upper:]' '[:lower:]')"
  [[ "$candidate_revision" =~ ^[0-9a-f]{40}$ ]] || {
    echo "SIFT_SOURCE_REVISION must be a full 40-character Git SHA" >&2
    exit 1
  }
  export SIFT_SOURCE_REVISION="$candidate_revision"
  bash apps/sift/e2e/candidate_root.sh
  cargo build --locked \
    -p vat -p lumen -p tape -p relay -p defer -p sift --bins
  bash scripts/faberline-core-test.sh build-stamp
  cargo test --locked -p sift "$@"
  candidate_bin="${CARGO_TARGET_DIR:-$repo_root/target}/debug/sift"
  [[ -x "$candidate_bin" ]] || {
    echo "candidate Sift binary was not produced at $candidate_bin" >&2
    exit 1
  }
  SIFT_EXPECTED_SOURCE_REVISION="$candidate_revision" \
    bash apps/sift/e2e/prometheus_compliance.sh
  bash apps/sift/e2e/docker_named_volume.sh
  exit 0
fi

exec cargo test -p sift "$@"
