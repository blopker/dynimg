#!/bin/bash
# Snapshot tests: compare rendered output against known good images
# Run from project root: ./scripts/snapshot-tests.sh
# Update snapshots:      ./scripts/snapshot-tests.sh --update

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SNAPSHOTS_DIR="$PROJECT_ROOT/tests/snapshots"
EXAMPLES_DIR="$PROJECT_ROOT/examples"
OUTPUT_DIR="$PROJECT_ROOT/tests/output"
mkdir -p "$OUTPUT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Parse args
UPDATE_MODE=false
if [ "$1" = "--update" ] || [ "$1" = "-u" ]; then
    UPDATE_MODE=true
fi

# Build release version
echo "Building dynimg..."
cargo build --release --manifest-path "$PROJECT_ROOT/Cargo.toml" --quiet || exit 1
DYNIMG="$PROJECT_ROOT/target/release/dynimg"

echo ""
echo "=== Snapshot Tests ==="

if [ "$UPDATE_MODE" = true ]; then
    echo -e "${YELLOW}Running in UPDATE mode - snapshots will be regenerated${NC}"
fi
echo ""

# Track results
PASSED=0
FAILED=0

# Run a snapshot test
# Args: name, extension, input_html, [extra_args...]
snapshot_test() {
    local name="$1"
    local ext="$2"
    local input="$3"
    shift 3
    local extra_args=("$@")

    local output_file="$OUTPUT_DIR/${name}.${ext}"
    local snapshot_file="$SNAPSHOTS_DIR/${name}.${ext}"

    printf "  %-40s " "$name"

    # Capture timing
    local start_time=$(perl -MTime::HiRes=time -e 'printf "%.3f", time')

    # Render the image
    if ! "$DYNIMG" "$input" -o "$output_file" "${extra_args[@]}" > /dev/null 2>&1; then
        local end_time=$(perl -MTime::HiRes=time -e 'printf "%.3f", time')
        local elapsed=$(echo "$end_time - $start_time" | bc)
        printf "${RED}RENDER FAILED${NC}    (%5.2fs)\n" "$elapsed"
        : $((FAILED++))
        return
    fi

    local end_time=$(perl -MTime::HiRes=time -e 'printf "%.3f", time')
    local elapsed=$(echo "$end_time - $start_time" | bc)

    if [ "$UPDATE_MODE" = true ]; then
        cp "$output_file" "$snapshot_file"
        printf "${YELLOW}UPDATED${NC}          (%5.2fs)\n" "$elapsed"
        : $((PASSED++))
        return
    fi

    # Check if snapshot exists
    if [ ! -f "$snapshot_file" ]; then
        printf "${RED}NO SNAPSHOT${NC}      (%5.2fs)\n" "$elapsed"
        echo "         Run with --update to create snapshot"
        : $((FAILED++))
        return
    fi

    # Compare using sha256
    local output_hash=$(shasum -a 256 "$output_file" | cut -d' ' -f1)
    local snapshot_hash=$(shasum -a 256 "$snapshot_file" | cut -d' ' -f1)

    if [ "$output_hash" = "$snapshot_hash" ]; then
        printf "${GREEN}OK${NC}               (%5.2fs)\n" "$elapsed"
        : $((PASSED++))
    else
        printf "${RED}MISMATCH${NC}         (%5.2fs)\n" "$elapsed"
        echo "         Output:   $OUTPUT_DIR/${name}.png"
        echo "         Expected: $snapshot_file"
        : $((FAILED++))
    fi
}

echo "--- Inline HTML (no external deps) ---"
snapshot_test "inline-only" png "$EXAMPLES_DIR/inline-only.html"
snapshot_test "inline-only-jpg" jpg "$EXAMPLES_DIR/inline-only.html" --quality 90
snapshot_test "inline-only-webp" webp "$EXAMPLES_DIR/inline-only.html"
snapshot_test "inline-custom-size" png "$EXAMPLES_DIR/inline-only.html" -w 400 -H 300

echo ""
echo "--- Transparent Background ---"
snapshot_test "transparent" png "$EXAMPLES_DIR/transparent.html"
snapshot_test "transparent-webp" webp "$EXAMPLES_DIR/transparent.html"
snapshot_test "transparent-jpg" jpg "$EXAMPLES_DIR/transparent.html" --quality 90

echo ""
echo "--- Emoji ---"
snapshot_test "emoji" png "$EXAMPLES_DIR/emoji.html"

echo ""
echo "--- Data URIs (no flags needed) ---"
snapshot_test "data-uri" png "$EXAMPLES_DIR/data-uri.html" -w 400 -H 350

echo ""
echo "--- Kitchen Sink (floats, border-style, mask, font-variant, inline bg, background-size, data URIs) ---"
snapshot_test "kitchen-sink" png "$EXAMPLES_DIR/kitchen-sink.html" -w 500

echo ""
echo "--- Mixed Assets ---"
snapshot_test "mixed-no-flags" png "$EXAMPLES_DIR/mixed-assets.html"
snapshot_test "mixed-assets-only" png "$EXAMPLES_DIR/mixed-assets.html" --assets "$EXAMPLES_DIR/assets"
snapshot_test "mixed-net-only" png "$EXAMPLES_DIR/mixed-assets.html" --allow-net
snapshot_test "mixed-both-flags" png "$EXAMPLES_DIR/mixed-assets.html" --allow-net --assets "$EXAMPLES_DIR/assets"

echo ""
echo "--- OG Image Templates ---"
snapshot_test "og-image" png "$EXAMPLES_DIR/og-image.html"
snapshot_test "social-card" png "$EXAMPLES_DIR/social-card.html"
snapshot_test "quote" png "$EXAMPLES_DIR/quote.html"

echo ""
echo "=== Results ==="
echo "  Passed: $PASSED"
echo "  Failed: $FAILED"

if [ "$UPDATE_MODE" = true ]; then
    echo ""
    echo "Snapshots saved to: $SNAPSHOTS_DIR"
fi

exit $FAILED
