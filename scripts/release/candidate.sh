#!/usr/bin/env bash
# build-release step 3 for one app: dispatch <app>-release-candidate.yml from
# main at the exact landed commit, watch it to a terminal state, and download
# the run-scoped candidate bundle. This script never tags, never promotes,
# and never rebuilds; the candidate workflow owns build, image, attestation,
# and the final manifest.
#
# usage: scripts/release/candidate.sh <app> <version> <commit> [--out <dir>]
#   app      one of: tape sift keep relay defer
#   version  X.Y.Z, equal to <root>/Cargo.toml at <commit>
#   commit   40-hex sha; must be the head of origin/main (the workflow's
#            identity job requires the dispatched commit to equal GITHUB_SHA)
#   --out    bundle download directory
#            (default ${TMPDIR:-/tmp}/<app>-release-candidate-<run>-<attempt>)
# exit: 0 run concluded success and the final manifest binds this identity;
#   1 run red, bundle absent, or manifest mismatch; 2 refused.
# The watch takes 20-60 minutes; run it in the background from a session.
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/release/apps.sh
source "$SCRIPT_DIR/apps.sh"
GIT=(git -c core.fsmonitor=false)

out=''
positional=()
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out=${2:?--out needs a directory}; shift 2 ;;
    -*) release_refuse "unknown flag $1" ;;
    *) positional+=("$1"); shift ;;
  esac
done
[ "${#positional[@]}" -eq 3 ] || release_refuse "usage: scripts/release/candidate.sh <app> <version> <commit> [--out <dir>]"
app=${positional[0]}; version=${positional[1]}; commit=${positional[2]}
release_app_require "$app"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || release_refuse "version must be X.Y.Z, got $version"
[[ "$commit" =~ ^[0-9a-f]{40}$ ]] || release_refuse "commit must be a 40-hex sha, got $commit"
for tool in gh jq; do command -v "$tool" >/dev/null || release_refuse "$tool is required"; done

workflow="${app}-release-candidate.yml"
root="$(release_app_root "$app")"
tag="${app}@${version}"

dirty=$("${GIT[@]}" status --porcelain)
[ -z "$dirty" ] || { printf 'refused: dirty tree — the candidate proves a landed commit, not this tree; land or discard first\n%s\n' "$dirty" >&2; exit 2; }
"${GIT[@]}" fetch --quiet origin main
origin_main=$("${GIT[@]}" rev-parse origin/main)
[ "$origin_main" = "$commit" ] || release_refuse "origin/main is $origin_main, not $commit — the candidate identity job requires the dispatched commit to be the head of main; land first, then pass that head"
manifest_line=$("${GIT[@]}" show "${commit}:${root}/Cargo.toml" | sed -n '/^\[package\]/,/^\[/p' | grep -m1 '^version' || true)
manifest_version=$(sed -E 's/^version *= *"([^"]+)".*/\1/' <<<"$manifest_line")
[ "$manifest_version" = "$version" ] || release_refuse "${root}/Cargo.toml at $commit says '${manifest_line:-<no version line>}', not version $version"
[ -z "$("${GIT[@]}" ls-remote --tags origin "refs/tags/$tag")" ] || release_refuse "tag $tag already exists on origin — a candidate is dispatched before the tag, never after"
if gh release view "$tag" --repo "$RELEASE_REPO" --json tagName >/dev/null 2>&1; then
  release_refuse "GitHub Release $tag already exists"
fi
gh workflow view "$workflow" --repo "$RELEASE_REPO" --yaml >/dev/null 2>&1 || release_refuse "$workflow is not on the default branch yet — land it on main before dispatching"

prior_json=$(gh run list --repo "$RELEASE_REPO" --workflow "$workflow" -L 50 --json databaseId,headSha,event)
before_ids=$(jq -c --arg sha "$commit" '[.[] | select(.headSha == $sha and .event == "workflow_dispatch") | .databaseId]' <<<"$prior_json")

gh workflow run "$workflow" --repo "$RELEASE_REPO" --ref main -f version="$version" -f commit="$commit"

# gh workflow run prints no run id; the new run is the one for this sha that
# was not in the list before dispatch.
run_id=''
for _ in $(seq 1 18); do
  sleep 5
  run_id=$(gh run list --repo "$RELEASE_REPO" --workflow "$workflow" --branch main -L 20 \
      --json databaseId,headSha,event,createdAt \
    | jq -r --arg sha "$commit" --argjson before "$before_ids" \
        '[.[] | select(.headSha == $sha and .event == "workflow_dispatch"
                       and ((.databaseId as $id | $before | index($id)) == null))]
         | sort_by(.createdAt) | last | .databaseId // empty')
  [ -n "$run_id" ] && break
done
[ -n "$run_id" ] || { echo "error: dispatched, but no new $workflow run for $commit appeared within 90s; inspect: gh run list --workflow $workflow" >&2; exit 1; }
url=$(gh run view "$run_id" --repo "$RELEASE_REPO" --json url --jq .url)
echo "candidate run: $url"

# The gate is the read-back below, not the watch's exit code.
gh run watch "$run_id" --repo "$RELEASE_REPO" --interval 30 --exit-status || true
view=$(gh run view "$run_id" --repo "$RELEASE_REPO" --json conclusion,attempt)
conclusion=$(jq -r '.conclusion' <<<"$view")
attempt=$(jq -r '.attempt' <<<"$view")
echo "conclusion: $conclusion"
echo "attempt: $attempt"

out=${out:-${TMPDIR:-/tmp}/${app}-release-candidate-${run_id}-${attempt}}
mkdir -p "$out"
bundle="${app}-release-candidate-${run_id}-${attempt}"
if ! gh run download "$run_id" --repo "$RELEASE_REPO" -n "$bundle" -D "$out" 2>/dev/null; then
  echo "bundle: none (artifact $bundle is absent)"
  exit 1
fi
echo "bundle: $out"
[ "$conclusion" = success ] || exit 1
manifest="$out/final-candidate-manifest.json"
[ -f "$manifest" ] || { echo "error: $manifest is absent from the bundle" >&2; exit 1; }
jq -e --arg version "$version" --arg commit "$commit" --arg run "$run_id" --arg attempt "$attempt" \
  '.version == $version and .commit == $commit and .run_id == $run and .run_attempt == $attempt' "$manifest" >/dev/null \
  || { echo "error: final manifest does not bind $version $commit run $run_id attempt $attempt" >&2; exit 1; }
root_digest=$(jq -r '.image.root_digest' "$manifest")
amd64=$(jq -r '.image.amd64_digest' "$manifest")
arm64=$(jq -r '.image.arm64_digest' "$manifest")
image="${RELEASE_IMAGE_OWNER}/${app}@${root_digest}"
echo "root digest: $root_digest"
echo "image: $image"
verifier="$(release_app_scripts_dir "$app")/verify-release-candidate.sh"
app_flag=''
if release_app_uses_shared_scripts "$app"; then app_flag="--app $app "; fi
echo "next: ${verifier} ${app_flag}--repo $RELEASE_REPO --version $version --commit $commit --run-id $run_id --run-attempt $attempt --manifest $manifest --manifest-sidecar $manifest.sha256 --artifacts-dir $out --image $image --candidate-tag release-candidate-${run_id}-${attempt} --amd64-digest $amd64 --arm64-digest $arm64 --mode full"
