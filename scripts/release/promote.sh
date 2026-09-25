#!/usr/bin/env bash
# build-release step 7 for one app: bind the GKE receipt to the verified
# candidate, create and push the one annotated <app>@<version> tag at the
# candidate's exact commit, dispatch <app>-release.yml at that tag, and watch
# it. The promotion workflow re-proves tag, ruleset, candidate run, receipt,
# signature, provenance, and SBOM attestations before any public write; this
# script never rebuilds, never re-signs, and never moves or deletes a tag.
#
# usage: scripts/release/promote.sh <app> <version> <candidate-run-id> <candidate-run-attempt> <receipt> [--out <dir>]
#   receipt  the GKE receipt (named as scripts/release/apps.sh dictates) with
#            its <receipt>.sha256 sidecar next to it
#   --out    directory for the public-verifier output (default
#            ${TMPDIR:-/tmp}/<app>-release-<version>)
# exit: 0 promotion run concluded success; 1 run red; 2 refused.
# The watch takes 5-20 minutes; run it in the background from a session.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/release/apps.sh
source "$SCRIPT_DIR/apps.sh"
GIT=(git -c core.fsmonitor=false)
sha256_file() { if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'; else shasum -a 256 "$1" | awk '{print $1}'; fi; }

out=''
positional=()
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out=${2:?--out needs a directory}; shift 2 ;;
    -*) release_refuse "unknown flag $1" ;;
    *) positional+=("$1"); shift ;;
  esac
done
[ "${#positional[@]}" -eq 5 ] || release_refuse "usage: scripts/release/promote.sh <app> <version> <candidate-run-id> <candidate-run-attempt> <receipt> [--out <dir>]"
app=${positional[0]}; version=${positional[1]}; run_id=${positional[2]}; attempt=${positional[3]}; receipt=${positional[4]}
release_app_require "$app"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || release_refuse "version must be X.Y.Z, got $version"
[[ "$run_id" =~ ^[0-9]+$ && "$attempt" =~ ^[0-9]+$ ]] || release_refuse "candidate run id and attempt must be decimal"
for tool in gh jq base64; do command -v "$tool" >/dev/null || release_refuse "$tool is required"; done

tag="${app}@${version}"
workflow="${app}-release.yml"
receipt_name="$(release_app_receipt_name "$app")"
sidecar="${receipt}.sha256"
{ [ -f "$receipt" ] && [ ! -L "$receipt" ]; } || release_refuse "receipt is not a regular file: $receipt"
[ "$(basename "$receipt")" = "$receipt_name" ] || release_refuse "receipt must be named $receipt_name, got $(basename "$receipt")"
{ [ -f "$sidecar" ] && [ ! -L "$sidecar" ]; } || release_refuse "receipt sidecar is absent: $sidecar"
receipt_sha=$(sha256_file "$receipt")
sidecar_sha=$(sha256_file "$sidecar")
cmp -s "$sidecar" <(printf '%s  %s\n' "$receipt_sha" "$receipt_name") || release_refuse "sidecar does not bind the receipt bytes: $sidecar"
receipt_bytes=$(wc -c <"$receipt" | tr -d '[:space:]')
[ "$receipt_bytes" -le 32768 ] || release_refuse "receipt exceeds 32768 bytes"

dirty=$("${GIT[@]}" status --porcelain)
[ -z "$dirty" ] || { printf 'refused: dirty tree — promotion tags a landed commit, not this tree\n%s\n' "$dirty" >&2; exit 2; }
branch=$("${GIT[@]}" branch --show-current)
[ "$branch" = main ] || release_refuse "promotion runs from main, not from '${branch:-detached HEAD}'"
"${GIT[@]}" fetch --quiet origin main

# --- candidate identity from GitHub, never from the caller ----------------
run_json=$(gh api "repos/${RELEASE_REPO}/actions/runs/${run_id}")
jq -e --arg attempt "$attempt" --arg path ".github/workflows/${app}-release-candidate.yml" '
  (.run_attempt | tostring) == $attempt and .event == "workflow_dispatch" and .status == "completed" and
  .conclusion == "success" and .head_branch == "main" and .path == $path
' <<<"$run_json" >/dev/null || release_refuse "run $run_id attempt $attempt is not a successful ${app}-release-candidate dispatch from main"
commit=$(jq -r '.head_sha' <<<"$run_json")
[[ "$commit" =~ ^[0-9a-f]{40}$ ]] || release_refuse "candidate run has no head sha"
"${GIT[@]}" merge-base --is-ancestor "$commit" origin/main || release_refuse "candidate commit $commit is not on origin/main"

bundle_dir=$(mktemp -d)
trap 'rm -rf "$bundle_dir"' EXIT
gh run download "$run_id" --repo "$RELEASE_REPO" -n "${app}-release-candidate-${run_id}-${attempt}" -D "$bundle_dir" \
  || release_refuse "candidate bundle ${app}-release-candidate-${run_id}-${attempt} cannot be downloaded (expired?)"
manifest="$bundle_dir/final-candidate-manifest.json"
[ -f "$manifest" ] || release_refuse "candidate bundle has no final-candidate-manifest.json"
cmp -s "$manifest.sha256" <(printf '%s  final-candidate-manifest.json\n' "$(sha256_file "$manifest")") || release_refuse "candidate manifest sidecar does not bind its bytes"
jq -e --arg version "$version" --arg commit "$commit" --arg run "$run_id" --arg attempt "$attempt" --arg tag "$tag" \
  '.version == $version and .commit == $commit and .run_id == $run and .run_attempt == $attempt and .tag == $tag' "$manifest" >/dev/null \
  || release_refuse "final manifest does not bind $tag $commit run $run_id attempt $attempt"
root_digest=$(jq -r '.image.root_digest' "$manifest")
image="${RELEASE_IMAGE_OWNER}/${app}@${root_digest}"

# --- receipt binds this exact candidate ------------------------------------
jq -e --arg schema "$(release_app_receipt_schema "$app")" --arg version "$version" --arg commit "$commit" --arg run "$run_id" --arg attempt "$attempt" --arg root "$root_digest" --arg manifest_sha "$(sha256_file "$manifest")" '
  .schema == $schema and .complete == true and
  .candidate.version == $version and .candidate.commit == $commit and
  .candidate.run_id == $run and .candidate.run_attempt == $attempt and
  .candidate.root_digest == $root and .candidate.manifest_sha256 == $manifest_sha
' "$receipt" >/dev/null || release_refuse "receipt does not bind candidate $tag $commit run $run_id attempt $attempt root $root_digest"
if release_app_uses_shared_scripts "$app"; then
  jq -e --arg image "$image" '.result == "passed" and .gke.image == $image and .gke.image_provenance == "prebuilt"' "$receipt" >/dev/null \
    || release_refuse "receipt is not a passed run of the candidate image $image"
fi

# --- tag protection, then the one tag ---------------------------------------
if gh release view "$tag" --repo "$RELEASE_REPO" --json tagName >/dev/null 2>&1; then
  release_refuse "GitHub Release $tag already exists; run the public verifier instead of promoting again"
fi
rulesets='[]'
while IFS= read -r id; do
  rulesets=$(jq -c --argjson detail "$(gh api "repos/${RELEASE_REPO}/rulesets/${id}")" '. + [$detail]' <<<"$rulesets")
done < <(gh api --paginate "repos/${RELEASE_REPO}/rulesets?per_page=100" | jq -r '.[] | .id')
jq -e --arg pattern "refs/tags/${app}@*" '
  any(.[]; .target == "tag" and .enforcement == "active" and
           (.conditions.ref_name.include == [$pattern]) and ((.conditions.ref_name.exclude // []) == []) and
           ([.rules[].type] | sort == ["deletion","update"]) and ((.bypass_actors // []) | length == 0))
' <<<"$rulesets" >/dev/null || release_refuse "no active tag ruleset protects refs/tags/${app}@* (update + deletion, no bypass); create it before promoting"

remote_tag=$("${GIT[@]}" ls-remote --tags origin "refs/tags/$tag" "refs/tags/$tag^{}")
if [ -n "$remote_tag" ]; then
  peeled=$(grep -F "refs/tags/$tag^{}" <<<"$remote_tag" | cut -f1 || true)
  [ -n "$peeled" ] || release_refuse "tag $tag exists on origin but is not annotated; it cannot be repaired by this script"
  [ "$peeled" = "$commit" ] || release_refuse "tag $tag exists on origin at $peeled, not $commit — a release tag is never moved"
  echo "tag: $tag already on origin at $commit (rerun of the same identity)"
else
  if "${GIT[@]}" rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
    local_type=$("${GIT[@]}" cat-file -t "refs/tags/$tag")
    local_peel=$("${GIT[@]}" rev-parse "refs/tags/$tag^{commit}")
    { [ "$local_type" = tag ] && [ "$local_peel" = "$commit" ]; } || release_refuse "local tag $tag exists but is not an annotated tag at $commit; inspect and remove it by hand"
  else
    "${GIT[@]}" tag -a "$tag" "$commit" -m "Release $tag"
  fi
  "${GIT[@]}" push origin "refs/tags/$tag"
  echo "tag: $tag pushed at $commit"
fi

# --- promotion dispatch at the tag ------------------------------------------
prefix=$(release_app_promotion_input_prefix "$app")
inputs=(-f version="$version" -f candidate_run_id="$run_id")
if release_app_promotion_takes_attempt "$app"; then inputs+=(-f candidate_run_attempt="$attempt"); fi
inputs+=(
  -f "${prefix}_b64=$(base64 <"$receipt" | tr -d '\n')"
  -f "${prefix}_sha256=$receipt_sha"
  -f "${prefix}_sidecar_b64=$(base64 <"$sidecar" | tr -d '\n')"
  -f "${prefix}_sidecar_sha256=$sidecar_sha"
)
prior_json=$(gh run list --repo "$RELEASE_REPO" --workflow "$workflow" -L 50 --json databaseId,headSha,event)
before_ids=$(jq -c --arg sha "$commit" '[.[] | select(.headSha == $sha and .event == "workflow_dispatch") | .databaseId]' <<<"$prior_json")
gh workflow run "$workflow" --repo "$RELEASE_REPO" --ref "$tag" "${inputs[@]}"

promotion_id=''
for _ in $(seq 1 18); do
  sleep 5
  promotion_id=$(gh run list --repo "$RELEASE_REPO" --workflow "$workflow" -L 20 --json databaseId,headSha,event,createdAt \
    | jq -r --arg sha "$commit" --argjson before "$before_ids" \
        '[.[] | select(.headSha == $sha and .event == "workflow_dispatch"
                       and ((.databaseId as $id | $before | index($id)) == null))]
         | sort_by(.createdAt) | last | .databaseId // empty')
  [ -n "$promotion_id" ] && break
done
[ -n "$promotion_id" ] || { echo "error: dispatched, but no new $workflow run for $commit appeared within 90s; inspect: gh run list --workflow $workflow" >&2; exit 1; }
url=$(gh run view "$promotion_id" --repo "$RELEASE_REPO" --json url --jq .url)
echo "promotion run: $url"
gh run watch "$promotion_id" --repo "$RELEASE_REPO" --interval 30 --exit-status || true
conclusion=$(gh run view "$promotion_id" --repo "$RELEASE_REPO" --json conclusion --jq .conclusion)
echo "conclusion: $conclusion"

out=${out:-${TMPDIR:-/tmp}/${app}-release-${version}}
mkdir -p "$out"
verifier="$(release_app_scripts_dir "$app")/verify-release-artifacts.sh"
case "$app" in
  tape)
    next="$verifier --repo $RELEASE_REPO --tag $tag --commit $commit --candidate-run-id $run_id --candidate-run-attempt $attempt --mode public --gke-receipt $receipt --gke-receipt-sidecar $sidecar --output $out/public-contract.json" ;;
  *)
    next="$verifier --app $app --repo $RELEASE_REPO --tag $tag --commit $commit --candidate-run-id $run_id --candidate-run-attempt $attempt --mode public --gke-receipt $receipt --gke-receipt-sidecar $sidecar --output $out/public-contract.json" ;;
esac
echo "next: $next"
[ "$conclusion" = success ] || exit 1
