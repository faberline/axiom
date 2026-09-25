#!/usr/bin/env bash
# Slow migration gate: compiles every registered RaftStateMachine implementor
# in this repository. Each application uses its bounded package and feature
# gate. lumen's implementors build in faberline/lumen, and raft-runtime's
# implementor_build_coverage inventory lives with raft-runtime in faberline/core.
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "cargo build -p defer"
cargo build -p defer

echo "cargo build -p keep --features raft"
cargo build -p keep --features raft

echo "cargo build -p loom"
cargo build -p loom

echo "cargo build -p relay"
cargo build -p relay

echo "cargo build -p tape"
cargo build -p tape

echo "cargo build -p sift"
cargo build -p sift
