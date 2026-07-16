#!/bin/bash
# Run (or update) snapshot tests in an ubuntu-latest-like container, so Linux
# baselines can be generated locally instead of downloaded from CI artifacts.
#
#   ./scripts/linux-snapshots.sh              # run the full suite on Linux
#   ./scripts/linux-snapshots.sh --update     # regenerate the Linux system-fonts baseline
#
# The container matches the GitHub runner where it matters for rendering:
# Ubuntu 24.04 with the same font packages from the same apt archive. The
# platform-agnostic baselines are shared with macOS, so --update only touches
# the per-OS system-fonts test.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
IMAGE=dynimg-linux-snapshots

docker build -q -t "$IMAGE" -f "$SCRIPT_DIR/linux-snapshots.Dockerfile" "$SCRIPT_DIR" >/dev/null

RUN_ARGS=()
if [ "$1" = "--update" ] || [ "$1" = "-u" ]; then
    # Only the per-OS test: shared baselines are platform-agnostic and are
    # regenerated on macOS with ./scripts/snapshot-tests.sh --update
    RUN_ARGS=(-e ONLY=system-fonts)
    set -- --update
fi

exec docker run --rm \
    -v "$PROJECT_ROOT":/work \
    -v "$HOME/.cargo/registry":/root/.cargo/registry \
    -e CARGO_TARGET_DIR=/work/scratch/target-linux-snapshots \
    "${RUN_ARGS[@]}" \
    "$IMAGE" ./scripts/snapshot-tests.sh "$@"
