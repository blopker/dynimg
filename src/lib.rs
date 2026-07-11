//! # dynimg
//!
//! A fast library for rendering HTML/CSS to images.
//!
//! ## Example
//!
//! ```no_run
//! use dynimg::{render, RenderOptions};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), dynimg::Error> {
//!     let html = r#"
//!         <html>
//!         <body style="background: #4f46e5; padding: 40px;">
//!             <h1 style="color: white; font-family: sans-serif;">Hello World</h1>
//!         </body>
//!         </html>
//!     "#;
//!
//!     let image = render(html, RenderOptions::default()).await?;
//!     println!("Rendered {}x{} image", image.width, image.height);
//!
//!     // Save to file
//!     image.save_png("output.png")?;
//!
//!     Ok(())
//! }
//! ```

#[cfg(feature = "python")]
mod python;

#[cfg(feature = "python")]
pub use python::_dynimg;

use anyrender::{PaintScene as _, render_to_buffer};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::{BaseDocument, DocumentConfig, StyleThreading, util::Color};
use blitz_html::HtmlDocument;
use blitz_net::Provider;
use blitz_paint::paint_scene;
use blitz_traits::net::{NetHandler, NetProvider, Request};
use blitz_traits::shell::{ColorScheme, Viewport};
use bytes::Bytes;
use kurbo::Rect;
use peniko::Fill;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur during rendering
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("PNG encoding error: {0}")]
    PngEncoding(#[from] png::EncodingError),

    #[error("JPEG encoding error: {0}")]
    JpegEncoding(#[from] zenjpeg::encoder::Error),

    #[error("Invalid image buffer")]
    InvalidBuffer,
}

/// Options for rendering HTML to an image
#[derive(Debug, Clone)]
pub struct RenderOptions {
    /// Viewport width in CSS pixels (default: 1200)
    pub width: u32,

    /// Viewport height in CSS pixels. If None, auto-sizes to content height.
    pub height: Option<u32>,

    /// Scale factor for output resolution (default: 2.0 for retina displays).
    /// Output dimensions = viewport × scale
    pub scale: f32,

    /// Allow network requests for loading remote resources (images, fonts, etc.)
    pub allow_net: bool,

    /// Directory for loading local assets. Paths are sandboxed to this directory.
    pub assets_dir: Option<PathBuf>,

    /// Base URL for resolving relative paths. If None, uses assets_dir or current directory.
    pub base_url: Option<String>,

    /// Background color as a CSS hex string, e.g. "#ffffff" (default: transparent).
    /// Automatically set to white for JPEG output via CLI and `render_to_file()`.
    pub background: Option<String>,

    /// Enable verbose output. When false (default), stdout and stderr from
    /// dependencies are suppressed. When true, captured output is forwarded to stderr.
    pub verbose: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            width: 1200,
            height: None,
            scale: 2.0,
            allow_net: false,
            assets_dir: None,
            base_url: None,
            background: None,
            verbose: false,
        }
    }
}

impl RenderOptions {
    /// Create options with a specific viewport size
    pub fn with_size(width: u32, height: u32) -> Self {
        Self {
            width,
            height: Some(height),
            ..Default::default()
        }
    }

    /// Set the viewport width
    pub fn width(mut self, width: u32) -> Self {
        self.width = width;
        self
    }

    /// Set the viewport height
    pub fn height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self
    }

    /// Set the scale factor
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Enable network access for remote resources
    pub fn allow_net(mut self) -> Self {
        self.allow_net = true;
        self
    }

    /// Set the assets directory for local resources
    pub fn assets_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.assets_dir = Some(path.into());
        self
    }

    /// Set the base URL for resolving relative paths
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the background color as a CSS hex string, e.g. "#ffffff" (default: transparent)
    pub fn background(mut self, color: impl Into<String>) -> Self {
        self.background = Some(color.into());
        self
    }

    /// Enable verbose output (forward dependency stdout/stderr to stderr)
    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }
}

/// A rendered image with RGBA pixel data
#[derive(Debug, Clone)]
pub struct RenderedImage {
    /// Raw RGBA pixel data (4 bytes per pixel)
    pub data: Vec<u8>,

    /// Image width in pixels
    pub width: u32,

    /// Image height in pixels
    pub height: u32,
}

impl RenderedImage {
    /// Save the image as PNG
    pub fn save_png(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        write_png(path.as_ref(), &self.data, self.width, self.height)
    }

    /// Save the image as JPEG with the specified quality (1-100)
    pub fn save_jpeg(&self, path: impl AsRef<Path>, quality: u8) -> Result<(), Error> {
        write_jpeg(path.as_ref(), &self.data, self.width, self.height, quality)
    }

    /// Save the image as lossless WebP
    pub fn save_webp(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        write_webp_lossless(path.as_ref(), &self.data, self.width, self.height)
    }

    /// Encode the image as PNG bytes
    pub fn to_png(&self) -> Result<Vec<u8>, Error> {
        encode_png(&self.data, self.width, self.height)
    }

    /// Encode the image as JPEG bytes with the specified quality (1-100)
    pub fn to_jpeg(&self, quality: u8) -> Result<Vec<u8>, Error> {
        encode_jpeg(&self.data, self.width, self.height, quality)
    }

    /// Encode the image as lossless WebP bytes
    pub fn to_webp(&self) -> Vec<u8> {
        encode_webp_lossless(&self.data, self.width, self.height)
    }
}

/// Render HTML to an image
///
/// # Example
///
/// ```no_run
/// use dynimg::{render, RenderOptions};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), dynimg::Error> {
/// let html = "<html><body><h1>Hello</h1></body></html>";
/// let image = render(html, RenderOptions::default()).await?;
/// image.save_png("output.png")?;
/// # Ok(())
/// # }
/// ```
pub async fn render(html: &str, options: RenderOptions) -> Result<RenderedImage, Error> {
    // Create provider for assets and/or network.
    // Always created so unresolvable requests get empty responses,
    // preventing Blitz's FOUC prevention from blocking rendering.
    let provider = Arc::new(CombinedProvider::new(
        options.assets_dir.clone(),
        options.allow_net,
    ));

    // Build base URL for asset resolution
    let base_url = options.base_url.clone().or_else(|| {
        options
            .assets_dir
            .as_ref()
            .and_then(|p| p.canonicalize().ok())
            .or_else(|| std::env::current_dir().ok())
            .map(|p| format!("file://{}/", p.display()))
    });

    // Parse document
    let mut document = HtmlDocument::from_html(
        html,
        DocumentConfig {
            base_url,
            net_provider: Some(provider.clone() as _),
            viewport: None,
            // Sequential bypasses Stylo's global rayon pool, which panics with
            // "already mutably borrowed" when two documents resolve in parallel.
            // This makes concurrent render() calls safe without a global lock.
            style_threading: StyleThreading::Sequential,
            ..Default::default()
        },
    );

    // Extract meta options and merge with provided options
    let meta_options = extract_meta_options(document.as_ref());
    let width = meta_options.width.unwrap_or(options.width);
    let height = meta_options.height.or(options.height);
    let scale = meta_options.scale.unwrap_or(options.scale);

    document.set_viewport(Viewport::new(
        width * (scale as u32),
        height.unwrap_or(800) * (scale as u32),
        scale,
        ColorScheme::Light,
    ));

    // Render the document
    let background = options
        .background
        .as_deref()
        .map(parse_hex_color)
        .unwrap_or(Color::TRANSPARENT);

    render_document(&mut document, &provider, width, height, scale, background).await
}

/// Render HTML and save directly to a file.
/// The output format is detected from the file extension.
///
/// # Example
///
/// ```no_run
/// use dynimg::{render_to_file, RenderOptions};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), dynimg::Error> {
/// let html = "<html><body><h1>Hello</h1></body></html>";
/// render_to_file(html, "output.png", RenderOptions::default(), 90).await?;
/// # Ok(())
/// # }
/// ```
pub async fn render_to_file(
    html: &str,
    path: impl AsRef<Path>,
    options: RenderOptions,
    quality: u8,
) -> Result<(), Error> {
    let path = path.as_ref();

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    // Use white background for JPEG (no transparency support)
    let options =
        if matches!(ext.as_deref(), Some("jpg") | Some("jpeg")) && options.background.is_none() {
            RenderOptions {
                background: Some("#ffffff".to_string()),
                ..options
            }
        } else {
            options
        };

    let image = render(html, options).await?;

    match ext.as_deref() {
        Some("png") => image.save_png(path),
        Some("jpg") | Some("jpeg") => image.save_jpeg(path, quality),
        Some("webp") => image.save_webp(path),
        _ => image.save_png(path), // Default to PNG
    }
}

// ============================================================================
// Internal implementation
// ============================================================================

/// Parse a CSS hex color string (e.g. "#ffffff" or "#fff") to a Color.
/// Falls back to transparent if parsing fails.
fn parse_hex_color(s: &str) -> Color {
    let s = s.trim().trim_start_matches('#');

    let (r, g, b) = match s.len() {
        // #rgb format
        3 => {
            let r = u8::from_str_radix(&s[0..1], 16).unwrap_or(0);
            let g = u8::from_str_radix(&s[1..2], 16).unwrap_or(0);
            let b = u8::from_str_radix(&s[2..3], 16).unwrap_or(0);
            (r * 17, g * 17, b * 17) // Expand #rgb to #rrggbb
        }
        // #rrggbb format
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(0);
            (r, g, b)
        }
        _ => return Color::TRANSPARENT,
    };

    // Color components are f32 in 0.0-1.0 range
    Color::new([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
}

/// Options extracted from HTML meta tags
#[derive(Debug, Default)]
struct MetaOptions {
    width: Option<u32>,
    height: Option<u32>,
    scale: Option<f32>,
    quality: Option<u8>,
}

/// Extract dynimg meta tags from a parsed document
fn extract_meta_options(doc: &BaseDocument) -> MetaOptions {
    let mut options = MetaOptions::default();
    let mut stack = vec![0usize];

    while let Some(node_id) = stack.pop() {
        let Some(node) = doc.get_node(node_id) else {
            continue;
        };

        stack.extend(node.children.iter().copied());

        let Some(element) = node.element_data() else {
            continue;
        };

        if !element.name.local.eq_str_ignore_ascii_case("meta") {
            continue;
        }

        let mut name_value: Option<&str> = None;
        let mut content_value: Option<&str> = None;

        for attr in element.attrs.iter() {
            if attr.name.local.eq_str_ignore_ascii_case("name") {
                name_value = Some(&attr.value);
            } else if attr.name.local.eq_str_ignore_ascii_case("content") {
                content_value = Some(&attr.value);
            }
        }

        let (Some(name), Some(content)) = (name_value, content_value) else {
            continue;
        };

        match name {
            "dynimg:width" => options.width = content.parse().ok(),
            "dynimg:height" => options.height = content.parse().ok(),
            "dynimg:scale" => options.scale = content.parse().ok(),
            "dynimg:quality" => options.quality = content.parse().ok(),
            _ => {}
        }
    }

    options
}

/// A NetProvider that serves files from a sandboxed assets directory
struct AssetProvider {
    assets_dir: PathBuf,
}

impl AssetProvider {
    fn new(assets_dir: PathBuf) -> Self {
        Self { assets_dir }
    }

    fn resolve_path(&self, url: &str) -> Option<PathBuf> {
        let path_str = if let Some(stripped) = url.strip_prefix("file://") {
            stripped
        } else if url.starts_with("http://") || url.starts_with("https://") {
            return None;
        } else {
            url
        };

        let requested_path = Path::new(path_str);
        let full_path = if requested_path.is_absolute() {
            requested_path.to_path_buf()
        } else {
            self.assets_dir.join(requested_path)
        };

        let canonical = full_path.canonicalize().ok()?;
        let assets_canonical = self.assets_dir.canonicalize().ok()?;

        if canonical.starts_with(&assets_canonical) {
            Some(canonical)
        } else {
            None
        }
    }
}

impl NetProvider for AssetProvider {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url.to_string();
        if let Some(path) = self.resolve_path(&url)
            && let Ok(data) = fs::read(&path)
        {
            handler.bytes(url, Bytes::from(data));
        }
    }
}

/// Combined provider for assets and network requests
struct CombinedProvider {
    assets: Option<AssetProvider>,
    network: Option<Arc<Provider>>,
}

impl CombinedProvider {
    fn new(assets_dir: Option<PathBuf>, allow_net: bool) -> Self {
        Self {
            assets: assets_dir.map(AssetProvider::new),
            network: if allow_net {
                Some(Arc::new(Provider::new(None)))
            } else {
                None
            },
        }
    }

    fn is_empty(&self) -> bool {
        self.network.as_ref().map(|n| n.is_empty()).unwrap_or(true)
    }
}

impl NetProvider for CombinedProvider {
    fn fetch(&self, doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let scheme = request.url.scheme();

        // Non-HTTP(S) URLs (e.g. file://) are local resources that need the assets provider
        if scheme != "http" && scheme != "https" {
            if let Some(ref assets) = self.assets {
                assets.fetch(doc_id, request, handler);
            } else {
                // Respond with empty bytes so Blitz's critical resource tracker
                // gets cleared (prevents blank renders from FOUC prevention)
                handler.bytes(request.url.to_string(), Bytes::new());
            }
            return;
        }

        if let Some(ref network) = self.network {
            network.fetch(doc_id, request, handler);
        }
    }
}

async fn render_document(
    document: &mut HtmlDocument,
    provider: &Arc<CombinedProvider>,
    width: u32,
    height: Option<u32>,
    scale: f32,
    background: Color,
) -> Result<RenderedImage, Error> {
    // Resolve resource requests
    // Wait for all network requests including cascading requests.
    // CSS stylesheets may trigger font fetches when processed, so we need
    // multiple consecutive "empty" checks to ensure all cascading requests complete.
    // Using 5 cycles provides safety margin for complex pages with many resources.
    let mut consecutive_empty = 0u32;
    const REQUIRED_EMPTY_CYCLES: u32 = 5;

    while consecutive_empty < REQUIRED_EMPTY_CYCLES {
        document.resolve(0.0);
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;

        if provider.is_empty() {
            consecutive_empty += 1;
        } else {
            consecutive_empty = 0;
        }
    }

    // Compute style and layout
    document.as_mut().resolve(0.0);

    // Determine final dimensions
    let computed_height = document.as_ref().root_element().final_layout.size.height;
    let render_height = height.unwrap_or_else(|| computed_height.ceil() as u32);

    let render_width = (width as f64 * scale as f64) as u32;
    let render_height_scaled = (render_height as f64 * scale as f64) as u32;

    // Render to RGBA buffer
    let buffer = render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| {
            scene.fill(
                Fill::NonZero,
                Default::default(),
                background,
                Default::default(),
                &Rect::new(0.0, 0.0, render_width as f64, render_height_scaled as f64),
            );

            paint_scene(
                scene,
                document.as_mut(),
                scale as f64,
                render_width,
                render_height_scaled,
                0,
                0,
            );
        },
        render_width,
        render_height_scaled,
    );

    Ok(RenderedImage {
        data: buffer,
        width: render_width,
        height: render_height_scaled,
    })
}

// ============================================================================
// Image encoding functions
// ============================================================================

fn write_png(path: &Path, buffer: &[u8], width: u32, height: u32) -> Result<(), Error> {
    let data = encode_png(buffer, width, height)?;
    fs::write(path, data)?;
    Ok(())
}

fn encode_png(buffer: &[u8], width: u32, height: u32) -> Result<Vec<u8>, Error> {
    const PPM: u32 = (144.0 * 39.3701) as u32;

    // Pre-allocate output (PNG is typically 10-50% of raw size after compression)
    let mut output = Vec::with_capacity(buffer.len() / 4);
    {
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: PPM,
            yppu: PPM,
            unit: png::Unit::Meter,
        }));

        let mut writer = encoder.write_header()?;
        writer.write_image_data(buffer)?;
        writer.finish()?;
    }
    Ok(output)
}

fn write_jpeg(
    path: &Path,
    buffer: &[u8],
    width: u32,
    height: u32,
    quality: u8,
) -> Result<(), Error> {
    let data = encode_jpeg(buffer, width, height, quality)?;
    fs::write(path, data)?;
    Ok(())
}

fn encode_jpeg(buffer: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>, Error> {
    use zenjpeg::encoder::{ChromaSubsampling, EncoderConfig, PixelLayout, Unstoppable};

    // Pre-allocate RGB buffer (3 bytes per pixel instead of 4)
    let pixel_count = (width * height) as usize;
    let mut rgb_buffer = Vec::with_capacity(pixel_count * 3);

    // Convert RGBA to RGB (drop alpha channel)
    for chunk in buffer.chunks_exact(4) {
        rgb_buffer.extend_from_slice(&chunk[..3]);
    }

    // Use zenjpeg for encoding - quality is 0-100, use 4:2:0 chroma subsampling for good compression
    let config = EncoderConfig::ycbcr(quality, ChromaSubsampling::Quarter);
    let mut encoder = config.encode_from_bytes(width, height, PixelLayout::Rgb8Srgb)?;
    encoder.push_packed(&rgb_buffer, Unstoppable)?;
    let jpeg = encoder.finish()?;

    Ok(jpeg)
}

fn write_webp_lossless(path: &Path, buffer: &[u8], width: u32, height: u32) -> Result<(), Error> {
    let data = encode_webp_lossless(buffer, width, height);
    fs::write(path, data)?;
    Ok(())
}

fn encode_webp_lossless(buffer: &[u8], width: u32, height: u32) -> Vec<u8> {
    let encoder = webp::Encoder::from_rgba(buffer, width, height);
    let mut config = webp::WebPConfig::new().unwrap();
    config.lossless = 1;
    config.quality = 75.0;
    config.method = 0; // 0=fastest, 6=slowest (default)
    let webp_data = encoder.encode_advanced(&config).unwrap();
    webp_data.to_vec()
}
