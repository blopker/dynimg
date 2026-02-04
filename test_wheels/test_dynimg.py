#!/usr/bin/env python3
"""Test script for dynimg wheels."""

import sys
import time
from contextlib import contextmanager
from pathlib import Path

OUTPUT_DIR = Path(__file__).parent / "output"


class RunTimer:
    def __init__(self):
        self.start = time.perf_counter()

    @contextmanager
    def measure(self, name):
        t0 = time.perf_counter()
        yield
        t1 = time.perf_counter()
        print(f"[{t1 - self.start:.3f}s] {name} took {(t1 - t0) * 1000:.2f}ms")


def test_import():
    """Test basic import."""
    import dynimg

    print(f"dynimg version: {dynimg.__version__}")
    print(f"Module: {dynimg}")
    return True


def test_render_basic():
    """Test basic rendering."""
    import dynimg

    timer = RunTimer()
    html = '<html><body style="background:blue;"><h1>Test</h1></body></html>'

    with timer.measure("Render"):
        img = dynimg.render(
            html, dynimg.RenderOptions(width=100, height=100, scale=1.0)
        )

    assert img.width == 100, f"Expected width 100, got {img.width}"
    assert img.height == 100, f"Expected height 100, got {img.height}"

    with timer.measure("Save PNG"):
        img.save_png(str(OUTPUT_DIR / "test_basic.png"))

    with timer.measure("Save JPEG"):
        img.save_jpeg(str(OUTPUT_DIR / "test_basic.jpg"))

    with timer.measure("Save WebP"):
        img.save_webp(str(OUTPUT_DIR / "test_basic.webp"))

    print(f"Basic render: {img.width}x{img.height}")
    return True


def test_render_gradient():
    """Test gradient rendering."""
    import dynimg

    timer = RunTimer()
    html = """
    <html>
    <body style="background: linear-gradient(135deg, #667eea, #764ba2);
                 display: flex; justify-content: center; align-items: center;
                 height: 630px; margin: 0;">
        <h1 style="color: white; font-family: system-ui; font-size: 64px;">
            Hello World
        </h1>
    </body>
    </html>
    """
    options = dynimg.RenderOptions(width=1200, height=630, scale=2.0)

    with timer.measure("Render"):
        img = dynimg.render(html, options)

    assert img.width == 2400, f"Expected width 2400, got {img.width}"
    assert img.height == 1260, f"Expected height 1260, got {img.height}"
    with timer.measure("Save WebP"):
        img.save_webp(str(OUTPUT_DIR / "test_gradient.webp"))

    with timer.measure("Save PNG"):
        img.save_png(str(OUTPUT_DIR / "test_gradient.png"))

    with timer.measure("Save JPEG"):
        img.save_jpeg(str(OUTPUT_DIR / "test_gradient.jpg"))

    print(f"Gradient render: {img.width}x{img.height}")
    return True


def test_save_formats():
    """Test saving to different formats."""
    import dynimg

    timer = RunTimer()
    html = '<html><body style="background:red; width:50px; height:50px;"></body></html>'
    with timer.measure("Render"):
        img = dynimg.render(html, dynimg.RenderOptions(width=50, height=50, scale=1.0))

    # Test PNG
    png_path = OUTPUT_DIR / "test_output.png"
    with timer.measure("Save PNG"):
        img.save_png(str(png_path))
    assert png_path.exists(), "PNG file not created"
    png_size = png_path.stat().st_size
    print(f"PNG saved: {png_size} bytes")

    # Test WebP
    webp_path = OUTPUT_DIR / "test_output.webp"
    with timer.measure("Save WebP"):
        img.save_webp(str(webp_path))
    assert webp_path.exists(), "WebP file not created"
    webp_size = webp_path.stat().st_size
    print(f"WebP saved: {webp_size} bytes")

    # Test JPEG
    jpeg_path = OUTPUT_DIR / "test_output.jpg"
    with timer.measure("Save JPEG"):
        img.save_jpeg(str(jpeg_path), quality=90)
    assert jpeg_path.exists(), "JPEG file not created"
    jpeg_size = jpeg_path.stat().st_size
    print(f"JPEG saved: {jpeg_size} bytes")

    return True


def test_to_bytes():
    """Test encoding to bytes."""
    import dynimg

    timer = RunTimer()
    html = (
        '<html><body style="background:green; width:50px; height:50px;"></body></html>'
    )

    with timer.measure("Render"):
        img = dynimg.render(html, dynimg.RenderOptions(width=50, height=50, scale=1.0))

    png_bytes = img.to_png()
    assert len(png_bytes) > 0, "PNG bytes empty"
    assert png_bytes[:8] == b"\x89PNG\r\n\x1a\n", "Invalid PNG header"
    print(f"PNG bytes: {len(png_bytes)} bytes")

    webp_bytes = img.to_webp()
    assert len(webp_bytes) > 0, "WebP bytes empty"
    assert webp_bytes[:4] == b"RIFF", "Invalid WebP header"
    print(f"WebP bytes: {len(webp_bytes)} bytes")

    jpeg_bytes = img.to_jpeg(quality=90)
    assert len(jpeg_bytes) > 0, "JPEG bytes empty"
    assert jpeg_bytes[:2] == b"\xff\xd8", "Invalid JPEG header"
    print(f"JPEG bytes: {len(jpeg_bytes)} bytes")

    return True


def test_render_to_file():
    """Test render_to_file convenience function."""
    import dynimg

    timer = RunTimer()
    html = (
        '<html><body style="background:yellow; width:50px; height:50px;"></body></html>'
    )

    png_path = OUTPUT_DIR / "test_direct.png"
    with timer.measure("render_to_file PNG"):
        dynimg.render_to_file(html, str(png_path))
    assert png_path.exists(), "Direct PNG not created"
    print(f"render_to_file PNG: {png_path.stat().st_size} bytes")
    png_path.unlink()

    webp_path = OUTPUT_DIR / "test_direct.webp"
    with timer.measure("render_to_file WebP"):
        dynimg.render_to_file(
            html,
            str(webp_path),
            options=dynimg.RenderOptions(width=100, height=100, scale=1.0),
            quality=85,
        )
    assert webp_path.exists(), "Direct WebP not created"
    print(f"render_to_file WebP: {webp_path.stat().st_size} bytes")
    webp_path.unlink()

    return True


def main():
    """Run all tests."""
    OUTPUT_DIR.mkdir(exist_ok=True)

    tests = [
        ("Import", test_import),
        ("Basic Render", test_render_basic),
        ("Gradient Render", test_render_gradient),
        ("Save Formats", test_save_formats),
        ("To Bytes", test_to_bytes),
        ("Render to File", test_render_to_file),
    ]

    print("=" * 50)
    print("dynimg Wheel Test Suite")
    print("=" * 50)

    passed = 0
    failed = 0

    for name, test_func in tests:
        print(f"\n[TEST] {name}")
        try:
            if test_func():
                print(f"[PASS] {name}")
                passed += 1
            else:
                print(f"[FAIL] {name}")
                failed += 1
        except Exception as e:
            print(f"[FAIL] {name}: {e}")
            failed += 1

    print("\n" + "=" * 50)
    print(f"Results: {passed} passed, {failed} failed")
    print("=" * 50)

    return failed == 0


if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)
