#!/usr/bin/env bash
# Per-app release table shared by scripts/release/*.sh. Source it; never run it.
#
# Five apps ride the candidate-first release path behind `build-release <app>`:
# landed main -> immutable candidate -> digest-pinned GKE acceptance ->
# protected annotated tag -> no-rebuild promotion. tape keeps its own
# verifiers under apps/tape/scripts; sift, keep, relay, and defer use the
# --app parameterized twins in this directory. Every function prints one
# value per line and refuses an app it does not know, so a typo cannot fall
# through to an empty target list.
RELEASE_REPO="chrischeng-c4/axiom"
RELEASE_IMAGE_OWNER="ghcr.io/chrischeng-c4"
RELEASE_COMPATIBILITY_LINE="- Compatibility: no HTTP, CLI, wire format, on-disk format, or Kubernetes manifest shape changed in this release."

release_refuse() { printf 'refused: %s\n' "$*" >&2; exit 2; }

release_app_known() {
  case "$1" in tape|sift|keep|relay|defer) return 0 ;; *) return 1 ;; esac
}
release_app_require() {
  release_app_known "${1:-}" || release_refuse "unknown release app: ${1:-<none>} (known: tape sift keep relay defer)"
}
# Where the app's Cargo.toml, build.sh, and Dockerfile.release live.
release_app_root() {
  release_app_require "$1"
  case "$1" in sift) printf 'apps/sift\n' ;; *) printf 'apps/%s\n' "$1" ;; esac
}
# Directory of the app's verify-release-candidate.sh and verify-release-artifacts.sh.
release_app_scripts_dir() {
  release_app_require "$1"
  case "$1" in tape) printf 'apps/%s/scripts\n' "$1" ;; *) printf 'scripts/release\n' ;; esac
}
# True for the apps whose verifiers take --app (the shared twins here).
release_app_uses_shared_scripts() {
  release_app_require "$1"
  [[ "$1" != tape ]]
}
release_app_targets() {
  release_app_require "$1"
  case "$1" in
    tape|sift)
      printf '%s\n' aarch64-apple-darwin x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-unknown-linux-musl aarch64-unknown-linux-musl ;;
    keep|relay|defer)
      printf '%s\n' x86_64-unknown-linux-musl aarch64-unknown-linux-musl ;;
  esac
}
# Which harness produces the GKE receipt (build-release steps 5 and 6).
release_app_gke_backend() {
  release_app_require "$1"
  case "$1" in tape|sift) printf 'gcp\n' ;; keep|relay|defer) printf 'gke-acceptance\n' ;; esac
}
release_app_receipt_name() {
  release_app_require "$1"
  printf '%s-gke-receipt.json\n' "$1"
}
release_app_receipt_schema() {
  release_app_require "$1"
  printf '%s.gke-release-receipt/v1\n' "$1"
}
# Functional fields every receipt must carry as "passed" (shared-script apps only).
release_app_functional_fields() {
  release_app_require "$1"
  case "$1" in
    sift) printf '%s\n' operator_reconcile_1x1 standard_gke_cri_collector lumen_structured_stdout_materialized scheduled_backup gcs_backup ;;
    keep|relay|defer) printf '%s\n' readyz round_trip durability ;;
    *) release_refuse "$1 keeps its own receipt contract under apps/$1/scripts" ;;
  esac
}
# Promotion workflow input names.
release_app_promotion_input_prefix() {
  release_app_require "$1"
  printf 'gke_receipt\n'
}
release_app_promotion_takes_attempt() {
  release_app_require "$1"
  true
}
release_app_manifest_schema() {
  release_app_require "$1"
  printf 'cclab.%s.candidate-manifest.v1\n' "$1"
}
# Candidate job ids bound by the final manifest's `jobs` map.
release_app_candidate_jobs() {
  release_app_require "$1"
  case "$1" in
    tape) printf '%s\n' identity build tape-release-gates ghcr-image-and-attest manifest verify-candidate verify-libraries kind-amd64 kind-arm64 result ;;
    *) printf '%s\n' identity build "$1-release-gates" ghcr-image-and-attest manifest verify-candidate result ;;
  esac
}
# Display names of every job in one successful candidate attempt (shared-script apps only).
release_app_candidate_job_names() {
  release_app_require "$1"
  release_app_uses_shared_scripts "$1" || release_refuse "$1 keeps its own job inventory under apps/$1/scripts"
  local target
  printf '%s\n' "candidate identity" "verify exact $1 release gates" "build candidate image and attest" "bind candidate inputs" "verify exact candidate gates" "final candidate receipt"
  while IFS= read -r target; do printf 'build (%s)\n' "$target"; done < <(release_app_targets "$1")
}
