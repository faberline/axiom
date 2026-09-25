#!/usr/bin/env bash
# Verify only a run-scoped release candidate of one shared-script app (sift,
# keep, relay, defer). This script never creates a Git tag, GitHub Release,
# semver/latest image tag, signature, or attestation. tape keeps its own
# copies under apps/tape/scripts; the per-app facts here come from
# scripts/release/apps.sh.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/release/apps.sh
source "$SCRIPT_DIR/apps.sh"

usage() {
  cat >&2 <<'USAGE'
Usage: verify-release-candidate.sh --app <sift|keep|relay|defer> \
  --repo <owner/repo> --version <semver> --commit <40-hex> \
  --run-id <id> --run-attempt <attempt> \
  --manifest <path> --manifest-sidecar <path> --artifacts-dir <directory> \
  [--image <repo@sha256:...> --candidate-tag <tag> \
   --amd64-digest <sha256:...> --arm64-digest <sha256:...>] \
  --mode <local|full>

`local` validates synthetic files only. It is not candidate acceptance.
`full` also validates the run-scoped GHCR image and its existing attestations.
USAGE
  exit 2
}

fail() { printf '%s\n' "$*" >&2; exit 1; }

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

target_list() { release_app_targets "$APP"; }

host_target() {
  case "$(uname -s):$(uname -m)" in
    Darwin:arm64|Darwin:aarch64) printf '%s\n' aarch64-apple-darwin ;;
    Linux:x86_64|Linux:amd64) printf '%s\n' x86_64-unknown-linux-gnu ;;
    Linux:aarch64|Linux:arm64) printf '%s\n' aarch64-unknown-linux-gnu ;;
    *) fail "unsupported verifier host: $(uname -s) $(uname -m)" ;;
  esac
}

# The archive this host can execute: its own triple when the app ships it,
# otherwise (Linux only) the static musl sibling, which runs on any Linux
# regardless of the host libc. Apps that ship only musl archives still get
# their binary run here instead of a skipped check.
host_archive_target() {
  local host musl
  host="$(host_target)"
  if target_list | grep -qx "$host"; then printf '%s\n' "$host"; return 0; fi
  musl="${host%-gnu}-musl"
  if [[ "$host" == *-unknown-linux-gnu ]] && target_list | grep -qx "$musl"; then printf '%s\n' "$musl"; return 0; fi
  return 1
}
require_regular_file() { [[ -f "$1" && ! -L "$1" ]] || fail "required regular file is absent: $1"; }

verify_archive() {
  local target="$1" archive="$2" archive_sha="$3" sidecar="$4" sidecar_sha="$5"
  local listed_sha listed_name extra members mode
  require_regular_file "$archive"
  require_regular_file "$sidecar"
  [[ "$(sha256_file "$archive")" == "$archive_sha" ]] || fail "archive checksum mismatch: ${archive##*/}"
  [[ "$(sha256_file "$sidecar")" == "$sidecar_sha" ]] || fail "sidecar checksum mismatch: ${sidecar##*/}"
  read -r listed_sha listed_name extra <"$sidecar" || fail "cannot read checksum sidecar: ${sidecar##*/}"
  [[ -z "${extra:-}" && "$listed_sha" == "$archive_sha" && "$listed_name" == "${archive##*/}" ]] || fail "invalid checksum sidecar: ${sidecar##*/}"
  members="$(tar -tzf "$archive" | LC_ALL=C sort)" || fail "archive cannot be listed: ${archive##*/}"
  [[ "$members" == "$(printf '%s\n' "${APP}-${target}/" "${APP}-${target}/README.md" "${APP}-${target}/${APP}" | LC_ALL=C sort)" ]] || fail "archive members changed: ${archive##*/}"
  mode="$(tar -tvzf "$archive" | awk -v path="${APP}-${target}/${APP}" '$NF == path { print $1 }')" || fail "archive metadata cannot be read: ${archive##*/}"
  [[ "$mode" =~ ^-.{2}x ]] || fail "archive binary is not executable: ${archive##*/}"
}

validate_manifest() {
  local targets expected_keys actual_keys expected_jobs target_count
  expected_keys='["artifacts","candidate_tag","commit","image","pr","repository","run_attempt","run_id","run_url","sboms","schema","source_ref","tag","version","workflow_id","workflow_path","workflow_ref"]'
  actual_keys="$(jq -cer 'keys | sort' "$MANIFEST")" || fail "candidate manifest is not JSON"
  expected_jobs="$(release_app_candidate_jobs "$APP" | jq -Rn '[inputs | select(length > 0)] | map({key: ., value: "success"}) | from_entries')"
  if jq -e 'has("jobs")' "$MANIFEST" >/dev/null; then
    expected_keys='["artifacts","candidate_tag","commit","image","jobs","pr","repository","run_attempt","run_id","run_url","sboms","schema","source_ref","tag","version","workflow_id","workflow_path","workflow_ref"]'
    jq -e --argjson jobs "$expected_jobs" '.jobs == $jobs' "$MANIFEST" >/dev/null || fail "final candidate manifest does not bind all successful jobs"
  fi
  [[ "$actual_keys" == "$expected_keys" ]] || fail "candidate manifest keys changed: $actual_keys"
  targets="$(target_list | jq -Rsc 'split("\n") | map(select(length > 0))')"
  target_count="$(jq -r 'length' <<<"$targets")"
  jq -e --arg app "$APP" --arg repo "$REPO" --arg version "$VERSION" --arg commit "$COMMIT" --arg run_id "$RUN_ID" --arg attempt "$RUN_ATTEMPT" --arg tag "${APP}@${VERSION}" --arg candidate "release-candidate-${RUN_ID}-${RUN_ATTEMPT}" --arg schema "$(release_app_manifest_schema "$APP")" --arg workflow_path ".github/workflows/${APP}-release-candidate.yml" --arg workflow_ref "${REPO}/.github/workflows/${APP}-release-candidate.yml@refs/heads/main" --arg image_repo "${RELEASE_IMAGE_OWNER}/${APP}" --argjson targets "$targets" --argjson count "$target_count" '
    .schema == $schema and
    .repository == $repo and .workflow_path == $workflow_path and
    (.workflow_id | type == "number" and . > 0) and .run_id == $run_id and .run_attempt == $attempt and
    .run_url == ("https://github.com/" + $repo + "/actions/runs/" + $run_id + "/attempts/" + $attempt) and
    .source_ref == "refs/heads/main" and .workflow_ref == $workflow_ref and
    .commit == $commit and .version == $version and .tag == $tag and .candidate_tag == $candidate and
    (.pr | type == "object" and (.number | type == "number" and . > 0) and (.url | type == "string" and test("^https://github\\.com/" + $repo + "/pull/"))) and
    (.image | type == "object" and (.repository == $image_repo) and (.root_digest | type == "string" and test("^sha256:[0-9a-f]{64}$")) and (.amd64_digest | type == "string" and test("^sha256:[0-9a-f]{64}$")) and (.arm64_digest | type == "string" and test("^sha256:[0-9a-f]{64}$"))) and
    ([.image.root_digest, .image.amd64_digest, .image.arm64_digest] | unique | length == 3) and
    (.artifacts | type == "array" and length == $count and map(.target) == $targets and all(.[];
      (keys | sort) == ["archive","archive_sha256","sidecar","sidecar_sha256","target"] and
      (.archive == ($app + "-" + .target + ".tar.gz")) and
      (.sidecar == (.archive + ".sha256")) and
      (.archive_sha256 | type == "string" and test("^[0-9a-f]{64}$")) and
      (.sidecar_sha256 | type == "string" and test("^[0-9a-f]{64}$")))) and
    (.sboms | type == "object" and (keys | sort) == ["amd64","arm64"] and
      (.amd64 | (keys | sort) == ["file","sha256"] and .file == "spdx-amd64.json" and (.sha256 | type == "string" and test("^[0-9a-f]{64}$"))) and
      (.arm64 | (keys | sort) == ["file","sha256"] and .file == "spdx-arm64.json" and (.sha256 | type == "string" and test("^[0-9a-f]{64}$"))))
  ' "$MANIFEST" >/dev/null || fail "candidate manifest bindings changed"
}

verify_local_artifacts() {
  local target archive archive_sha sidecar sidecar_sha private_home unpack_dir binary version_line sbom sbom_sha
  validate_manifest
  while IFS= read -r target; do
    archive="$(jq -er --arg target "$target" '.artifacts[] | select(.target == $target) | .archive' "$MANIFEST")"
    archive_sha="$(jq -er --arg target "$target" '.artifacts[] | select(.target == $target) | .archive_sha256' "$MANIFEST")"
    sidecar="$(jq -er --arg target "$target" '.artifacts[] | select(.target == $target) | .sidecar' "$MANIFEST")"
    sidecar_sha="$(jq -er --arg target "$target" '.artifacts[] | select(.target == $target) | .sidecar_sha256' "$MANIFEST")"
    verify_archive "$target" "$ARTIFACTS_DIR/$archive" "$archive_sha" "$ARTIFACTS_DIR/$sidecar" "$sidecar_sha"
  done < <(target_list)
  for sbom in amd64 arm64; do
    sbom_sha="$(jq -er --arg arch "$sbom" '.sboms[$arch].sha256' "$MANIFEST")"
    require_regular_file "$ARTIFACTS_DIR/spdx-${sbom}.json"
    [[ "$(sha256_file "$ARTIFACTS_DIR/spdx-${sbom}.json")" == "$sbom_sha" ]] || fail "SBOM checksum mismatch: ${sbom}"
    jq -e '.spdxVersion == "SPDX-2.3"' "$ARTIFACTS_DIR/spdx-${sbom}.json" >/dev/null || fail "invalid SPDX 2.3 SBOM: ${sbom}"
  done
  target="$(host_archive_target)" || {
    printf 'note: %s ships no archive this verifier host (%s) can run; host binary check skipped\n' "$APP" "$(host_target)" >&2
    return 0
  }
  archive="$(jq -er --arg target "$target" '.artifacts[] | select(.target == $target) | .archive' "$MANIFEST")"
  private_home="$(mktemp -d)"
  unpack_dir="$(mktemp -d)"
  trap 'rm -rf "${private_home:-}" "${unpack_dir:-}"' RETURN
  mkdir -p "$private_home/tmp"
  tar -xzf "$ARTIFACTS_DIR/$archive" -C "$unpack_dir"
  binary="$unpack_dir/${APP}-${target}/${APP}"
  [[ -f "$binary" && -x "$binary" ]] || fail "candidate archive has no executable host binary"
  version_line="$(env -i HOME="$private_home" PATH="$PATH" TMPDIR="$private_home/tmp" "$binary" --version)" || fail "candidate binary did not report a version"
  [[ "$version_line" == "${APP} $VERSION" ]] || fail "candidate binary version mismatch: $version_line"
  [[ ! -e "$private_home/.aws" && ! -e "$private_home/.config/gcloud" ]] || fail "candidate binary wrote credential state"
  rm -rf "$private_home" "$unpack_dir"
  trap - RETURN
}

verify_attestation() {
  local label="$1" subject="$2" digest="$3" predicate="$4" result
  result="$(gh attestation verify "oci://${subject}" --bundle-from-oci --repo "$REPO" --source-ref refs/heads/main --source-digest "$COMMIT" --cert-identity "$EXPECTED_CERT_ID" --cert-oidc-issuer https://token.actions.githubusercontent.com --predicate-type "$predicate" --format json)"
  jq -e --arg digest "${digest#sha256:}" --arg predicate "$predicate" '
    type == "array" and length > 0 and all(.[]; .verificationResult.statement.predicateType == $predicate and all(.verificationResult.statement.subject[]; .digest.sha256 == $digest))
  ' <<<"$result" >/dev/null || fail "$label attestation subject or predicate mismatch"
}

verify_sbom_attestation() {
  local arch="$1" subject="$2" digest="$3" result
  result="$(gh attestation verify "oci://${subject}" --bundle-from-oci --repo "$REPO" --source-ref refs/heads/main --source-digest "$COMMIT" --cert-identity "$EXPECTED_CERT_ID" --cert-oidc-issuer https://token.actions.githubusercontent.com --predicate-type "https://spdx.dev/Document/v2.3" --format json)"
  jq -e --arg digest "${digest#sha256:}" --slurpfile sbom "$ARTIFACTS_DIR/spdx-${arch}.json" '
    type == "array" and any(.[];
      .verificationResult.statement.predicateType == "https://spdx.dev/Document/v2.3" and
      ([.verificationResult.statement.subject[]?.digest.sha256] | index($digest) != null) and
      .verificationResult.statement.predicate == $sbom[0])
  ' <<<"$result" >/dev/null || fail "linux/${arch} SBOM does not semantically match a verified child attestation"
}

verify_full_supply_chain() {
  local image_repo root_digest reported_root raw candidate_digest labels expected_run_url
  [[ -n "$IMAGE" && -n "$AMD64_DIGEST" && -n "$ARM64_DIGEST" ]] || fail "full mode requires image and child digests"
  image_repo="${IMAGE%@*}"
  root_digest="${IMAGE#*@}"
  [[ "$IMAGE" == "${RELEASE_IMAGE_OWNER}/${APP}@sha256:"* ]] || fail "invalid candidate image"
  [[ "$CANDIDATE_TAG" == "release-candidate-${RUN_ID}-${RUN_ATTEMPT}" ]] || fail "candidate tag is not scoped to this run attempt"
  jq -e --arg root "$root_digest" --arg amd64 "$AMD64_DIGEST" --arg arm64 "$ARM64_DIGEST" --arg tag "$CANDIDATE_TAG" '
    .image.root_digest == $root and .image.amd64_digest == $amd64 and .image.arm64_digest == $arm64 and .candidate_tag == $tag
  ' "$MANIFEST" >/dev/null || fail "CLI and manifest image identity differ"
  reported_root="$(docker buildx imagetools inspect "$IMAGE" --format '{{json .Manifest}}' | jq -er '.digest')" || fail "candidate root digest cannot be resolved"
  [[ "$reported_root" == "$root_digest" ]] || fail "candidate root digest does not bind image reference"
  raw="$(docker buildx imagetools inspect --raw "$IMAGE")"
  jq -e --arg amd64 "$AMD64_DIGEST" --arg arm64 "$ARM64_DIGEST" '
    (.manifests | type == "array" and length == 2) and all(.manifests[]; .platform.os == "linux" and (.platform | has("variant") | not)) and
    ([.manifests[] | {arch:.platform.architecture,digest}] | sort_by(.arch) == [{arch:"amd64",digest:$amd64},{arch:"arm64",digest:$arm64}])
  ' <<<"$raw" >/dev/null || fail "root image index is not the exact two child digests"
  candidate_digest="$(docker buildx imagetools inspect "${image_repo}:${CANDIDATE_TAG}" --format '{{json .Manifest}}' | jq -er '.digest')" || fail "candidate tag cannot be resolved"
  [[ "$candidate_digest" == "$root_digest" ]] || fail "candidate tag digest does not bind root index"
  expected_run_url="https://github.com/${REPO}/actions/runs/${RUN_ID}/attempts/${RUN_ATTEMPT}"
  for pair in "amd64:$AMD64_DIGEST" "arm64:$ARM64_DIGEST"; do
    labels="$(docker buildx imagetools inspect "${image_repo}@${pair#*:}" --format '{{json .Image.Config.Labels}}')" || fail "${pair%%:*} image labels cannot be read"
    jq -e --arg commit "$COMMIT" --arg version "$VERSION" --arg run_url "$expected_run_url" '
      .["org.opencontainers.image.source"] == "https://github.com/chrischeng-c4/axiom" and .["org.opencontainers.image.revision"] == $commit and .["org.opencontainers.image.version"] == $version and .["org.opencontainers.image.url"] == $run_url
    ' <<<"$labels" >/dev/null || fail "${pair%%:*} image labels do not bind candidate identity"
  done
  cosign verify --certificate-identity "$EXPECTED_CERT_ID" --certificate-oidc-issuer https://token.actions.githubusercontent.com "$IMAGE" >/dev/null
  verify_attestation "root provenance" "$IMAGE" "$root_digest" "https://slsa.dev/provenance/v1"
  verify_sbom_attestation amd64 "${image_repo}@${AMD64_DIGEST}" "$AMD64_DIGEST"
  verify_sbom_attestation arm64 "${image_repo}@${ARM64_DIGEST}" "$ARM64_DIGEST"
}

if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then return 0; fi

APP=""; REPO=""; VERSION=""; COMMIT=""; RUN_ID=""; RUN_ATTEMPT=""; MANIFEST=""; MANIFEST_SIDECAR=""; ARTIFACTS_DIR=""; IMAGE=""; CANDIDATE_TAG=""; AMD64_DIGEST=""; ARM64_DIGEST=""; MODE=""
while [[ $# -gt 0 ]]; do
  [[ $# -ge 2 ]] || usage
  case "$1" in
    --app) [[ -z "$APP" ]] || usage; APP="$2" ;;
    --repo) [[ -z "$REPO" ]] || usage; REPO="$2" ;;
    --version) [[ -z "$VERSION" ]] || usage; VERSION="$2" ;;
    --commit) [[ -z "$COMMIT" ]] || usage; COMMIT="$2" ;;
    --run-id) [[ -z "$RUN_ID" ]] || usage; RUN_ID="$2" ;;
    --run-attempt) [[ -z "$RUN_ATTEMPT" ]] || usage; RUN_ATTEMPT="$2" ;;
    --manifest) [[ -z "$MANIFEST" ]] || usage; MANIFEST="$2" ;;
    --manifest-sidecar) [[ -z "$MANIFEST_SIDECAR" ]] || usage; MANIFEST_SIDECAR="$2" ;;
    --artifacts-dir) [[ -z "$ARTIFACTS_DIR" ]] || usage; ARTIFACTS_DIR="$2" ;;
    --image) [[ -z "$IMAGE" ]] || usage; IMAGE="$2" ;;
    --candidate-tag) [[ -z "$CANDIDATE_TAG" ]] || usage; CANDIDATE_TAG="$2" ;;
    --amd64-digest) [[ -z "$AMD64_DIGEST" ]] || usage; AMD64_DIGEST="$2" ;;
    --arm64-digest) [[ -z "$ARM64_DIGEST" ]] || usage; ARM64_DIGEST="$2" ;;
    --mode) [[ -z "$MODE" ]] || usage; MODE="$2" ;;
    *) usage ;;
  esac
  shift 2
done
[[ -n "$APP" ]] || usage
release_app_require "$APP"
release_app_uses_shared_scripts "$APP" || fail "$APP verifies with apps/$APP/scripts/verify-release-candidate.sh, not the shared verifier"
[[ "$REPO" == "$RELEASE_REPO" ]] || fail "unsupported repository: $REPO"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ && "$COMMIT" =~ ^[0-9a-f]{40}$ ]] || fail "invalid version or commit"
[[ "$RUN_ID" =~ ^[0-9]+$ && "$RUN_ATTEMPT" =~ ^[0-9]+$ ]] || fail "invalid run identity"
[[ "$MODE" == local || "$MODE" == full ]] || fail "mode must be local or full"
require_regular_file "$MANIFEST"; require_regular_file "$MANIFEST_SIDECAR"
[[ -d "$ARTIFACTS_DIR" && ! -L "$ARTIFACTS_DIR" ]] || fail "artifacts directory is invalid"
manifest_sha="$(sha256_file "$MANIFEST")"
read -r sidecar_sha sidecar_name sidecar_extra <"$MANIFEST_SIDECAR" || fail "cannot read manifest sidecar"
[[ -z "${sidecar_extra:-}" && "$sidecar_sha" == "$manifest_sha" && "$sidecar_name" == "${MANIFEST##*/}" ]] || fail "manifest sidecar does not bind exact manifest bytes"
CANDIDATE_TAG="${CANDIDATE_TAG:-release-candidate-${RUN_ID}-${RUN_ATTEMPT}}"
EXPECTED_CERT_ID="https://github.com/${REPO}/.github/workflows/${APP}-release-candidate.yml@refs/heads/main"
verify_local_artifacts
if [[ "$MODE" == full ]]; then
  verify_full_supply_chain
  printf 'FULL CANDIDATE VERIFICATION PASS: %s %s %s\n' "$APP" "$VERSION" "$COMMIT"
else
  printf 'LOCAL FIXTURE ONLY: artifacts verified; this is not candidate acceptance.\n'
fi
