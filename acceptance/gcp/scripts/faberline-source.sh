#!/usr/bin/env bash
# Check out the faberline/<repo> commit this checkout's Cargo.lock pins, so a
# binary built from it is the one the workspace resolves. lumen and rig live in
# their own repositories; cargo cannot pass features to a package outside the
# workspace, so their binaries are built in their own workspaces instead.
#
# usage: faberline-source.sh <lumen|rig> <dest>   (prints the pinned commit)
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
repo="${1:-}"
dest="${2:-}"
case "$repo" in
  lumen|rig) ;;
  *) echo "usage: faberline-source.sh <lumen|rig> <dest>" >&2; exit 2 ;;
esac
[[ -n "$dest" ]] || { echo "usage: faberline-source.sh <lumen|rig> <dest>" >&2; exit 2; }

rev="$(
  sed -n "s|^source = \"git+https://github.com/faberline/$repo?rev=[0-9a-f]*#\([0-9a-f]\{40\}\)\"\$|\1|p" \
    "$REPO_ROOT/Cargo.lock" | sort -u
)"
[[ "$rev" =~ ^[0-9a-f]{40}$ ]] || {
  echo "Cargo.lock must pin exactly one faberline/$repo commit" >&2
  exit 1
}

if [[ "$(git -C "$dest" rev-parse HEAD 2>/dev/null || true)" != "$rev" ]]; then
  mkdir -p "$dest"
  git init -q "$dest"
  git -C "$dest" fetch -q --depth 1 "https://github.com/faberline/$repo" "$rev"
  git -C "$dest" checkout -q --detach FETCH_HEAD
fi
printf '%s\n' "$rev"
