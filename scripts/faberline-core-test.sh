#!/usr/bin/env bash
# Test faberline/core packages at the exact commit this checkout's Cargo.lock
# pins. Core crates are git dependencies here, and cargo refuses to test a
# package outside the workspace that has dev-dependencies, so each package is
# tested inside core's own workspace (against core's own Cargo.lock) from the
# checkout cargo already made of that commit.
#
# usage: scripts/faberline-core-test.sh <package>... [<cargo test args>...]
#   The leading non-option arguments are packages; everything from the first
#   argument that starts with `-` is passed to each `cargo test` verbatim, e.g.
#   scripts/faberline-core-test.sh service-auth --lib reload::tests -- --list
# usage: scripts/faberline-core-test.sh --root
#   prints the faberline/core checkout, for a runner that wraps cargo itself.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
usage="usage: scripts/faberline-core-test.sh <package>... [<cargo test args>...]"
root_only=false
[[ "${1:-}" == --root && $# -eq 1 ]] && root_only=true && shift
packages=()
while (($#)) && [[ "$1" != -* ]]; do
  packages+=("$1")
  shift
done
$root_only || ((${#packages[@]})) || { echo "$usage" >&2; exit 2; }

core="$(
  cargo metadata --locked --format-version 1 --manifest-path "$ROOT_DIR/Cargo.toml" |
    python3 -c '
import json, sys
from pathlib import Path
roots = {
    Path(p["manifest_path"]).parents[2]
    for p in json.load(sys.stdin)["packages"]
    if (p.get("source") or "").startswith("git+https://github.com/faberline/core?")
}
if len(roots) != 1:
    sys.exit(f"Cargo.lock must resolve exactly one faberline/core checkout, found {len(roots)}")
print(roots.pop())
'
)"
[[ -f "$core/Cargo.toml" && -f "$core/Cargo.lock" ]] || {
  echo "faberline/core checkout is incomplete: $core" >&2
  exit 1
}
$root_only && { printf '%s\n' "$core"; exit 0; }

for pkg in "${packages[@]}"; do
  echo "faberline/core: cargo test -p $pkg $*"
  cargo test --locked --manifest-path "$core/Cargo.toml" \
    --target-dir "${CARGO_TARGET_DIR:-$ROOT_DIR/target}" -p "$pkg" "$@"
done
