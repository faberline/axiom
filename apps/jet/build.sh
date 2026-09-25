#!/usr/bin/env bash
# <HANDWRITE gap="standardize:claim-code" tracker="projects-jet-build-sh" reason="Existing code claimed during Score standardization until deterministic generator coverage lands.">
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: apps/jet/build.sh <debug|release>

debug    Build jet and install target/debug/jet to ~/.cargo/bin/jet.
release  Build/install jet, create a release commit, and print the tag to push after git:land.

Note: this is the local host install. Cross-platform release binaries
(macOS arm64 + Linux x64/arm64) are built by .github/workflows/jet-release.yml
when the jet@<version> tag is pushed.
EOF
}

fail_hint() {
  local mode="$1"
  echo ""
  echo "Build failed."
  echo "Retry with: apps/jet/build.sh ${mode}"
  echo "Verify with: ~/.cargo/bin/jet --version"
}

MODE="${1:-}"
if [[ "${2:-}" == "-h" || "${2:-}" == "--help" || "${2:-}" == "help" ]]; then
  usage
  exit 0
fi
if [[ $# -gt 1 ]]; then
  usage >&2
  exit 2
fi
case "$MODE" in
  -h|--help|help|"")
    usage
    exit 0
    ;;
  debug|release)
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"
. scripts/project-build-lib.sh

RUSTUP_STABLE="$HOME/.rustup/toolchains/stable-aarch64-apple-darwin/bin"
if [[ -x "$RUSTUP_STABLE/cargo" ]]; then
  export PATH="$RUSTUP_STABLE:$PATH"
fi

trap 'fail_hint "$MODE"' ERR

install_jet() {
  local profile="$1"
  install -m 755 "target/${profile}/jet" "$HOME/.cargo/bin/jet"
  codesign -s - -f "$HOME/.cargo/bin/jet" 2>/dev/null || true
  echo "Installed: $("$HOME/.cargo/bin/jet" --version 2>/dev/null || echo 'jet')"
  echo "Verify with: ~/.cargo/bin/jet --version"
}

if [[ "$MODE" == "debug" ]]; then
  VERSION_FILES=(apps/jet/Cargo.toml)
  CURRENT_VERSION="$(project_build_read_version apps/jet/Cargo.toml)"
  project_build_prepare_debug_version jet "$CURRENT_VERSION" "${VERSION_FILES[@]}"
  cargo build -p jet --bin jet
  install_jet debug
  project_build_restore_manifests
  echo ""
  echo "Build complete (debug ${PROJECT_BUILD_DEBUG_VERSION})."
  exit 0
fi

VERSION_FILES=(apps/jet/Cargo.toml)
CURRENT_VERSION="$(project_build_read_version apps/jet/Cargo.toml)"
export PROJECT_BUILD_REQUIRE_REMOTE_TAG_CHECK=1
project_build_prepare_release_version jet "$CURRENT_VERSION" "${VERSION_FILES[@]}"

cargo update -w 2>/dev/null || cargo generate-lockfile
cargo build --release -p jet --bin jet
install_jet release

TAG="${PROJECT_BUILD_RELEASE_TAG}"
git add Cargo.lock README.md apps/jet
git commit --allow-empty -m "release(jet): ${TAG}"

project_build_print_release_next_steps jet "$TAG"

# </HANDWRITE>
