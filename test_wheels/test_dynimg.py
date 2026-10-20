"""Tests for the installed dynimg package (run against built wheels in CI)."""

from pathlib import Path

import dynimg
import pytest

FONTS_DIR = Path(__file__).parent.parent / "examples" / "assets" / "fonts"
FONT_PATH = FONTS_DIR / "Silkscreen-Regular.ttf"
EMOJI_FONT_PATH = FONTS_DIR / "TwemojiCOLRv0.ttf"

PNG_MAGIC = b"\x89PNG\r\n\x1a\n"
JPEG_MAGIC = b"\xff\xd8"
WEBP_MAGIC = b"RIFF"


def test_version():
    assert dynimg.__version__


def test_render_size(timer):
    html = '<html><body style="background:blue;"><h1>Test</h1></body></html>'
    with timer.measure("render"):
        img = dynimg.render(html, dynimg.RenderOptions(width=100, height=100, scale=1.0))
    assert (img.width, img.height) == (100, 100)
    assert len(img.data) == 100 * 100 * 4


def test_render_scale(timer):
    html = """
    <body style="background: linear-gradient(135deg, #667eea, #764ba2); margin: 0;">
        <h1 style="color: white; font-family: system-ui; font-size: 64px;">Hello World</h1>
    </body>
    """
    with timer.measure("render"):
        img = dynimg.render(html, dynimg.RenderOptions(width=1200, height=630, scale=2.0))
    assert (img.width, img.height) == (2400, 1260)
    with timer.measure("to_png"):
        img.to_png()
    with timer.measure("to_webp"):
        img.to_webp()
    with timer.measure("to_jpeg"):
        img.to_jpeg()


@pytest.fixture
def small_image():
    html = '<html><body style="background:red;"></body></html>'
    return dynimg.render(html, dynimg.RenderOptions(width=50, height=50, scale=1.0))


@pytest.mark.parametrize(
    ("save", "ext", "magic"),
    [
        (lambda img, p: img.save_png(p), "png", PNG_MAGIC),
        (lambda img, p: img.save_webp(p), "webp", WEBP_MAGIC),
        (lambda img, p: img.save_jpeg(p, quality=90), "jpg", JPEG_MAGIC),
    ],
    ids=["png", "webp", "jpeg"],
)
def test_save(small_image, out_dir, timer, save, ext, magic):
    path = out_dir / f"out.{ext}"
    with timer.measure("save"):
        save(small_image, str(path))
    assert path.read_bytes().startswith(magic)


def test_to_bytes(small_image):
    assert small_image.to_png().startswith(PNG_MAGIC)
    assert small_image.to_webp().startswith(WEBP_MAGIC)
    assert small_image.to_jpeg(quality=90).startswith(JPEG_MAGIC)


@pytest.mark.parametrize(
    ("ext", "magic"), [("png", PNG_MAGIC), ("webp", WEBP_MAGIC)], ids=["png", "webp"]
)
def test_render_to_file(out_dir, timer, ext, magic):
    html = '<html><body style="background:yellow;"></body></html>'
    path = out_dir / f"direct.{ext}"
    with timer.measure("render_to_file"):
        dynimg.render_to_file(
            html, str(path), options=dynimg.RenderOptions(width=100, height=100, scale=1.0)
        )
    assert path.read_bytes().startswith(magic)


def test_render_to_file_default_options(out_dir):
    html = '<html><body style="background:yellow;"><h1>Defaults</h1></body></html>'
    path = out_dir / "direct.jpg"
    dynimg.render_to_file(html, str(path))
    assert path.read_bytes().startswith(JPEG_MAGIC)


def test_render_to_file_jpeg_quality(out_dir):
    html = '<html><body style="background:linear-gradient(red,blue);"></body></html>'
    options = dynimg.RenderOptions(width=200, height=200, scale=1.0)
    low, high = out_dir / "low.jpg", out_dir / "high.jpg"
    dynimg.render_to_file(html, str(low), options=options, quality=20)
    dynimg.render_to_file(html, str(high), options=options, quality=95)
    assert low.stat().st_size < high.stat().st_size


FONT_HTML = (
    '<html><body style="margin:0; background:white;">'
    "<h1 style=\"font-family: 'Silkscreen'; font-size: 32px;\">Custom 0123</h1>"
    "</body></html>"
)


def render_png(html, width, height, fonts=None):
    options = dynimg.RenderOptions(width=width, height=height, scale=1.0, fonts=fonts)
    return dynimg.render(html, options).to_png()


def test_font_path_changes_rendering(timer):
    with timer.measure("render without font"):
        without = render_png(FONT_HTML, 300, 100)
    with timer.measure("render with font"):
        from_list = render_png(FONT_HTML, 300, 100, fonts=[str(FONT_PATH)])
    from_str = render_png(FONT_HTML, 300, 100, fonts=str(FONT_PATH))
    assert from_list != without
    assert from_str == from_list


def test_emoji_font_mapping(timer):
    # The text generic is mapped in BOTH renders: on fontconfig hosts an
    # unmapped generic expands to a platform list that can include an emoji
    # font (e.g. Noto), which would win over the emoji mapping. Pinning the
    # text generic is the documented pattern for fully pinned emoji.
    html = (
        '<html><body style="margin:0; background:white; font-size:32px;">'
        '<p style="font-family: sans-serif;">\N{GRINNING FACE}\N{ROCKET}</p>'
        "</body></html>"
    )
    text_only = {"sans-serif": str(FONT_PATH)}
    with_emoji = {**text_only, "emoji": str(EMOJI_FONT_PATH)}
    with timer.measure("render with emoji font"):
        emoji = render_png(html, 200, 80, fonts=with_emoji)
    assert emoji != render_png(html, 200, 80, fonts=text_only)


def test_font_name_mappings():
    html = (
        '<html><body style="margin:0; background:white; font-size:24px;">'
        '<p style="font-family: sans-serif;">Mapped sans-serif</p>'
        "<p style=\"font-family: 'brand';\">Custom-named font</p>"
        "</body></html>"
    )
    mixed = [str(FONT_PATH), {"sans-serif": str(FONT_PATH), "brand": str(FONT_PATH)}]
    assert render_png(html, 300, 120, fonts=mixed) != render_png(html, 300, 120)


def test_missing_font_raises():
    with pytest.raises(RuntimeError, match="(?i)font"):
        render_png(FONT_HTML, 300, 100, fonts=["/nonexistent/font.ttf"])


def test_invalid_font_raises(out_dir):
    not_a_font = out_dir / "not-a-font.ttf"
    not_a_font.write_bytes(b"definitely not a font")
    with pytest.raises(RuntimeError, match="(?i)font"):
        render_png(FONT_HTML, 300, 100, fonts=[str(not_a_font)])


META_HTML = """<html><head>
    <meta name="dynimg:width" content="40">
    <meta name="dynimg:height" content="20">
    <meta name="dynimg:scale" content="1">
    <meta name="dynimg:quality" content="42">
</head><body></body></html>"""


def test_meta_tags_apply_when_options_unset():
    img = dynimg.render(META_HTML)
    assert (img.width, img.height, img.quality) == (40, 20, 42)


def test_options_override_meta_tags():
    opts = dynimg.RenderOptions(width=30, scale=2.0, quality=80)
    assert opts.height is None and opts.width == 30
    img = dynimg.render(META_HTML, opts)
    # Height still comes from the meta tag, scaled by the explicit scale
    assert (img.width, img.height, img.quality) == (60, 40, 80)


def test_size_limit():
    html = """<html><head>
        <meta name="dynimg:width" content="8000">
        <meta name="dynimg:height" content="8000">
    </head><body></body></html>"""
    with pytest.raises(RuntimeError, match="16000x16000"):
        dynimg.render(html)


def test_nan_scale_raises():
    """Must be a normal exception, not a Rust panic (PanicException isn't an
    Exception subclass, so `except Exception` wouldn't catch it)."""
    html = '<meta name="dynimg:scale" content="NaN"><h1>x</h1>'
    with pytest.raises(RuntimeError, match="Invalid scale"):
        dynimg.render(html)


def test_cli(out_dir):
    """The `dynimg` console script ships with the wheel."""
    html = out_dir / "in.html"
    html.write_text('<body style="background:blue"></body>')
    out = out_dir / "out.png"
    assert dynimg.cli_main(["dynimg", str(html), "-o", str(out), "-w", "40", "-H", "20"]) == 0
    assert out.read_bytes().startswith(PNG_MAGIC)
    assert dynimg.cli_main(["dynimg", "--bogus"]) == 2
