#!/bin/bash
# Render all example HTML files to test various features
# Run from project root: ./scripts/render-examples.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUTPUT_DIR="$PROJECT_ROOT/output"
EXAMPLES_DIR="$PROJECT_ROOT/examples"
ASSETS_DIR="$EXAMPLES_DIR/assets"

# Build release version
echo "Building dynimg..."
cargo build --release --manifest-path "$PROJECT_ROOT/Cargo.toml" || exit 1
DYNIMG="$PROJECT_ROOT/target/release/dynimg"

# Clean output directory
rm -rf "$OUTPUT_DIR"/*
mkdir -p "$OUTPUT_DIR"

echo ""
echo "=== Running Tests ==="

# Track results
PASSED=0
FAILED=0

run_test() {
    local name="$1"
    local expect_fail="$2"
    shift 2

    printf "  %-50s " "$name"

    local start_time=$(perl -MTime::HiRes=time -e 'printf "%.3f", time')
    if "$@" > /dev/null 2>&1; then
        local end_time=$(perl -MTime::HiRes=time -e 'printf "%.3f", time')
        local elapsed=$(echo "$end_time - $start_time" | bc)
        if [ "$expect_fail" = "true" ]; then
            printf "UNEXPECTED PASS  (%5.2fs)\n" "$elapsed"
            ((FAILED++))
        else
            printf "OK               (%5.2fs)\n" "$elapsed"
            ((PASSED++))
        fi
    else
        local end_time=$(perl -MTime::HiRes=time -e 'printf "%.3f", time')
        local elapsed=$(echo "$end_time - $start_time" | bc)
        if [ "$expect_fail" = "true" ]; then
            printf "EXPECTED FAIL    (%5.2fs)\n" "$elapsed"
            ((PASSED++))
        else
            printf "FAIL             (%5.2fs)\n" "$elapsed"
            ((FAILED++))
        fi
    fi
}

echo ""
echo "--- Inline Only (no external deps) ---"
run_test "PNG output" false "$DYNIMG" "$EXAMPLES_DIR/inline-only.html" -o "$OUTPUT_DIR/inline-only.png"
run_test "JPEG output (quality 85)" false "$DYNIMG" "$EXAMPLES_DIR/inline-only.html" -o "$OUTPUT_DIR/inline-only.jpg" --quality 85
run_test "WebP output" false "$DYNIMG" "$EXAMPLES_DIR/inline-only.html" -o "$OUTPUT_DIR/inline-only.webp"
run_test "Custom dimensions" false "$DYNIMG" "$EXAMPLES_DIR/inline-only.html" -o "$OUTPUT_DIR/inline-custom.png" -w 400 -H 300

echo ""
echo "--- Remote Image (requires --allow-net) ---"
run_test "Without --allow-net" false "$DYNIMG" "$EXAMPLES_DIR/remote-image.html" -o "$OUTPUT_DIR/remote-no-net.png"
run_test "With --allow-net" false "$DYNIMG" "$EXAMPLES_DIR/remote-image.html" -o "$OUTPUT_DIR/remote-with-net.png" --allow-net

echo ""
echo "--- Local Assets (requires --assets) ---"
run_test "Without --assets" false "$DYNIMG" "$EXAMPLES_DIR/local-assets.html" -o "$OUTPUT_DIR/local-no-assets.png"
run_test "With --assets" false "$DYNIMG" "$EXAMPLES_DIR/local-assets.html" -o "$OUTPUT_DIR/local-with-assets.png" --assets "$ASSETS_DIR"

echo ""
echo "--- Mixed Assets (requires both flags) ---"
run_test "Without flags" false "$DYNIMG" "$EXAMPLES_DIR/mixed-assets.html" -o "$OUTPUT_DIR/mixed-no-flags.png"
run_test "With --allow-net only" false "$DYNIMG" "$EXAMPLES_DIR/mixed-assets.html" -o "$OUTPUT_DIR/mixed-net-only.png" --allow-net
run_test "With --allow-net only" false "$DYNIMG" "$EXAMPLES_DIR/mixed-assets.html" -o "$OUTPUT_DIR/mixed-net-only.webp" --allow-net
run_test "With --assets only" false "$DYNIMG" "$EXAMPLES_DIR/mixed-assets.html" -o "$OUTPUT_DIR/mixed-assets-only.png" --assets "$ASSETS_DIR"
run_test "With both flags" false "$DYNIMG" "$EXAMPLES_DIR/mixed-assets.html" -o "$OUTPUT_DIR/mixed-both-flags.png" --allow-net --assets "$ASSETS_DIR"
run_test "With both flags" false "$DYNIMG" "$EXAMPLES_DIR/google-fonts.html" -o "$OUTPUT_DIR/google-fonts.png" --allow-net --assets "$ASSETS_DIR"
run_test "With both flags" false "$DYNIMG" "$EXAMPLES_DIR/google-fonts.html" -o "$OUTPUT_DIR/google-fonts.webp" --allow-net --assets "$ASSETS_DIR"

echo ""
echo "--- OG Image Templates ---"
run_test "og-image.html" false "$DYNIMG" "$EXAMPLES_DIR/og-image.html" -o "$OUTPUT_DIR/og-image.png"
run_test "og-image.html" false "$DYNIMG" "$EXAMPLES_DIR/og-image.html" -o "$OUTPUT_DIR/og-image.webp"
run_test "social-card.html" false "$DYNIMG" "$EXAMPLES_DIR/social-card.html" -o "$OUTPUT_DIR/social-card.png"
run_test "quote.html" false "$DYNIMG" "$EXAMPLES_DIR/quote.html" -o "$OUTPUT_DIR/quote.png"

echo ""
echo "--- Stdin Input ---"
printf "  %-50s " "From stdin"
start_time=$(perl -MTime::HiRes=time -e 'printf "%.3f", time')
echo '<html><body style="background:#10b981;padding:50px;"><h1 style="color:white;font-size:48px;">Stdin Test</h1></body></html>' | "$DYNIMG" - -o "$OUTPUT_DIR/stdin-test.png" > /dev/null 2>&1
result=$?
end_time=$(perl -MTime::HiRes=time -e 'printf "%.3f", time')
elapsed=$(echo "$end_time - $start_time" | bc)
if [ $result -eq 0 ]; then
    printf "OK               (%5.2fs)\n" "$elapsed"
    ((PASSED++))
else
    printf "FAIL             (%5.2fs)\n" "$elapsed"
    ((FAILED++))
fi

echo ""
echo "--- Expected Failures ---"
run_test "Invalid output format (.txt)" true "$DYNIMG" "$EXAMPLES_DIR/inline-only.html" -o "$OUTPUT_DIR/invalid.txt"
run_test "Non-existent input file" true "$DYNIMG" "non-existent.html" -o "$OUTPUT_DIR/missing.png"

echo ""
echo "=== Results ==="
echo "  Passed: $PASSED"
echo "  Failed: $FAILED"
echo ""

# List output files
echo "=== Generated Files ==="
ls -lh "$OUTPUT_DIR"

exit $FAILED
