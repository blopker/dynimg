#!/bin/bash
# Snapshot tests: compare rendered output against known good images
# Run from project root: ./scripts/snapshot-tests.sh
# Update snapshots:      ./scripts/snapshot-tests.sh --update

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Most baselines live in tests/snapshots/ and are platform-agnostic: their
# tests pin every font via $PIN below. Comparison is perceptual (zensim via
# snapcmp) rather than byte-exact because SIMD rounding differs across CPU
# architectures — JPEG encoding (scores ~96 vs 100 across arch) and, on
# gradient/mask-heavy pages, vello_cpu rasterization itself (~98-99.5).
# Tests that deliberately exercise system fonts use snapshot_test_os and
# per-platform baselines in tests/snapshots/{macos,linux}.
case "$(uname -s)" in
    Darwin) PLATFORM="macos" ;;
    Linux)  PLATFORM="linux" ;;
    *)      echo "Unsupported platform for snapshots: $(uname -s)"; exit 1 ;;
esac
SNAPSHOTS_DIR="$PROJECT_ROOT/tests/snapshots"
OS_SNAPSHOTS_DIR="$PROJECT_ROOT/tests/snapshots/$PLATFORM"
EXAMPLES_DIR="$PROJECT_ROOT/examples"
FONTS_DIR="$EXAMPLES_DIR/assets/fonts"
OUTPUT_DIR="$PROJECT_ROOT/tests/output"
mkdir -p "$OUTPUT_DIR" "$OS_SNAPSHOTS_DIR"

# Minimum zensim score (100 = identical). Cross-arch SIMD noise bottoms out
# around ~96 (measured: x86_64 renders vs arm64 baselines, worst case 95.87);
# real rendering regressions score far below zero (a subtle font swap: -222).
MIN_SCORE=90

# Pins every generic family and emoji to bundled fonts so rendering doesn't
# depend on host fonts.
PIN=(
    --font "sans-serif=$FONTS_DIR/Inter-Variable.ttf"
    --font "system-ui=$FONTS_DIR/Inter-Variable.ttf"
    --font "serif=$FONTS_DIR/Gelasio-Variable.ttf"
    --font "monospace=$FONTS_DIR/RobotoMono-Bold.ttf"
    --font "emoji=$FONTS_DIR/TwemojiCOLRv0.ttf"
)

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

# Build release version + the perceptual comparison tool.
# Honors CARGO_TARGET_DIR so containers/CI can build to a separate directory.
TARGET_DIR="${CARGO_TARGET_DIR:-$PROJECT_ROOT/target}"
echo "Building dynimg..."
cargo build --release --manifest-path "$PROJECT_ROOT/Cargo.toml" --quiet || exit 1
cargo build --release --example snapcmp --manifest-path "$PROJECT_ROOT/Cargo.toml" --quiet || exit 1
DYNIMG="$TARGET_DIR/release/dynimg"
SNAPCMP="$TARGET_DIR/release/examples/snapcmp"

echo ""
echo "=== Snapshot Tests ==="

if [ "$UPDATE_MODE" = true ]; then
    echo -e "${YELLOW}Running in UPDATE mode - snapshots will be regenerated${NC}"
fi
echo ""

# Track results
PASSED=0
FAILED=0
RAN=0

# Run a snapshot test against a platform-agnostic baseline. Every test in
# this mode is automatically font-pinned (see $PIN) so its baseline renders
# identically on all platforms.
# Args: name, extension, input_html, [extra_args...]
snapshot_test() {
    snapshot_test_in_dir "$SNAPSHOTS_DIR" "$@" "${PIN[@]}"
}

# Run a snapshot test against a per-platform baseline (for tests that
# deliberately use system fonts).
snapshot_test_os() {
    snapshot_test_in_dir "$OS_SNAPSHOTS_DIR" "$@"
}

snapshot_test_in_dir() {
    local dir="$1"
    local name="$2"
    local ext="$3"
    local input="$4"
    shift 4
    local extra_args=("$@")

    # ONLY=<name> runs a single test (e.g. regenerating one baseline)
    if [ -n "$ONLY" ] && [ "$name" != "$ONLY" ]; then
        return
    fi
    : $((RAN++))

    local output_file="$OUTPUT_DIR/${name}.${ext}"
    local snapshot_file="$dir/${name}.${ext}"

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

    # Compare perceptually (see MIN_SCORE above)
    local score
    if score=$("$SNAPCMP" "$snapshot_file" "$output_file" --min-score "$MIN_SCORE"); then
        printf "${GREEN}OK${NC} %6s        (%5.2fs)\n" "$score" "$elapsed"
        : $((PASSED++))
    else
        printf "${RED}MISMATCH${NC} %6s  (%5.2fs)\n" "$score" "$elapsed"
        echo "         Output:   $output_file"
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
echo "--- Custom Fonts ---"
snapshot_test "custom-font" png "$EXAMPLES_DIR/custom-font.html" -w 500 -H 220 --font "$FONTS_DIR/Silkscreen-Regular.ttf"
snapshot_test "fonts-showcase" png "$EXAMPLES_DIR/fonts-showcase.html" -w 600 \
    --font "$FONTS_DIR/PlaywriteINGuides-Regular.ttf" \
    --font "$FONTS_DIR/RobotoMono-Bold.ttf" \
    --font "$FONTS_DIR/Silkscreen-Regular.ttf" \
    --font "emoji=$FONTS_DIR/TwemojiCOLRv0.ttf" \
    --font "cursive=$FONTS_DIR/Silkscreen-Regular.ttf" \
    --font "brand=$FONTS_DIR/PlaywriteINGuides-Regular.ttf" \
    --assets "$EXAMPLES_DIR/assets"

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
echo "--- System Fonts (per-OS baseline: no font pinning, host fonts differ) ---"
snapshot_test_os "system-fonts" png "$EXAMPLES_DIR/system-fonts.html" -w 500 -H 260

echo ""
echo "=== Results ==="
echo "  Passed: $PASSED"
echo "  Failed: $FAILED"

if [ "$UPDATE_MODE" = true ]; then
    echo ""
    echo "Snapshots saved to: $SNAPSHOTS_DIR"
fi

if [ -n "$ONLY" ] && [ "$RAN" -eq 0 ]; then
    echo ""
    echo "ERROR: ONLY=$ONLY matched no tests"
    exit 1
fi

exit $FAILED
