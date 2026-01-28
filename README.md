# dynimg

A fast CLI tool for generating high-quality images from HTML/CSS. Built on [Blitz](https://github.com/DioxusLabs/blitz), a modular Rust rendering engine.

Perfect for generating dynamic images like Open Graph (OG) images, social media cards, email headers, and more.

## Features

- **Multiple output formats**: PNG, WebP, and JPEG
- **High-quality rendering**: 2x resolution scaling for crisp images
- **Fast**: Native Rust performance with no browser overhead
- **Flexible sizing**: Configurable width, height, and scale factor
- **Secure by default**: Network and filesystem access disabled unless explicitly enabled

## Installation

```bash
cargo install dynimg
```

Or build from source:

```bash
git clone https://github.com/blopker/dynimg
cd dynimg
cargo build --release
```

## Usage

### Basic Usage

Render an HTML file to PNG:

```bash
dynimg input.html -o output.png
```

### Output Formats

```bash
# PNG (default, lossless)
dynimg input.html -o image.png

# WebP (smaller file size)
dynimg input.html -o image.webp

# JPEG (with quality setting)
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
  -q, --quality <1-100>     JPEG/WebP quality [default: 90]
      --allow-net           Allow network access for loading remote resources
      --assets <DIR>        Asset directory for local resources
  -v, --verbose             Enable verbose logging
      --help                Print help
      --version             Print version

Options can also be set via HTML meta tags (see below). CLI flags override meta tags.

Note: Output image dimensions = viewport × scale. A 1200×630 viewport at 2x scale produces a 2400×1260 image.
```

## HTML Meta Tags

You can configure rendering options directly in your HTML using meta tags. CLI flags take precedence over meta tags.

```html
<meta name="dynimg:width" content="1200">   <!-- viewport width -->
<meta name="dynimg:height" content="630">   <!-- viewport height -->
<meta name="dynimg:scale" content="2">      <!-- output multiplier -->
<meta name="dynimg:quality" content="90">   <!-- JPEG/WebP quality -->
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

## License

MIT
