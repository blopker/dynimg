#!/bin/bash
# Test a wheel from CI artifacts
#
# Usage: ./test_from_ci.sh [RUN_ID] [ARTIFACT_NAME]
#
# Examples:
#   ./test_from_ci.sh                           # Latest run, auto-detect platform
#   ./test_from_ci.sh 12345678                  # Specific run, auto-detect platform
#   ./test_from_ci.sh 12345678 wheel-linux-x86_64  # Specific run and artifact

set -e

cd "$(dirname "$0")"

# Auto-detect platform artifact name
detect_artifact() {
    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64)  echo "wheel-linux-x86_64" ;;
        Linux-aarch64) echo "wheel-linux-aarch64" ;;
        Darwin-x86_64) echo "wheel-macos-x86_64" ;;
        Darwin-arm64)  echo "wheel-macos-aarch64" ;;
        *) echo "Unknown platform: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
    esac
}

RUN_ID="${1:-}"
ARTIFACT="${2:-$(detect_artifact)}"

echo "=== Downloading wheel from CI ==="
echo "Artifact: $ARTIFACT"

# Clean up old wheels
rm -f *.whl

if [ -n "$RUN_ID" ]; then
    echo "Run ID: $RUN_ID"
    gh run download "$RUN_ID" -n "$ARTIFACT"
else
    echo "Run ID: latest"
    gh run download -n "$ARTIFACT"
fi

# Find the downloaded wheel
WHEEL=$(ls *.whl 2>/dev/null | head -1)
if [ -z "$WHEEL" ]; then
    echo "No wheel found!" >&2
    exit 1
fi
echo "Downloaded: $WHEEL"

echo ""
echo "=== Setting up venv ==="
rm -rf .venv
python3 -m venv .venv
source .venv/bin/activate

echo ""
echo "=== Installing wheel ==="
pip install --quiet "$WHEEL"

echo ""
echo "=== Running tests ==="
python test_dynimg.py

echo ""
echo "Done!"
