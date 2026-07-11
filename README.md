# dynimg

A fast library and CLI for rendering HTML/CSS to images. Use from Python, Rust, or the command line. Built on [Blitz](https://github.com/DioxusLabs/blitz), a modular Rust rendering engine.

Perfect for generating dynamic images like Open Graph (OG) images, social media cards, email headers, and more.

## Example Output

<table>
<tr>
<td><img src="https://raw.githubusercontent.com/blopker/dynimg/main/tests/snapshots/macos/og-image.png" width="400" alt="OG Image Example"></td>
<td><img src="https://raw.githubusercontent.com/blopker/dynimg/main/tests/snapshots/macos/social-card.png" width="400" alt="Social Card Example"></td>
</tr>
</table>

## Features

- **Python + Rust + CLI**: Use from Python, as a Rust library, or command-line tool
- **Multiple output formats**: PNG, WebP (lossless), and JPEG
- **Transparent backgrounds**: PNG and WebP support transparency (JPEG uses white)
- **High-quality rendering**: Configurable scale factor for retina displays
- **Fast**: Native Rust performance with no browser overhead
- **Secure by default**: Network and filesystem access disabled unless explicitly enabled

## Installation

### Python

```bash
pip install dynimg
```

### Rust CLI

```bash
cargo install dynimg
```

### Rust Library

```toml
[dependencies]
dynimg = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Library Usage

```rust
use dynimg::{render, RenderOptions};

#[tokio::main]
async fn main() -> Result<(), dynimg::Error> {
    let html = r#"
        <html>
        <body style="background: linear-gradient(135deg, #667eea, #764ba2);
                     display: flex; justify-content: center; align-items: center;
                     height: 630px; margin: 0;">
            <h1 style="color: white; font-family: system-ui; font-size: 64px;">
                Hello World
            </h1>
        </body>
        </html>
    "#;

    // Render with default options (1200×auto viewport, 2x scale)
    let image = render(html, RenderOptions::default()).await?;

    // Save as PNG
    image.save_png("output.png")?;

    // Or get raw bytes
    let png_bytes = image.to_png()?;
    let webp_bytes = image.to_webp();
    let jpeg_bytes = image.to_jpeg(90)?;

    Ok(())
}
```

### Configuration

```rust
use dynimg::RenderOptions;

// Using builder pattern
let options = RenderOptions::default()
    .width(1200)
    .height(630)
    .scale(2.0)
    .allow_net()
    .assets_dir("./assets")
    .background("#ffffff")  // CSS hex color (default: transparent)
    .verbose()              // Show dependency output on stderr
    .font_file("./fonts/Inter-Regular.ttf")?;  // Custom font (TTF/OTF/WOFF/WOFF2)

// Or struct initialization
let options = RenderOptions {
    width: 1200,
    height: Some(630),
    scale: 2.0,
    allow_net: true,
    assets_dir: Some("./assets".into()),
    base_url: None,
    background: Some("#ffffff".into()),  // None = transparent
    verbose: false,                      // Suppress dependency output (default)
};
```

### Convenience function

```rust
use dynimg::{render_to_file, RenderOptions};

// Render directly to a file (format detected from extension)
render_to_file(html, "output.png", RenderOptions::default(), 90).await?;
```

## CLI Usage

### Basic Usage

Render an HTML file to PNG:

```bash
dynimg input.html -o output.png
```

### Output Formats

```bash
# PNG (lossless, supports transparency)
dynimg input.html -o image.png

# WebP (lossless, supports transparency)
dynimg input.html -o image.webp

# JPEG (lossy, no transparency - uses white background)
dynimg input.html -o image.jpg --quality 90
```

### Transparent Backgrounds

PNG and WebP output supports transparent backgrounds. Simply don't set a background on your HTML body:

```html
<body style="padding: 20px;">
  <div style="background: white; border-radius: 8px; padding: 20px;">
    Content with transparent surrounding area
  </div>
</body>
```

<img src="https://raw.githubusercontent.com/blopker/dynimg/main/tests/snapshots/macos/transparent.png" width="300" alt="Transparent background example">

JPEG doesn't support transparency, so it automatically uses a white background.

### Image Dimensions

The `--width` and `--height` options set the **viewport size** (CSS layout dimensions). The actual output image is scaled by the `--scale` factor (default: 2x for high-DPI/retina displays).

**Output image size = viewport × scale**

```bash
# Default: 1200px viewport → 2400px output (at 2x scale)
dynimg input.html -o output.png

# OG image: 1200×630 viewport → 2400×1260 output
dynimg input.html -o output.png --width 1200 --height 630

# 1x scale for exact pixel dimensions (1200×630 output)
dynimg input.html -o output.png --width 1200 --height 630 --scale 1

# 3x scale for extra-high-DPI (3600×1890 output)
dynimg input.html -o output.png --width 1200 --height 630 --scale 3
```

Your HTML/CSS should use the viewport dimensions (e.g., `width: 1200px`) - the scale factor handles the high-resolution rendering.

### Reading from stdin

```bash
echo '<html><body><h1>Hello</h1></body></html>' | dynimg - -o output.png
```

### Loading External Resources

By default, network and filesystem access are disabled for security. Enable them to load images, fonts, and other resources:

```bash
# Load images/fonts from URLs
dynimg input.html -o output.png --allow-net

# Load images/fonts from a local assets directory
dynimg input.html -o output.png --assets ./assets

# Allow both
dynimg input.html -o output.png --allow-net --assets ./assets
```

### Custom Fonts

Register font files or directories directly — no `@font-face` or asset directory needed. The font's family name (read from the file) matches `font-family` in CSS:

```bash
dynimg input.html -o output.png --font Inter-Regular.ttf --font Inter-Bold.ttf

# Or register everything in a directory (recursive)
dynimg input.html -o output.png --font ./fonts
```

```html
<h1 style="font-family: 'Inter'">Uses the registered font</h1>
```

Registered fonts take priority over system fonts with the same family name.

Fonts can also be registered under a CSS name with `NAME=FILE`. A generic name (`serif`, `sans-serif`, `monospace`, `cursive`, `fantasy`, `system-ui`, `emoji`, `math`) maps that generic to the font, ahead of the platform mapping — `emoji` replaces the platform emoji font, so emoji render identically on every host. Any other name becomes the font's family name:

```bash
dynimg input.html -o output.png \
  --font sans-serif=Inter-Regular.ttf \
  --font emoji=Twemoji.ttf \
  --font brand=FancyDisplay.ttf   # matches font-family: 'brand'
```

When using `--assets`, all local paths are resolved relative to the asset directory. Attempts to load files outside this directory will error:

```html
<!-- With --assets ./assets -->
<img src="logo.png">         <!-- loads ./assets/logo.png -->
<img src="img/hero.png">     <!-- loads ./assets/img/hero.png -->
<img src="../secret.png">    <!-- ERROR: outside assets directory -->
```

For self-contained templates, consider using inline base64 data URIs instead:

```html
<img src="data:image/png;base64,iVBORw0KGgo...">
```

### Minimal containers (Docker)

System font discovery on Linux uses fontconfig when it's installed, but it's optional — dynimg loads it dynamically and degrades gracefully when it's missing. In a minimal container you don't need fontconfig, libexpat, or `fc-cache`; just pass your fonts directly:

```python
options = dynimg.RenderOptions(fonts=["/app/static/fonts"])
```

When no system fonts are discoverable, registered fonts also back generic CSS families (`sans-serif`, `serif`, `monospace`, ...), so unstyled text still renders.

Name mappings pin the generic families and emoji to your fonts ahead of the platform mapping, making rendering identical on macOS, Linux, and in containers — no fontconfig aliasing required:

```python
options = dynimg.RenderOptions(
    fonts=[
        "/app/static/fonts",
        {
            "sans-serif": "/app/static/fonts/Montserrat.ttf",
            "emoji": "/app/static/fonts/TwemojiCOLRv0.ttf",
        },
    ],
)
```

## CLI Reference

```
dynimg [OPTIONS] <INPUT> -o <OUTPUT>

Arguments:
  <INPUT>   HTML file path or '-' for stdin

Options:
  -o, --output <FILE>       Output image path (format detected from extension)
  -w, --width <PIXELS>      Viewport width in CSS pixels [default: 1200]
  -H, --height <PIXELS>     Viewport height in CSS pixels [default: document height]
  -s, --scale <FACTOR>      Scale multiplier for output (2 = 2x resolution) [default: 2]
  -q, --quality <1-100>     JPEG quality [default: 90]
      --allow-net           Allow network access for loading remote resources
      --assets <DIR>        Asset directory for local resources
      --font <[NAME=]PATH>  Custom font file or directory (TTF/OTF/WOFF/WOFF2),
                            repeatable. NAME= registers under a CSS name:
                            generics (sans-serif, emoji, ...) map that generic,
                            other names become the font-family name
  -v, --verbose             Enable verbose logging and show dependency output on stderr
      --help                Print help
      --version             Print version

Options can also be set via HTML meta tags (see below). CLI flags override meta tags.

Note: Output image dimensions = viewport × scale. A 1200×630 viewport at 2x scale produces a 2400×1260 image.
```

## Python Usage

```python
import dynimg

html = """
<html>
<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}
</style>
<body style="background: linear-gradient(135deg, #667eea, #764ba2);
             display: flex; justify-content: center; align-items: center;
             height: 630px; margin: 0;">
    <h1 style="color: white; font-family: system-ui; font-size: 64px;">
        Hello World
    </h1>
</body>
</html>
"""

# Render with default options
image = dynimg.render(html)

# Save to file
image.save("output.png")

# Or get bytes
png_bytes = image.to_png()
webp_bytes = image.to_webp()
jpeg_bytes = image.to_jpeg(quality=90)
```

### Configuration

```python
import dynimg

options = dynimg.RenderOptions(
    width=1200,          # Viewport width (default: 1200)
    height=630,          # Viewport height (default: auto)
    scale=2.0,           # Output scale factor (default: 2.0)
    allow_net=True,      # Allow network requests (default: False)
    assets_dir="./assets",  # Local assets directory (default: None)
    base_url="https://example.com",  # Base URL for relative URLs (default: None)
    background="#ffffff",  # CSS hex color (default: transparent)
    verbose=False,       # Show dependency output on stderr (default: False)
    # Custom fonts: file paths, directories, bytes, or {css_name: font} mappings (default: None)
    fonts=["./fonts/dir", woff2_bytes, {"emoji": "./fonts/TwemojiCOLRv0.ttf"}],
)

image = dynimg.render(html, options)
```

### Direct File Output

```python
# Render directly to a file (format detected from extension)
dynimg.render_to_file(html, "output.png")

# With options
dynimg.render_to_file(html, "output.png", options=options)

# JPEG with quality setting
dynimg.render_to_file(html, "output.jpg", quality=90)
```

### Image Properties

```python
image = dynimg.render(html)
print(f"Size: {image.width}x{image.height}")
print(f"Bytes: {len(image.data)}")
```

## HTML Meta Tags

You can configure rendering options directly in your HTML using meta tags. CLI flags take precedence over meta tags.

```html
<meta name="dynimg:width" content="1200">   <!-- viewport width -->
<meta name="dynimg:height" content="630">   <!-- viewport height -->
<meta name="dynimg:scale" content="2">      <!-- output multiplier -->
<meta name="dynimg:quality" content="90">   <!-- JPEG quality -->
```

This is useful for templates that should always render at specific dimensions. Remember: the output image size is viewport × scale.

## Example Templates

### OG Image

<img src="https://raw.githubusercontent.com/blopker/dynimg/main/tests/snapshots/macos/og-image.png" width="500" alt="OG Image Example">

<details>
<summary>View HTML</summary>

```html
<!DOCTYPE html>
<html>
<head>
  <meta name="dynimg:width" content="1200">
  <meta name="dynimg:height" content="630">
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { font-family: system-ui, -apple-system, sans-serif; }
    .container {
      width: 1200px;
      height: 630px;
      display: flex;
      flex-direction: column;
      justify-content: center;
      padding: 80px;
      background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    }
    h1 { color: white; font-size: 72px; font-weight: 700; line-height: 1.1; margin-bottom: 24px; }
    p { color: rgba(255, 255, 255, 0.85); font-size: 32px; }
    .footer { position: absolute; bottom: 40px; left: 80px; color: rgba(255, 255, 255, 0.6); font-size: 24px; }
  </style>
</head>
<body>
  <div class="container">
    <h1>Your Blog Post Title Here</h1>
    <p>A compelling subtitle that captures attention</p>
    <div class="footer">yoursite.com</div>
  </div>
</body>
</html>
```
</details>

### Social Card

<img src="https://raw.githubusercontent.com/blopker/dynimg/main/tests/snapshots/macos/social-card.png" width="500" alt="Social Card Example">

<details>
<summary>View HTML</summary>

```html
<!DOCTYPE html>
<html>
<head>
  <meta name="dynimg:width" content="1200">
  <meta name="dynimg:height" content="675">
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body { font-family: system-ui, -apple-system, sans-serif; }
    .card {
      width: 1200px;
      height: 675px;
      background: #0f172a;
      display: flex;
      padding: 60px;
    }
    .content { flex: 1; display: flex; flex-direction: column; justify-content: center; }
    .tag {
      display: inline-block;
      background: #3b82f6;
      color: white;
      padding: 8px 16px;
      border-radius: 20px;
      font-size: 18px;
      font-weight: 600;
      margin-bottom: 24px;
      width: fit-content;
    }
    h1 { color: white; font-size: 56px; font-weight: 700; line-height: 1.2; margin-bottom: 20px; }
    p { color: #94a3b8; font-size: 24px; line-height: 1.5; }
    .avatar {
      width: 200px;
      height: 200px;
      background: linear-gradient(135deg, #06b6d4, #3b82f6);
      border-radius: 100px;
      display: flex;
      align-items: center;
      justify-content: center;
      align-self: center;
      margin-left: 60px;
    }
    .avatar-text { color: white; font-size: 72px; font-weight: 700; }
  </style>
</head>
<body>
  <div class="card">
    <div class="content">
      <span class="tag">Tutorial</span>
      <h1>Getting Started with Rust</h1>
      <p>Learn the fundamentals of Rust programming language with practical examples.</p>
    </div>
    <div class="avatar">
      <span class="avatar-text">RS</span>
    </div>
  </div>
</body>
</html>
```
</details>

## Supported CSS Features

dynimg uses Blitz for rendering, which supports:

- Flexbox and Grid layouts
- CSS variables
- Media queries
- Complex selectors
- Gradients and shadows
- Web fonts (via `@font-face`, requires `--allow-net` or `--assets`)
- Images (requires `--allow-net` or `--assets`, or use data URIs)

## Performance

dynimg is designed for speed:

- No browser startup overhead
- Native Rust rendering pipeline
- Efficient image encoding

Typical rendering time: 50-200ms depending on complexity.

## Development

### Building

```bash
# Build CLI
cargo build --release

# Build Python wheel
pip install maturin
maturin build --release --features python

# Install locally for development
maturin develop --features python
```

### Running Tests

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt -- --check
```

## Releasing

Releases are automated via a release script and GitHub Actions. To create a new release:

```bash
./scripts/release.sh
```

The script will:
1. Verify you're on the main branch with a clean working directory
2. Check that you're up to date with the remote
3. Bump the patch version in `Cargo.toml` and `pyproject.toml`
4. Commit the version changes and create an annotated tag
5. Push the commit and tag to origin

This triggers the GitHub Actions release workflow which:
- Builds wheels for Linux (x86_64, aarch64) and macOS (x86_64, aarch64)
- Creates a GitHub Release with all artifacts
- Publishes to PyPI

## License

MIT

## AI Warning

This is AI slop, if you want to use it, fork and make it your own!
