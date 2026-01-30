# dynimg

A fast library and CLI for rendering HTML/CSS to images. Use from Python, Rust, or the command line. Built on [Blitz](https://github.com/DioxusLabs/blitz), a modular Rust rendering engine.

Perfect for generating dynamic images like Open Graph (OG) images, social media cards, email headers, and more.

## Features

- **Python + Rust + CLI**: Use from Python, as a Rust library, or command-line tool
- **Multiple output formats**: PNG, WebP (lossless), and JPEG
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
    .assets_dir("./assets");

// Or struct initialization
let options = RenderOptions {
    width: 1200,
    height: Some(630),
    scale: 2.0,
    allow_net: true,
    assets_dir: Some("./assets".into()),
    base_url: None,
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
# PNG (lossless)
dynimg input.html -o image.png

# WebP (lossless)
dynimg input.html -o image.webp

# JPEG (lossy)
dynimg input.html -o image.jpg --quality 90
```

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
  -v, --verbose             Enable verbose logging
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

## Example HTML Template

```html
<!DOCTYPE html>
<html>
<head>
  <meta name="dynimg:width" content="1200">
  <meta name="dynimg:height" content="630">
  <style>
    * {
        margin: 0;
        padding: 0;
        box-sizing: border-box;
    }
    .container {
      width: 1200px;
      height: 630px;
      display: flex;
      flex-direction: column;
      justify-content: center;
      align-items: center;
      background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
      font-family: system-ui, sans-serif;
    }
    h1 {
      color: white;
      font-size: 64px;
      margin: 0;
    }
    p {
      color: rgba(255,255,255,0.8);
      font-size: 32px;
    }
  </style>
</head>
<body>
  <div class="container">
    <h1>Hello World</h1>
    <p>Welcome to my site</p>
  </div>
</body>
</html>
```

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

Releases are automated via GitHub Actions. To create a new release:

1. Update the version in `Cargo.toml`
2. Create and push a git tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

This triggers the release workflow which:
- Builds wheels for Linux (x86_64, aarch64) and macOS (x86_64, aarch64)
- Creates a GitHub Release with all artifacts
- (Optional) Publishes to PyPI (when enabled)

## License

MIT

## AI Warning

This is AI slop, if you want to use it, fork and make it your own!
