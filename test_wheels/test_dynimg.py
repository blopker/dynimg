#!/usr/bin/env python3
"""Test script for dynimg wheels."""

import sys


def test_import():
    """Test basic import."""
    import dynimg

    print(f"dynimg version: {dynimg.__version__}")
    print(f"Module: {dynimg}")
    return True


def test_render_basic():
    """Test basic rendering."""
    import dynimg

    html = '<html><body style="background:blue;"><h1>Test</h1></body></html>'
    img = dynimg.render(html, dynimg.RenderOptions(width=100, height=100, scale=1.0))

    assert img.width == 100, f"Expected width 100, got {img.width}"
    assert img.height == 100, f"Expected height 100, got {img.height}"
    print(f"Basic render: {img.width}x{img.height}")
    return True


def test_render_gradient():
    """Test gradient rendering."""
    import dynimg

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
    img = dynimg.render(html, options)

    assert img.width == 2400, f"Expected width 2400, got {img.width}"
    assert img.height == 1260, f"Expected height 1260, got {img.height}"
    print(f"Gradient render: {img.width}x{img.height}")
    return True


def test_save_formats():
    """Test saving to different formats."""
    import os

    import dynimg

    html = '<html><body style="background:red; width:50px; height:50px;"></body></html>'
    img = dynimg.render(html, dynimg.RenderOptions(width=50, height=50, scale=1.0))

    # Test PNG
    img.save_png("test_output.png")
    assert os.path.exists("test_output.png"), "PNG file not created"
    png_size = os.path.getsize("test_output.png")
    print(f"PNG saved: {png_size} bytes")

    # Test WebP
    img.save_webp("test_output.webp")
    assert os.path.exists("test_output.webp"), "WebP file not created"
    webp_size = os.path.getsize("test_output.webp")
    print(f"WebP saved: {webp_size} bytes")

    # Test JPEG
    img.save_jpeg("test_output.jpg", quality=90)
    assert os.path.exists("test_output.jpg"), "JPEG file not created"
    jpeg_size = os.path.getsize("test_output.jpg")
    print(f"JPEG saved: {jpeg_size} bytes")

    # Cleanup
    os.remove("test_output.png")
    os.remove("test_output.webp")
    os.remove("test_output.jpg")

    return True


def test_to_bytes():
    """Test encoding to bytes."""
    import dynimg

    html = (
        '<html><body style="background:green; width:50px; height:50px;"></body></html>'
    )
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
    import os

    import dynimg

    html = (
        '<html><body style="background:yellow; width:50px; height:50px;"></body></html>'
    )

    dynimg.render_to_file(html, "test_direct.png")
    assert os.path.exists("test_direct.png"), "Direct PNG not created"
    print(f"render_to_file PNG: {os.path.getsize('test_direct.png')} bytes")
    os.remove("test_direct.png")

    dynimg.render_to_file(
        html,
        "test_direct.webp",
        options=dynimg.RenderOptions(width=100, height=100, scale=1.0),
        quality=85,
    )
    assert os.path.exists("test_direct.webp"), "Direct WebP not created"
    print(f"render_to_file WebP: {os.path.getsize('test_direct.webp')} bytes")
    os.remove("test_direct.webp")

    return True


def main():
    """Run all tests."""
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
