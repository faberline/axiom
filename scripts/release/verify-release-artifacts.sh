#!/usr/bin/env bash
# Verify a promotion of one shared-script app (sift, keep, relay, defer). This
# script never creates or changes a tag, image, release, signature,
# provenance, or attestation. tape keeps its own copies under
# apps/tape/scripts; the per-app facts here come from scripts/release/apps.sh.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/release/apps.sh
source "$SCRIPT_DIR/apps.sh"

usage() {
  cat >&2 <<'USAGE'
Usage: verify-release-artifacts.sh --app <sift|keep|relay|defer> \
     --repo <owner/repo> --tag <app@semver> --commit <40-hex> \
     --candidate-run-id <id> --candidate-run-attempt <attempt> \
     --mode <candidate|fixture|public> \
  [--candidate-receipt-dir <dir> --release-assets-dir <dir> --output <path> \
   --gke-receipt <path> --gke-receipt-sidecar <path>]

candidate: prove the immutable tag, tag ruleset, candidate run, receipt, image,
and existing attestations. fixture: compare supplied candidate and release bytes
without network access. public: also verify the published GitHub Release.
USAGE
  exit 2
}

fail() { printf '%s\n' "$*" >&2; exit 1; }
require_file() { [[ -f "$1" && ! -L "$1" ]] || fail "required regular file is absent: $1"; }
sha256_file() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'; else shasum -a 256 "$1" | awk '{print $1}'; fi; }

image_digest_or_absent() {
  local ref="$1" error manifest status
  error="$(mktemp)"
  set +e
  manifest="$(docker buildx imagetools inspect "$ref" --format '{{json .Manifest}}' 2>"$error")"
  status=$?
  set -e
  if [[ "$status" == 0 ]]; then
    rm -f "$error"
    jq -er '.digest' <<<"$manifest"
    return 0
  fi
  if grep -Eqi 'manifest unknown|not found' "$error"; then
    rm -f "$error"
    return 0
  fi
  cat "$error" >&2
  rm -f "$error"
  fail "cannot prove GHCR image state: $ref"
}

targets() { release_app_targets "$APP"; }

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
  if targets | grep -qx "$host"; then printf '%s\n' "$host"; return 0; fi
  musl="${host%-gnu}-musl"
  if [[ "$host" == *-unknown-linux-gnu ]] && targets | grep -qx "$musl"; then printf '%s\n' "$musl"; return 0; fi
  return 1
}
expected_jobs_json() {
  release_app_candidate_jobs "$APP" | jq -Rn '[inputs | select(length > 0)] | map({key: ., value: "success"}) | from_entries'
}

validate_receipt() {
  local manifest="$1" sidecar="$2" receipt_dir="$3" actual expected name extra target archive sidecar_name targets_json count
  require_file "$manifest"; require_file "$sidecar"
  actual="$(sha256_file "$manifest")"
  read -r expected name extra <"$sidecar" || fail "cannot read final receipt sidecar"
  [[ "$expected" == "$actual" && "$name" == "${manifest##*/}" && -z "${extra:-}" ]] || fail "final receipt sidecar does not bind exact bytes"
  targets_json="$(targets | jq -Rsc 'split("\n") | map(select(length > 0))')"
  count="$(jq -r 'length' <<<"$targets_json")"
  jq -e --arg app "$APP" --arg repo "$REPO" --arg tag "$TAG" --arg version "${TAG#"${APP}"@}" --arg commit "$COMMIT" --arg run "$CANDIDATE_RUN_ID" --arg attempt "$CANDIDATE_RUN_ATTEMPT" --arg schema "$(release_app_manifest_schema "$APP")" --arg image_repo "${RELEASE_IMAGE_OWNER}/${APP}" --argjson jobs "$(expected_jobs_json)" --argjson targets "$targets_json" --argjson count "$count" '
    (keys | sort) == ["artifacts","candidate_tag","commit","image","jobs","pr","repository","run_attempt","run_id","run_url","sboms","schema","source_ref","tag","version","workflow_id","workflow_path","workflow_ref"] and
    .schema == $schema and .repository == $repo and .version == $version and .tag == $tag and .commit == $commit and
    .run_id == $run and .run_attempt == $attempt and .source_ref == "refs/heads/main" and .workflow_path == (".github/workflows/" + $app + "-release-candidate.yml") and
    .workflow_ref == ($repo + "/.github/workflows/" + $app + "-release-candidate.yml@refs/heads/main") and
    .run_url == ("https://github.com/" + $repo + "/actions/runs/" + .run_id + "/attempts/" + .run_attempt) and
    .candidate_tag == ("release-candidate-" + .run_id + "-" + .run_attempt) and
    .jobs == $jobs and
    (.image | (
      (keys | sort) == ["amd64_digest","arm64_digest","repository","root_digest"] and
      .repository == $image_repo and
      ([.root_digest,.amd64_digest,.arm64_digest] | all(test("^sha256:[0-9a-f]{64}$"))) and
      ([.root_digest,.amd64_digest,.arm64_digest] | unique | length == 3)
    )) and
    (.artifacts | type == "array" and length == $count and map(.target) == $targets and all(.[]; .archive == ($app + "-" + .target + ".tar.gz") and .sidecar == (.archive + ".sha256") and (.archive_sha256 | test("^[0-9a-f]{64}$")) and (.sidecar_sha256 | test("^[0-9a-f]{64}$")))) and
    (.sboms.amd64.file == "spdx-amd64.json" and (.sboms.amd64.sha256 | test("^[0-9a-f]{64}$")) and .sboms.arm64.file == "spdx-arm64.json" and (.sboms.arm64.sha256 | test("^[0-9a-f]{64}$")))
  ' "$manifest" >/dev/null || fail "candidate final receipt contract changed"
  while IFS= read -r target; do
    archive="$(jq -er --arg t "$target" '.artifacts[] | select(.target == $t) | .archive' "$manifest")"
    sidecar_name="$(jq -er --arg t "$target" '.artifacts[] | select(.target == $t) | .sidecar' "$manifest")"
    require_file "$receipt_dir/$archive"; require_file "$receipt_dir/$sidecar_name"
    [[ "$(sha256_file "$receipt_dir/$archive")" == "$(jq -er --arg t "$target" '.artifacts[] | select(.target == $t) | .archive_sha256' "$manifest")" ]] || fail "candidate archive hash mismatch: $archive"
    [[ "$(sha256_file "$receipt_dir/$sidecar_name")" == "$(jq -er --arg t "$target" '.artifacts[] | select(.target == $t) | .sidecar_sha256' "$manifest")" ]] || fail "candidate checksum-sidecar hash mismatch: $sidecar_name"
  done < <(targets)
  for target in amd64 arm64; do
    require_file "$receipt_dir/spdx-${target}.json"
    [[ "$(sha256_file "$receipt_dir/spdx-${target}.json")" == "$(jq -er --arg t "$target" '.sboms[$t].sha256' "$manifest")" ]] || fail "candidate SPDX hash mismatch: $target"
    jq -e '.spdxVersion == "SPDX-2.3"' "$receipt_dir/spdx-${target}.json" >/dev/null || fail "candidate SPDX is invalid: $target"
  done
}

verify_archive_pair() {
  local dir="$1" target="$2" archive="${APP}-${2}.tar.gz" sidecar="${APP}-${2}.tar.gz.sha256" expected listed extra actual members mode
  require_file "$dir/$archive"; require_file "$dir/$sidecar"
  read -r expected listed extra <"$dir/$sidecar" || fail "cannot read release checksum: $sidecar"
  actual="$(sha256_file "$dir/$archive")"
  [[ "$expected" == "$actual" && "$listed" == "$archive" && -z "${extra:-}" ]] || fail "release checksum mismatch: $archive"
  members="$(tar -tzf "$dir/$archive" | LC_ALL=C sort)" || fail "cannot list release archive: $archive"
  [[ "$members" == "$(printf '%s\n' "${APP}-${target}/" "${APP}-${target}/README.md" "${APP}-${target}/${APP}" | LC_ALL=C sort)" ]] || fail "release archive members changed: $archive"
  mode="$(tar -tvzf "$dir/$archive" | awk -v path="${APP}-${target}/${APP}" '$NF == path { print $1 }')" || fail "cannot read release binary mode: $archive"
  [[ "$mode" =~ ^-.{2}x ]] || fail "release binary is not executable: $archive"
}

expected_asset_names() {
  local target
  { while IFS= read -r target; do printf '%s-%s.tar.gz\n%s-%s.tar.gz.sha256\n' "$APP" "$target" "$APP" "$target"; done < <(targets); printf 'spdx-amd64.json\nspdx-arm64.json\n%s\n%s.sha256\n' "$RECEIPT_NAME" "$RECEIPT_NAME"; } | LC_ALL=C sort
}

verify_release_assets_against_receipt() {
  local receipt_dir release_dir manifest expected actual target archive sidecar host private_home unpack binary version entry
  receipt_dir="$1"; release_dir="$2"; manifest="$receipt_dir/final-candidate-manifest.json"
  expected="$(expected_asset_names)"
  # `if`, not `&&`: a non-matching glob would leave the loop with status 1 and set -e would end the verifier silently.
  actual="$(for entry in "$release_dir"/* "$release_dir"/.[!.]*; do if [[ -f "$entry" ]]; then printf '%s\n' "${entry##*/}"; fi; done | LC_ALL=C sort)"
  [[ "$actual" == "$expected" ]] || fail "public release assets are not the exact receipt asset set"
  while IFS= read -r target; do
    archive="${APP}-${target}.tar.gz"
    sidecar="${archive}.sha256"
    verify_archive_pair "$release_dir" "$target"
    [[ "$(sha256_file "$release_dir/$archive")" == "$(jq -er --arg t "$target" '.artifacts[] | select(.target == $t) | .archive_sha256' "$manifest")" ]] || fail "public archive hash differs from final candidate receipt: $archive"
    [[ "$(sha256_file "$release_dir/$sidecar")" == "$(jq -er --arg t "$target" '.artifacts[] | select(.target == $t) | .sidecar_sha256' "$manifest")" ]] || fail "public checksum-sidecar hash differs from final candidate receipt: $sidecar"
  done < <(targets)
  for target in amd64 arm64; do
    jq -en --slurpfile candidate "$receipt_dir/spdx-${target}.json" --slurpfile release "$release_dir/spdx-${target}.json" '$candidate[0] == $release[0]' >/dev/null || fail "published SPDX bytes differ from candidate: $target"
  done
  verify_public_gke_receipt "$release_dir"
  host="$(host_archive_target)" || {
    printf 'note: %s ships no archive this verifier host (%s) can run; host binary check skipped\n' "$APP" "$(host_target)" >&2
    return 0
  }
  private_home="$(mktemp -d)"; unpack="$(mktemp -d)"
  trap 'rm -rf "${private_home:-}" "${unpack:-}"' RETURN
  tar -xzf "$release_dir/${APP}-${host}.tar.gz" -C "$unpack"
  binary="$unpack/${APP}-${host}/${APP}"
  [[ -x "$binary" ]] || fail "host binary is missing from public asset"
  version="$(env -i HOME="$private_home" PATH="$PATH" TMPDIR="$private_home" "$binary" --version)" || fail "public host binary cannot report version"
  [[ "$version" == "${APP} ${TAG#"${APP}"@}" ]] || fail "public host binary version mismatch: $version"
  [[ ! -e "$private_home/.aws" && ! -e "$private_home/.config/gcloud" ]] || fail "public host binary wrote credential state"
  rm -rf "$private_home" "$unpack"; trap - RETURN
}

verify_annotated_tag_and_ruleset() {
  local encoded_tag ref tag_object rulesets details id
  encoded_tag="${TAG/@/%40}"
  ref="$(gh api "repos/${REPO}/git/ref/tags/${encoded_tag}")"
  [[ "$(jq -r '.object.type' <<<"$ref")" == tag ]] || fail "release tag is not annotated"
  tag_object="$(gh api "repos/${REPO}/git/tags/$(jq -er '.object.sha' <<<"$ref")")"
  [[ "$(jq -r '.object.type' <<<"$tag_object")" == commit && "$(jq -r '.object.sha' <<<"$tag_object")" == "$COMMIT" ]] || fail "annotated tag does not peel to the exact candidate commit"
  rulesets='[]'
  while IFS= read -r id; do
    details="$(gh api "repos/${REPO}/rulesets/${id}")"
    rulesets="$(jq -c --argjson detail "$details" '. + [$detail]' <<<"$rulesets")"
  done < <(gh api --paginate "repos/${REPO}/rulesets?per_page=100" | jq -r '.[] | .id')
  jq -e --arg pattern "refs/tags/${APP}@*" '
    any(.[];
      .target == "tag" and .enforcement == "active" and
      (.conditions.ref_name.include == [$pattern]) and
      ((.conditions.ref_name.exclude // []) == []) and
      ([.rules[].type] | sort == ["deletion","update"]) and
      ((.bypass_actors // []) | length == 0))
  ' <<<"$rulesets" >/dev/null || fail "exact active immutable ${APP} tag ruleset is absent"
}

flatten_paginated_jobs() { jq -cs '[.[] | .jobs[]]'; }
flatten_paginated_artifacts() { jq -cs '[.[] | .artifacts[]]'; }
validate_candidate_job_inventory() {
  local names
  names="$(release_app_candidate_job_names "$APP" | jq -Rsc 'split("\n") | map(select(length > 0)) | sort')"
  jq -e --argjson names "$names" '
    length == ($names | length) and all(.[]; .status == "completed" and .conclusion == "success") and
    ([.[].name] | sort == $names)
  ' >/dev/null || fail "candidate attempt does not contain the exact successful execution set"
}

fetch_candidate_receipt() {
  local run attempt candidate_workflow_id jobs artifact_name artifacts artifact_id zip
  run="$(gh api "repos/${REPO}/actions/runs/${CANDIDATE_RUN_ID}")"
  attempt="$(jq -er '.run_attempt' <<<"$run")"
  [[ "$attempt" == "$CANDIDATE_RUN_ATTEMPT" ]] || fail "candidate run attempt changed"
  candidate_workflow_id="$(gh api "repos/${REPO}/actions/workflows/${APP}-release-candidate.yml" --jq '.id')"
  jq -e --arg commit "$COMMIT" --argjson workflow "$candidate_workflow_id" --arg repo "$REPO" '
    .event == "workflow_dispatch" and .status == "completed" and .conclusion == "success" and
    .head_branch == "main" and .head_sha == $commit and .workflow_id == $workflow and
    .head_repository.full_name == $repo
  ' <<<"$run" >/dev/null || fail "candidate run identity or conclusion changed"
  jobs="$(gh api --paginate "repos/${REPO}/actions/runs/${CANDIDATE_RUN_ID}/attempts/${attempt}/jobs?filter=latest&per_page=100" | flatten_paginated_jobs)"
  validate_candidate_job_inventory <<<"$jobs"
  artifact_name="${APP}-release-candidate-${CANDIDATE_RUN_ID}-${attempt}"
  artifacts="$(gh api --paginate "repos/${REPO}/actions/runs/${CANDIDATE_RUN_ID}/artifacts?per_page=100" | flatten_paginated_artifacts)"
  artifact_id="$(jq -er --arg name "$artifact_name" '[.[] | select(.name == $name and .expired == false)] | if length == 1 then .[0].id else error("exact receipt artifact absent") end' <<<"$artifacts")"
  zip="$(mktemp)"
  gh api -H 'Accept: application/vnd.github+json' "repos/${REPO}/actions/artifacts/${artifact_id}/zip" >"$zip"
  mkdir -p "$CANDIDATE_RECEIPT_DIR"
  unzip -q "$zip" -d "$CANDIDATE_RECEIPT_DIR"
  rm -f "$zip"
  CANDIDATE_ATTEMPT="$attempt"
  validate_receipt "$CANDIDATE_RECEIPT_DIR/final-candidate-manifest.json" "$CANDIDATE_RECEIPT_DIR/final-candidate-manifest.json.sha256" "$CANDIDATE_RECEIPT_DIR"
}

verify_candidate_supply_chain() {
  local manifest="$CANDIDATE_RECEIPT_DIR/final-candidate-manifest.json" root amd64 arm64 candidate_tag
  root="$(jq -er '.image.root_digest' "$manifest")"; amd64="$(jq -er '.image.amd64_digest' "$manifest")"; arm64="$(jq -er '.image.arm64_digest' "$manifest")"; candidate_tag="$(jq -er '.candidate_tag' "$manifest")"
  "$SCRIPT_DIR/verify-release-candidate.sh" --app "$APP" \
    --repo "$REPO" --version "${TAG#"${APP}"@}" --commit "$COMMIT" --run-id "$CANDIDATE_RUN_ID" --run-attempt "$CANDIDATE_ATTEMPT" \
    --manifest "$manifest" --manifest-sidecar "$CANDIDATE_RECEIPT_DIR/final-candidate-manifest.json.sha256" --artifacts-dir "$CANDIDATE_RECEIPT_DIR" \
    --image "${RELEASE_IMAGE_OWNER}/${APP}@${root}" --candidate-tag "$candidate_tag" --amd64-digest "$amd64" --arm64-digest "$arm64" --mode full
}

validate_gke_receipt() {
  local receipt="$GKE_RECEIPT" sidecar="$GKE_RECEIPT_SIDECAR" manifest actual bytes root amd64 arm64 backend functional gke_keys cleanup evidence_keys run_id_re
  [[ -n "$receipt" && -n "$sidecar" ]] || fail "$TAG requires the GKE receipt and sidecar"
  require_file "$receipt"; require_file "$sidecar"
  bytes="$(wc -c <"$receipt" | tr -d '[:space:]')"
  [[ "$bytes" =~ ^[0-9]+$ && "$bytes" -gt 0 && "$bytes" -le 32768 ]] || fail "GKE receipt size is invalid"
  actual="$(sha256_file "$receipt")"
  [[ "$actual" =~ ^[0-9a-f]{64}$ ]] || fail "GKE receipt hash is invalid"
  cmp -s "$sidecar" <(printf '%s  %s\n' "$actual" "$RECEIPT_NAME") || fail "GKE receipt sidecar is not exact"
  manifest="$CANDIDATE_RECEIPT_DIR/final-candidate-manifest.json"
  root="$(jq -er '.image.root_digest' "$manifest")"
  amd64="$(jq -er '.image.amd64_digest' "$manifest")"
  arm64="$(jq -er '.image.arm64_digest' "$manifest")"
  backend="$(release_app_gke_backend "$APP")"
  functional="$(release_app_functional_fields "$APP" | jq -Rn '[inputs | select(length > 0)] | map({key: ., value: "passed"}) | from_entries')"
  case "$backend" in
    gcp)
      gke_keys='["backend","cleanup","functional","image","image_provenance","run_id"]'
      cleanup='{"schema":"axiom.gcp.operator.cleanup.v1","status":"clean","preserved":{"artifact_registry":true,"preexisting_apis":true}}'
      evidence_keys='["acceptance_sha256","cleanup_sha256","images_sha256","run_sha256"]'
      run_id_re='^[a-z0-9][a-z0-9-]{0,39}$' ;;
    gke-acceptance)
      gke_keys='["backend","cleanup","functional","image","image_provenance","run_id","run_url"]'
      cleanup='{"schema":"axiom.gke-harness.cleanup.v1","status":"clean","namespace_deleted":true,"node_pool_parked":true}'
      evidence_keys='["gh_run_sha256","manifests_sha256","results_sha256","verdict_sha256"]'
      run_id_re='^gha-[0-9]+-[0-9]+$' ;;
    *) fail "no receipt contract for backend $backend" ;;
  esac
  jq -e --arg app "$APP" --arg repo "$REPO" --arg version "${TAG#"${APP}"@}" --arg manifest_sha "$(sha256_file "$manifest")" --arg commit "$COMMIT" --arg run "$CANDIDATE_RUN_ID" --arg attempt "$CANDIDATE_ATTEMPT" --arg root "$root" --arg amd64 "$amd64" --arg arm64 "$arm64" --arg schema "$(release_app_receipt_schema "$APP")" --arg image "${RELEASE_IMAGE_OWNER}/${APP}@${root}" --arg backend "$backend" --arg run_id_re "$run_id_re" --argjson functional "$functional" --argjson gke_keys "$gke_keys" --argjson cleanup "$cleanup" --argjson evidence_keys "$evidence_keys" '
    (keys | sort) == ["candidate","complete","evidence","gke","redaction","result","schema"] and
    .schema == $schema and .complete == true and .result == "passed" and
    (.candidate | (keys | sort) == ["amd64_digest","arm64_digest","commit","manifest_sha256","repository","root_digest","run_attempt","run_id","version","workflow_ref"]) and
    .candidate.repository == $repo and .candidate.version == $version and .candidate.commit == $commit and
    .candidate.workflow_ref == ($repo + "/.github/workflows/" + $app + "-release-candidate.yml@refs/heads/main") and
    .candidate.run_id == $run and .candidate.run_attempt == $attempt and .candidate.manifest_sha256 == $manifest_sha and
    .candidate.root_digest == $root and .candidate.amd64_digest == $amd64 and .candidate.arm64_digest == $arm64 and
    (.gke | (keys | sort) == $gke_keys) and .gke.backend == $backend and
    (.gke.run_id | type == "string" and test($run_id_re)) and
    (if $backend == "gke-acceptance" then
       (.gke.run_id | capture("^gha-(?<run>[0-9]+)-[0-9]+$").run) as $gha_run |
       .gke.run_url == ("https://github.com/" + $repo + "/actions/runs/" + $gha_run)
     else true end) and
    .gke.image == $image and .gke.image_provenance == "prebuilt" and
    .gke.functional == $functional and
    .gke.cleanup == $cleanup and
    (.evidence | (keys | sort) == $evidence_keys and all(.[]; type == "string" and test("^[0-9a-f]{64}$"))) and
    .redaction == {kubeconfig_retained:false,token_retained:false,secret_retained:false,cluster_identity_retained:false,command_output_retained:false}
  ' "$receipt" >/dev/null || fail "GKE receipt contract changed"
  GKE_RECEIPT_SHA256="$actual"
  GKE_RECEIPT_SIDECAR_SHA256="$(sha256_file "$sidecar")"
}

write_identity() {
  local manifest="$CANDIDATE_RECEIPT_DIR/final-candidate-manifest.json" identity
  identity="$(jq -nc --arg repo "$REPO" --arg tag "$TAG" --arg commit "$COMMIT" --arg candidate_run_id "$CANDIDATE_RUN_ID" --arg candidate_attempt "$CANDIDATE_ATTEMPT" --arg root "$(jq -er '.image.root_digest' "$manifest")" --arg amd64 "$(jq -er '.image.amd64_digest' "$manifest")" --arg arm64 "$(jq -er '.image.arm64_digest' "$manifest")" --arg pr "$(jq -er '.pr.url' "$manifest")" --arg candidate_url "$(jq -er '.run_url' "$manifest")" '{repository:$repo,tag:$tag,commit:$commit,candidate_run_id:$candidate_run_id,candidate_attempt:$candidate_attempt,root_digest:$root,amd64_digest:$amd64,arm64_digest:$arm64,pr_url:$pr,candidate_url:$candidate_url}')"
  jq -nc --argjson identity "$identity" --arg receipt "$GKE_RECEIPT_SHA256" --arg sidecar "$GKE_RECEIPT_SIDECAR_SHA256" '$identity + {gke_receipt_sha256:$receipt,gke_receipt_sidecar_sha256:$sidecar}' >"$OUTPUT"
}

verify_public_gke_receipt() {
  local release_dir="$1" public_receipt public_sidecar
  public_receipt="$release_dir/$RECEIPT_NAME"
  public_sidecar="$release_dir/$RECEIPT_NAME.sha256"
  require_file "$public_receipt"; require_file "$public_sidecar"
  cmp -s "$GKE_RECEIPT" "$public_receipt" || fail "public GKE receipt bytes differ from verified receipt"
  cmp -s "$GKE_RECEIPT_SIDECAR" "$public_sidecar" || fail "public GKE receipt sidecar differs from verified receipt"
}

verify_latest_is_safe() {
  local root="$1" image_repo="${RELEASE_IMAGE_OWNER}/${APP}" latest releases tag version digest
  latest="$(image_digest_or_absent "${image_repo}:latest")"
  [[ -n "$latest" ]] || fail "public latest image tag is absent"
  [[ "$latest" == "$root" ]] && return 0
  releases="$(gh release list --repo "$REPO" --limit 100 --json tagName,isDraft)"
  while IFS= read -r tag; do
    version="${tag#"${APP}"@}"
    [[ "$tag" =~ ^${APP}@[0-9]+\.[0-9]+\.[0-9]+$ ]] || continue
    [[ "$(printf '%s\n%s\n' "${TAG#"${APP}"@}" "$version" | sort -V | tail -n1)" == "$version" && "$version" != "${TAG#"${APP}"@}" ]] || continue
    digest="$(image_digest_or_absent "${image_repo}:${version}")"
    [[ -n "$digest" ]] || fail "newer published semver image tag is absent: $version"
    [[ "$digest" == "$latest" ]] && return 0
  done < <(jq -r '.[] | select(.isDraft == false) | .tagName' <<<"$releases")
  fail "latest points to neither this root nor a newer published semver root"
}

verify_public_release_notes() {
  local receipt_sha256="$1" sidecar_sha256="$2" release_json
  release_json="$(cat)"
  jq -e --arg receipt "$receipt_sha256" --arg sidecar "$sidecar_sha256" --arg compatibility "$RELEASE_COMPATIBILITY_LINE" '
    (.body | if type == "string" then split("\n") else error("release body is not a string") end) as $lines |
    ([ $lines[] | select(. == ("- GKE receipt SHA-256: " + $receipt)) ] | length) == 1 and
    ([ $lines[] | select(. == ("- GKE receipt sidecar SHA-256: " + $sidecar)) ] | length) == 1 and
    ([ $lines[] | select(. == $compatibility) ] | length) == 1 and
    ([$lines[] | sub("^\\s+"; "")] | all(.[]; (startswith(">") | not) and (startswith("```") | not) and (startswith("~~~") | not) and (contains("<!--") | not)))
  ' <<<"$release_json" >/dev/null || fail "public GitHub Release notes do not bind exact ${APP} compatibility and GKE evidence"
}

verify_public_release() {
  local release_json release_dir manifest root amd64 arm64 pr_url candidate_url semver image_repo expected_assets actual_assets
  release_json="$(gh release view "$TAG" --repo "$REPO" --json assets,isDraft,tagName,targetCommitish,url,body)"
  jq -e --arg tag "$TAG" --arg commit "$COMMIT" '.tagName == $tag and .isDraft == false and .targetCommitish == $commit' <<<"$release_json" >/dev/null || fail "public GitHub Release identity changed"
  verify_public_release_notes "$GKE_RECEIPT_SHA256" "$GKE_RECEIPT_SIDECAR_SHA256" <<<"$release_json"
  manifest="$CANDIDATE_RECEIPT_DIR/final-candidate-manifest.json"
  root="$(jq -er '.image.root_digest' "$manifest")"; amd64="$(jq -er '.image.amd64_digest' "$manifest")"; arm64="$(jq -er '.image.arm64_digest' "$manifest")"; pr_url="$(jq -er '.pr.url' "$manifest")"; candidate_url="$(jq -er '.run_url' "$manifest")"
  jq -e --arg repo "$REPO" --arg tag "$TAG" --arg commit "$COMMIT" --arg pr_url "$pr_url" --arg candidate_url "$candidate_url" --arg root "$root" --arg amd64 "$amd64" --arg arm64 "$arm64" --arg receipt_sha256 "$GKE_RECEIPT_SHA256" --arg sidecar_sha256 "$GKE_RECEIPT_SIDECAR_SHA256" --arg compatibility "$RELEASE_COMPATIBILITY_LINE" '
    (.body | if type == "string" then split("\n") else error("release body is not a string") end) as $lines |
    ($lines | index("- Source commit: " + $commit) != null) and
    ($lines | index("- Pull request: " + $pr_url) != null) and
    ($lines | index("- Candidate run: " + $candidate_url) != null) and
    ($lines | index("- Root index digest: " + $root) != null) and
    ($lines | index("- linux/amd64 digest: " + $amd64) != null) and
    ($lines | index("- linux/arm64 digest: " + $arm64) != null) and
    ($lines | index("- GKE receipt SHA-256: " + $receipt_sha256) != null) and
    ($lines | index("- GKE receipt sidecar SHA-256: " + $sidecar_sha256) != null) and
    ($lines | index("- Release path: landed main -> immutable candidate -> digest-pinned GKE acceptance -> protected annotated tag -> no-rebuild promotion.") != null) and
    ($lines | index($compatibility) != null) and
    any($lines[]; test("^- Promotion run: https://github\\.com/" + $repo + "/actions/runs/[0-9]+/attempts/[0-9]+$"))
  ' <<<"$release_json" >/dev/null || fail "public GitHub Release notes do not bind exact promotion evidence"
  expected_assets="$(expected_asset_names)"
  actual_assets="$(jq -r '.assets[].name' <<<"$release_json" | LC_ALL=C sort)"
  [[ "$actual_assets" == "$expected_assets" ]] || fail "public GitHub Release asset inventory is not exact"
  release_dir="$(mktemp -d)"; trap 'rm -rf "${release_dir:-}"' RETURN
  gh release download "$TAG" --repo "$REPO" --dir "$release_dir" --pattern "${APP}-*.tar.gz" --pattern "${APP}-*.tar.gz.sha256" --pattern 'spdx-*.json' --pattern "$RECEIPT_NAME" --pattern "$RECEIPT_NAME.sha256"
  verify_release_assets_against_receipt "$CANDIDATE_RECEIPT_DIR" "$release_dir"
  semver="${TAG#"${APP}"@}"; image_repo="${RELEASE_IMAGE_OWNER}/${APP}"
  [[ "$(docker buildx imagetools inspect "${image_repo}:${semver}" --format '{{json .Manifest}}' | jq -er '.digest')" == "$root" ]] || fail "semver image tag does not bind candidate root"
  verify_latest_is_safe "$root"
  rm -rf "$release_dir"; trap - RETURN
}

if [[ "${BASH_SOURCE[0]}" != "$0" ]]; then return 0; fi

APP=""; REPO=""; TAG=""; COMMIT=""; CANDIDATE_RUN_ID=""; CANDIDATE_RUN_ATTEMPT=""; MODE=""; CANDIDATE_RECEIPT_DIR=""; RELEASE_ASSETS_DIR=""; OUTPUT=""; GKE_RECEIPT=""; GKE_RECEIPT_SIDECAR=""; GKE_RECEIPT_SHA256=""; GKE_RECEIPT_SIDECAR_SHA256=""
while [[ $# -gt 0 ]]; do
  [[ $# -ge 2 ]] || usage
  case "$1" in
    --app) APP="$2" ;; --repo) REPO="$2" ;; --tag) TAG="$2" ;; --commit) COMMIT="$2" ;; --candidate-run-id) CANDIDATE_RUN_ID="$2" ;; --candidate-run-attempt) CANDIDATE_RUN_ATTEMPT="$2" ;;
    --mode) MODE="$2" ;; --candidate-receipt-dir) CANDIDATE_RECEIPT_DIR="$2" ;; --release-assets-dir) RELEASE_ASSETS_DIR="$2" ;; --output) OUTPUT="$2" ;;
    --gke-receipt) GKE_RECEIPT="$2" ;; --gke-receipt-sidecar) GKE_RECEIPT_SIDECAR="$2" ;;
    *) usage ;;
  esac
  shift 2
done
[[ -n "$APP" ]] || usage
release_app_require "$APP"
release_app_uses_shared_scripts "$APP" || fail "$APP verifies with apps/$APP/scripts/verify-release-artifacts.sh, not the shared verifier"
RECEIPT_NAME="$(release_app_receipt_name "$APP")"
[[ "$REPO" == "$RELEASE_REPO" && "$TAG" =~ ^${APP}@[0-9]+\.[0-9]+\.[0-9]+$ && "$COMMIT" =~ ^[0-9a-f]{40}$ && "$CANDIDATE_RUN_ID" =~ ^[0-9]+$ && "$CANDIDATE_RUN_ATTEMPT" =~ ^[0-9]+$ ]] || fail "invalid promotion identity"
[[ "$MODE" == candidate || "$MODE" == fixture || "$MODE" == public ]] || usage

if [[ "$MODE" == fixture ]]; then
  [[ -n "$CANDIDATE_RECEIPT_DIR" && -n "$RELEASE_ASSETS_DIR" ]] || usage
  validate_receipt "$CANDIDATE_RECEIPT_DIR/final-candidate-manifest.json" "$CANDIDATE_RECEIPT_DIR/final-candidate-manifest.json.sha256" "$CANDIDATE_RECEIPT_DIR"
  CANDIDATE_ATTEMPT="$CANDIDATE_RUN_ATTEMPT"
  validate_gke_receipt
  verify_release_assets_against_receipt "$CANDIDATE_RECEIPT_DIR" "$RELEASE_ASSETS_DIR"
  printf 'LOCAL FIXTURE ONLY: release bytes verified; this is not public release acceptance.\n'
  exit 0
fi

[[ -n "$OUTPUT" ]] || usage
CANDIDATE_RECEIPT_DIR="$(mktemp -d)"; trap 'rm -rf "${CANDIDATE_RECEIPT_DIR:-}"' EXIT
verify_annotated_tag_and_ruleset
fetch_candidate_receipt
verify_candidate_supply_chain
validate_gke_receipt
write_identity
if [[ "$MODE" == public ]]; then verify_public_release; fi
printf 'PROMOTION VERIFICATION PASS: %s %s candidate=%s/%s\n' "$TAG" "$COMMIT" "$CANDIDATE_RUN_ID" "$CANDIDATE_ATTEMPT"
