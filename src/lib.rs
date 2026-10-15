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

mod net;

#[cfg(feature = "python")]
mod python;

#[cfg(feature = "python")]
pub use python::_dynimg;

use anyrender::{PaintScene as _, render_to_buffer};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::{
    BaseDocument, DocumentConfig, FontContext, StyleThreading, decode_font_bytes, local_name,
    util::Color,
};
use blitz_html::HtmlDocument;
use blitz_paint::paint_scene;
use blitz_traits::net::{NetHandler, NetProvider, Request, Url};
use blitz_traits::shell::{ColorScheme, Viewport};
use bytes::Bytes;
use data_url::DataUrl;
use kurbo::Rect;
use net::HttpProvider;
use peniko::{Blob, Fill};
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

    #[error("Failed to load font {0}: {1}")]
    FontLoad(PathBuf, String),

    #[error("Unsupported image format {0:?} (expected png, jpg, jpeg or webp)")]
    UnsupportedFormat(String),
}

/// Viewport width used when neither options nor a meta tag set one
pub const DEFAULT_WIDTH: u32 = 1200;
/// Scale factor used when neither options nor a meta tag set one
pub const DEFAULT_SCALE: f32 = 2.0;
/// JPEG quality used when neither options nor a meta tag set one
pub const DEFAULT_QUALITY: u8 = 90;

/// Output image format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    /// Lossy; has no alpha channel, so transparent areas need a background
    Jpeg,
    /// Lossless
    WebP,
}

impl ImageFormat {
    /// Parse a format name or file extension ("png", "jpg", "jpeg", "webp"),
    /// case-insensitively
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::WebP),
            _ => None,
        }
    }

    /// Detect the format from a file path's extension
    pub fn from_path(path: &Path) -> Result<Self, Error> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        Self::from_extension(ext).ok_or_else(|| Error::UnsupportedFormat(ext.to_string()))
    }
}

/// Options for rendering HTML to an image.
///
/// `width`, `height`, `scale` and `quality` can also be set by the page with
/// `<meta name="dynimg:...">` tags. Options set here take precedence over
/// meta tags, which take precedence over the defaults.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// Viewport width in CSS pixels (default: [`DEFAULT_WIDTH`])
    pub width: Option<u32>,

    /// Viewport height in CSS pixels. If None, auto-sizes to content height.
    pub height: Option<u32>,

    /// Scale factor for output resolution (default: [`DEFAULT_SCALE`], for
    /// retina displays). Output dimensions = viewport × scale
    pub scale: Option<f32>,

    /// JPEG quality 1-100 (default: [`DEFAULT_QUALITY`]). The resolved value
    /// is stored on [`RenderedImage::quality`] and used by
    /// [`RenderedImage::encode`].
    pub quality: Option<u8>,

    /// Allow network requests for loading remote resources (images, fonts, etc.)
    pub allow_net: bool,

    /// Directory for loading local assets. Paths are sandboxed to this directory.
    pub assets_dir: Option<PathBuf>,

    /// Base URL for resolving relative paths. If None, uses assets_dir or current directory.
    pub base_url: Option<String>,

    /// Background color as any CSS color, e.g. "#ffffff", "white" or
    /// "rgb(0 0 0 / 50%)" (default: transparent). Invalid colors are transparent.
    /// Automatically set to white for JPEG output via CLI and `render_to_file()`.
    pub background: Option<String>,

    /// Reserved for future diagnostic output; currently has no effect on
    /// rendering. (The CLI's `--verbose` flag enables its own logging.)
    pub verbose: bool,

    /// Custom font files (TTF/OTF/WOFF/WOFF2) to register for this render.
    /// Family names are read from the font files themselves, so a registered
    /// font named "Inter" matches `font-family: "Inter"` in CSS. Registered
    /// fonts take priority over system fonts with the same family name, and
    /// back the generic families (sans-serif, ...) on hosts with no
    /// discoverable system fonts.
    ///
    /// Files are read on each render; loading and registration are cheap
    /// (~1ms even for large fonts), so reusing options across renders is not
    /// required. Missing or invalid font files fail the render with
    /// [`Error::FontLoad`].
    pub fonts: Vec<PathBuf>,

    /// Font files registered under a CSS name: (name, path) pairs.
    ///
    /// If the name is a CSS generic — "serif", "sans-serif", "monospace",
    /// "cursive", "fantasy", "system-ui", "ui-serif", "ui-sans-serif",
    /// "ui-monospace", "ui-rounded", "emoji", "math", "fangsong" — the font
    /// maps that generic with priority over the platform mapping, so e.g.
    /// `font-family: sans-serif` resolves to it on any host. `"emoji"` takes
    /// priority over the platform emoji font (Apple Color Emoji, Noto, ...)
    /// for the emoji generic and inside text styled with other mapped
    /// generics; to fully pin emoji rendering, also map the text generics
    /// your pages use (on some hosts, unmapped generics expand to platform
    /// font lists that can include an emoji font). Platform fonts are used
    /// only as a last resort for glyphs the mapped fonts don't cover.
    ///
    /// Any other name registers the font under that family name instead of
    /// the name inside the font file, matching `font-family: "name"` in CSS.
    pub named_fonts: Vec<(String, PathBuf)>,
}

impl RenderOptions {
    /// Create options with a specific viewport size
    pub fn with_size(width: u32, height: u32) -> Self {
        Self::default().width(width).height(height)
    }

    /// Set the viewport width
    pub fn width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    /// Set the viewport height
    pub fn height(mut self, height: u32) -> Self {
        self.height = Some(height);
        self
    }

    /// Set the scale factor
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = Some(scale);
        self
    }

    /// Set the JPEG quality (1-100)
    pub fn quality(mut self, quality: u8) -> Self {
        self.quality = Some(quality);
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

    /// Set the background color as any CSS color, e.g. "#ffffff" or "white" (default: transparent)
    pub fn background(mut self, color: impl Into<String>) -> Self {
        self.background = Some(color.into());
        self
    }

    /// Set the reserved verbose flag (currently has no effect on rendering)
    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }

    /// Register a custom font file (TTF/OTF/WOFF/WOFF2). The file is read at
    /// render time; a missing or invalid file fails the render.
    pub fn font_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.fonts.push(path.into());
        self
    }

    /// Register a font file (TTF/OTF/WOFF/WOFF2) under a CSS name. Generic
    /// names ("sans-serif", "emoji", ...) map that generic to the font; any
    /// other name registers the font under that family name.
    pub fn named_font_file(mut self, name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        self.named_fonts.push((name.into(), path.into()));
        self
    }

    /// Adjust options for an output format: JPEG has no transparency, so it
    /// gets a white background unless one is already set.
    pub fn for_format(mut self, format: ImageFormat) -> Self {
        if format == ImageFormat::Jpeg && self.background.is_none() {
            self.background = Some("#ffffff".to_string());
        }
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

    /// JPEG quality resolved from the render options, the page's
    /// `dynimg:quality` meta tag, or [`DEFAULT_QUALITY`]
    pub quality: u8,
}

impl RenderedImage {
    /// Encode the image in the given format, using [`Self::quality`] for JPEG
    pub fn encode(&self, format: ImageFormat) -> Result<Vec<u8>, Error> {
        match format {
            ImageFormat::Png => self.to_png(),
            ImageFormat::Jpeg => self.to_jpeg(self.quality),
            ImageFormat::WebP => Ok(self.to_webp()),
        }
    }

    /// Save the image, detecting the format from the file extension
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        let path = path.as_ref();
        fs::write(path, self.encode(ImageFormat::from_path(path)?)?)?;
        Ok(())
    }

    /// Save the image as PNG
    pub fn save_png(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        Ok(fs::write(path, self.to_png()?)?)
    }

    /// Save the image as JPEG with the specified quality (1-100)
    pub fn save_jpeg(&self, path: impl AsRef<Path>, quality: u8) -> Result<(), Error> {
        Ok(fs::write(path, self.to_jpeg(quality)?)?)
    }

    /// Save the image as lossless WebP
    pub fn save_webp(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        Ok(fs::write(path, self.to_webp())?)
    }

    /// Encode the image as PNG bytes
    pub fn to_png(&self) -> Result<Vec<u8>, Error> {
        // 144 DPI, matching the default 2x scale
        const PPM: u32 = (144.0 * 39.3701) as u32;

        // Pre-allocate output (PNG is typically 10-50% of raw size after compression)
        let mut output = Vec::with_capacity(self.data.len() / 4);
        let mut encoder = png::Encoder::new(&mut output, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        encoder.set_pixel_dims(Some(png::PixelDimensions {
            xppu: PPM,
            yppu: PPM,
            unit: png::Unit::Meter,
        }));
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&self.data)?;
        writer.finish()?;
        Ok(output)
    }

    /// Encode the image as JPEG bytes with the specified quality (1-100)
    pub fn to_jpeg(&self, quality: u8) -> Result<Vec<u8>, Error> {
        use zenjpeg::encoder::{ChromaSubsampling, EncoderConfig, PixelLayout, Unstoppable};

        // 4:2:0 chroma subsampling for good compression; alpha is ignored
        let config = EncoderConfig::ycbcr(quality, ChromaSubsampling::Quarter);
        let mut encoder =
            config.encode_from_bytes(self.width, self.height, PixelLayout::Rgba8Srgb)?;
        encoder.push_packed(&self.data, Unstoppable)?;
        Ok(encoder.finish()?)
    }

    /// Encode the image as lossless WebP bytes
    pub fn to_webp(&self) -> Vec<u8> {
        let encoder = webp::Encoder::from_rgba(&self.data, self.width, self.height);
        let mut config = webp::WebPConfig::new().unwrap();
        config.lossless = 1;
        config.quality = 75.0;
        config.method = 0; // 0=fastest, 6=slowest (default)
        encoder.encode_advanced(&config).unwrap().to_vec()
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
    let RenderOptions {
        width,
        height,
        scale,
        quality,
        allow_net,
        assets_dir,
        base_url,
        background,
        verbose: _,
        fonts,
        named_fonts,
    } = options;

    // Build base URL for asset resolution
    let base_url = base_url.or_else(|| {
        assets_dir
            .as_ref()
            .and_then(|p| p.canonicalize().ok())
            .or_else(|| std::env::current_dir().ok())
            .map(|p| format!("file://{}/", p.display()))
    });

    // Create provider for assets and/or network.
    // Always created so unresolvable requests get empty responses,
    // preventing Blitz's FOUC prevention from blocking rendering.
    let provider = Arc::new(CombinedProvider::new(assets_dir, allow_net));

    // Register custom fonts alongside system fonts
    let font_ctx = if fonts.is_empty() && named_fonts.is_empty() {
        None
    } else {
        Some(build_font_context(&fonts, &named_fonts, true)?)
    };

    // Parse document
    let mut document = HtmlDocument::from_html(
        html,
        DocumentConfig {
            base_url,
            net_provider: Some(provider.clone() as _),
            viewport: None,
            font_ctx,
            // Sequential bypasses Stylo's global rayon pool, which panics with
            // "already mutably borrowed" when two documents resolve in parallel.
            // This makes concurrent render() calls safe without a global lock.
            style_threading: StyleThreading::Sequential,
            ..Default::default()
        },
    );

    // Explicit options win over meta tags, which win over defaults
    let meta = extract_meta_options(document.as_ref());
    let width = width.or(meta.width).unwrap_or(DEFAULT_WIDTH);
    let height = height.or(meta.height);
    let scale = scale.or(meta.scale).unwrap_or(DEFAULT_SCALE);
    let quality = quality.or(meta.quality).unwrap_or(DEFAULT_QUALITY);

    // Auto height lays out against a provisional 800px viewport, then
    // renders at the document's content height
    document.set_viewport(Viewport::new(
        scaled(width, scale),
        scaled(height.unwrap_or(800), scale),
        scale,
        ColorScheme::Light,
    ));

    let background = background
        .as_deref()
        .map(parse_color)
        .unwrap_or(Color::TRANSPARENT);

    let (data, width, height) =
        render_document(&mut document, &provider, width, height, scale, background).await;

    Ok(RenderedImage {
        data,
        width,
        height,
        quality,
    })
}

/// Render HTML and save directly to a file.
/// The output format is detected from the file extension; JPEG gets a white
/// background unless `options.background` is set.
///
/// # Example
///
/// ```no_run
/// use dynimg::{render_to_file, RenderOptions};
///
/// # #[tokio::main]
/// # async fn main() -> Result<(), dynimg::Error> {
/// let html = "<html><body><h1>Hello</h1></body></html>";
/// render_to_file(html, "output.png", RenderOptions::default()).await?;
/// # Ok(())
/// # }
/// ```
pub async fn render_to_file(
    html: &str,
    path: impl AsRef<Path>,
    options: RenderOptions,
) -> Result<(), Error> {
    let path = path.as_ref();
    let format = ImageFormat::from_path(path)?;
    let image = render(html, options.for_format(format)).await?;
    fs::write(path, image.encode(format)?)?;
    Ok(())
}

// ============================================================================
// Internal implementation
// ============================================================================

/// Parse a CSS color ("#fff", "white", "rgb(0 0 0 / 50%)", ...) to a Color.
/// Falls back to transparent if parsing fails.
fn parse_color(s: &str) -> Color {
    peniko::color::parse_color(s.trim())
        .map(|c| c.to_alpha_color())
        .unwrap_or(Color::TRANSPARENT)
}

/// Build a font context with the given custom fonts registered on top of
/// system fonts.
///
/// Named-font handling:
/// - A CSS generic name ("sans-serif", "emoji", ...) maps that generic to
///   the font with priority over the platform mapping, which is what parley
///   queries when resolving the generic keyword (and, for `emoji`, what it
///   queries for emoji clusters). Platform fonts remain only as a last
///   resort for glyphs the mapped fonts don't cover. Note that a family
///   merely NAMED "serif" would never match `font-family: serif` — CSS
///   treats unquoted generics as keywords, resolved through the generic
///   map, not the family-name table.
/// - Any other name registers the font under that family name (overriding
///   the name inside the font file), matching `font-family: "name"`.
///
/// Additionally, if a generic family still has no mapping — e.g. minimal
/// Linux containers without fontconfig, where system font discovery
/// gracefully finds nothing — the unnamed custom fonts are appended as its
/// fallback so unstyled text still renders.
fn build_font_context(
    fonts: &[PathBuf],
    named_fonts: &[(String, PathBuf)],
    use_system_fonts: bool,
) -> Result<FontContext, Error> {
    use fontique::{Collection, CollectionOptions, FontInfoOverride, GenericFamily};

    // `use_system_fonts` is always true in production; tests pass false to
    // simulate a host with no discoverable fonts.
    let mut font_ctx = FontContext {
        collection: Collection::new(CollectionOptions {
            system_fonts: use_system_fonts,
            ..Default::default()
        }),
        source_cache: Default::default(),
    };

    // Read a font file and register it, erroring on unreadable or invalid
    // fonts (fontique silently registers nothing for unparseable data)
    let register = |collection: &mut fontique::Collection,
                    path: &PathBuf,
                    family_name: Option<&str>|
     -> Result<Vec<fontique::FamilyId>, Error> {
        let bytes = fs::read(path).map_err(|e| Error::FontLoad(path.clone(), e.to_string()))?;
        // Decompress WOFF/WOFF2; TTF/OTF passes through borrowed, so reuse
        // the read buffer instead of copying
        let data = match decode_font_bytes(&bytes) {
            std::borrow::Cow::Owned(decompressed) => decompressed,
            std::borrow::Cow::Borrowed(_) => bytes,
        };
        let ids: Vec<_> = collection
            .register_fonts(
                Blob::new(Arc::new(data)),
                family_name.map(|name| FontInfoOverride {
                    family_name: Some(name),
                    ..Default::default()
                }),
            )
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        if ids.is_empty() {
            return Err(Error::FontLoad(
                path.clone(),
                "no font families found (not a valid font file?)".to_string(),
            ));
        }
        Ok(ids)
    };

    let mut registered = Vec::new();
    for path in fonts {
        registered.extend(register(&mut font_ctx.collection, path, None)?);
    }

    // Group generic-named fonts by generic, preserving listed order
    let mut by_generic: Vec<(GenericFamily, Vec<_>)> = Vec::new();
    for (name, path) in named_fonts {
        if let Some(generic) = GenericFamily::parse(name) {
            let ids = register(&mut font_ctx.collection, path, None)?;
            match by_generic.iter_mut().find(|(g, _)| *g == generic) {
                Some((_, group)) => group.extend(ids),
                None => by_generic.push((generic, ids)),
            }
        } else {
            register(&mut font_ctx.collection, path, Some(name))?;
        }
    }
    // Mapped fonts take priority for their generic (fontique appends the
    // platform's own list after ours as a last resort for uncovered glyphs).
    // An "emoji" mapping additionally backs every OTHER mapped generic:
    // platform generic lists can contain emoji-capable fonts (fontconfig
    // expands e.g. system-ui to a preference list that includes Noto Color
    // Emoji), and emoji clusters coverage-scan the styled family list before
    // the emoji generic — so the pinned emoji font must sit in front of the
    // platform entries there too, or emoji in mapped text become
    // host-dependent.
    let emoji_ids: Vec<_> = by_generic
        .iter()
        .find(|(g, _)| *g == GenericFamily::Emoji)
        .map(|(_, ids)| ids.clone())
        .unwrap_or_default();
    for (generic, ids) in by_generic {
        let backing = if generic == GenericFamily::Emoji {
            Vec::new()
        } else {
            emoji_ids.clone()
        };
        font_ctx
            .collection
            .set_generic_families(generic, ids.into_iter().chain(backing));
    }

    // Emoji excluded: an emoji font must never back text generics (emoji
    // fonts cover digits/space for keycap sequences and would hijack them)
    for &generic in GenericFamily::all() {
        if generic == GenericFamily::Emoji {
            continue;
        }
        if font_ctx
            .collection
            .generic_families(generic)
            .next()
            .is_none()
        {
            font_ctx
                .collection
                .append_generic_families(generic, registered.iter().copied());
        }
    }

    Ok(font_ctx)
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
    let meta_ids = doc
        .query_selector_all("meta[name][content]")
        .expect("static selector is valid");

    for node in meta_ids.into_iter().filter_map(|id| doc.get_node(id)) {
        let (Some(name), Some(content)) = (
            node.attr(local_name!("name")),
            node.attr(local_name!("content")),
        ) else {
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

/// A NetProvider that serves `file://` URLs from a sandboxed assets directory
struct AssetProvider {
    /// Canonical assets directory, or None if it doesn't exist (every request
    /// then misses)
    assets_dir: Option<PathBuf>,
}

impl AssetProvider {
    fn new(assets_dir: PathBuf) -> Self {
        Self {
            assets_dir: assets_dir.canonicalize().ok(),
        }
    }

    /// The canonical file path for a `file://` URL, if it exists inside the
    /// assets directory (symlinks and `..` can't escape the sandbox)
    fn resolve_path(&self, url: &Url) -> Option<PathBuf> {
        let assets_dir = self.assets_dir.as_ref()?;
        let path = url.to_file_path().ok()?.canonicalize().ok()?;
        path.starts_with(assets_dir).then_some(path)
    }
}

impl NetProvider for AssetProvider {
    fn fetch(&self, _doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        // Missing or out-of-sandbox files get empty bytes (see CombinedProvider)
        let data = self
            .resolve_path(&request.url)
            .and_then(|path| fs::read(path).ok())
            .map(Bytes::from)
            .unwrap_or_default();
        handler.bytes(request.url.to_string(), data);
    }
}

/// Combined provider for data URIs, assets, and network requests.
///
/// Every request gets a response, with empty bytes when the resource can't be
/// loaded. Blitz's `NetHandler` has no failure callback, and it blocks style,
/// layout and paint until render-blocking resources (e.g. `<head>`
/// stylesheets) respond, so a dropped request would render nothing.
struct CombinedProvider {
    assets: Option<AssetProvider>,
    network: Option<HttpProvider>,
}

impl CombinedProvider {
    fn new(assets_dir: Option<PathBuf>, allow_net: bool) -> Self {
        Self {
            assets: assets_dir.map(AssetProvider::new),
            network: allow_net.then(HttpProvider::new),
        }
    }

    fn is_empty(&self) -> bool {
        self.network.as_ref().is_none_or(HttpProvider::is_empty)
    }
}

impl NetProvider for CombinedProvider {
    fn fetch(&self, doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let scheme = request.url.scheme();

        // Inline data: URIs are self-contained; decode them regardless of
        // asset/network settings.
        if scheme == "data" {
            let decoded = DataUrl::process(request.url.as_str())
                .ok()
                .and_then(|data_url| data_url.decode_to_vec().ok())
                .map(|(body, _fragment)| Bytes::from(body))
                .unwrap_or_default();
            handler.bytes(request.url.to_string(), decoded);
            return;
        }

        // Non-HTTP(S) URLs (e.g. file://) are local resources that need the assets provider
        let provider: Option<&dyn NetProvider> = if scheme == "http" || scheme == "https" {
            self.network.as_ref().map(|n| n as _)
        } else {
            self.assets.as_ref().map(|a| a as _)
        };

        match provider {
            Some(provider) => provider.fetch(doc_id, request, handler),
            None => handler.bytes(request.url.to_string(), Bytes::new()),
        }
    }
}

/// Convert CSS pixels to output pixels
fn scaled(css_px: u32, scale: f32) -> u32 {
    (css_px as f64 * scale as f64) as u32
}

/// Load resources, lay out and paint the document. Returns RGBA pixel data
/// with its width and height in output pixels.
async fn render_document(
    document: &mut HtmlDocument,
    provider: &CombinedProvider,
    width: u32,
    height: Option<u32>,
    scale: f32,
    background: Color,
) -> (Vec<u8>, u32, u32) {
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
    let height = height.unwrap_or_else(|| {
        let content_height = document.as_ref().root_element().final_layout().size.height;
        content_height.ceil() as u32
    });
    let render_width = scaled(width, scale);
    let render_height = scaled(height, scale);

    // Render to RGBA buffer
    let buffer = render_to_buffer::<VelloCpuImageRenderer, _>(
        |scene| {
            scene.fill(
                Fill::NonZero,
                Default::default(),
                background,
                Default::default(),
                &Rect::new(0.0, 0.0, render_width as f64, render_height as f64),
            );

            paint_scene(
                scene,
                document.as_mut(),
                scale as f64,
                render_width,
                render_height,
                0,
                0,
            );
        },
        render_width,
        render_height,
    );

    (buffer, render_width, render_height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fontique::GenericFamily;

    fn test_font() -> PathBuf {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/assets/fonts/Silkscreen-Regular.ttf"
        ))
    }

    #[test]
    fn invalid_font_files_error() {
        let not_a_font = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"));
        let Err(err) = build_font_context(&[not_a_font], &[], false) else {
            panic!("expected error for non-font file");
        };
        assert!(matches!(err, Error::FontLoad(..)), "got: {err:?}");

        let missing = PathBuf::from("/nonexistent/font.ttf");
        let Err(err) = build_font_context(&[missing], &[], false) else {
            panic!("expected error for missing file");
        };
        assert!(matches!(err, Error::FontLoad(..)), "got: {err:?}");
    }

    #[test]
    fn custom_fonts_fill_empty_generic_families() {
        // Simulates a minimal container: no discoverable system fonts
        let mut ctx = build_font_context(&[test_font()], &[], false).unwrap();
        assert!(ctx.collection.family_by_name("Silkscreen").is_some());
        assert!(
            ctx.collection
                .generic_families(GenericFamily::SansSerif)
                .next()
                .is_some(),
            "custom fonts should back sans-serif when no system fonts exist"
        );
    }

    #[test]
    fn custom_fonts_do_not_override_system_generic_families() {
        let mut ctx = build_font_context(&[test_font()], &[], true).unwrap();
        let custom: Vec<_> = ctx
            .collection
            .family_by_name("Silkscreen")
            .map(|f| f.id())
            .into_iter()
            .collect();
        let sans: Vec<_> = ctx
            .collection
            .generic_families(GenericFamily::SansSerif)
            .collect();
        assert!(
            !sans.iter().any(|id| custom.contains(id)),
            "system-provided generic families should be left alone"
        );
    }

    fn emoji_font() -> PathBuf {
        PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/assets/fonts/TwemojiCOLRv0.ttf"
        ))
    }

    #[test]
    fn emoji_font_wins_over_system_emoji() {
        let mut ctx =
            build_font_context(&[test_font()], &[("emoji".to_string(), emoji_font())], true)
                .unwrap();
        let twemoji = ctx
            .collection
            .family_by_name("Twemoji COLRv0")
            .expect("emoji font registered")
            .id();
        let first = ctx
            .collection
            .generic_families(fontique::GenericFamily::Emoji)
            .next();
        assert_eq!(
            first,
            Some(twemoji),
            "emoji_font should be the first emoji fallback"
        );
        // Regular registered fonts must not pollute the emoji list
        let silkscreen = ctx.collection.family_by_name("Silkscreen").unwrap().id();
        assert!(
            !ctx.collection
                .generic_families(fontique::GenericFamily::Emoji)
                .any(|id| id == silkscreen)
        );
    }

    #[test]
    fn emoji_font_works_without_system_fonts() {
        let mut ctx =
            build_font_context(&[], &[("emoji".to_string(), emoji_font())], false).unwrap();
        assert!(
            ctx.collection
                .generic_families(fontique::GenericFamily::Emoji)
                .next()
                .is_some(),
            "emoji generic should be backed by the emoji font"
        );
    }

    #[test]
    fn emoji_font_does_not_back_text_generics() {
        let mut ctx =
            build_font_context(&[], &[("emoji".to_string(), emoji_font())], false).unwrap();
        assert!(
            ctx.collection
                .generic_families(fontique::GenericFamily::SansSerif)
                .next()
                .is_none(),
            "emoji font should not be used for sans-serif text"
        );
    }

    #[test]
    fn generic_font_wins_over_system_mapping() {
        let named = [("sans-serif".to_string(), test_font())];
        let mut ctx = build_font_context(&[], &named, true).unwrap();
        let silkscreen = ctx.collection.family_by_name("Silkscreen").unwrap().id();
        let first = ctx
            .collection
            .generic_families(fontique::GenericFamily::SansSerif)
            .next();
        assert_eq!(
            first,
            Some(silkscreen),
            "mapped font should be the first sans-serif candidate"
        );
    }

    #[test]
    fn emoji_mapping_backs_other_mapped_generics() {
        // Platform generic lists can contain emoji-capable fonts (fontconfig
        // includes Noto Color Emoji in e.g. system-ui expansions), and emoji
        // clusters scan the styled family list before the emoji generic. The
        // emoji mapping must therefore back every mapped generic, ahead of
        // any platform entries.
        let named = [
            ("sans-serif".to_string(), test_font()),
            ("emoji".to_string(), emoji_font()),
        ];
        let mut ctx = build_font_context(&[], &named, false).unwrap();
        let silkscreen = ctx.collection.family_by_name("Silkscreen").unwrap().id();
        let twemoji = ctx
            .collection
            .family_by_name("Twemoji COLRv0")
            .unwrap()
            .id();
        let sans: Vec<_> = ctx
            .collection
            .generic_families(fontique::GenericFamily::SansSerif)
            .collect();
        assert_eq!(
            sans,
            vec![silkscreen, twemoji],
            "mapped generic should be [its font, emoji font]"
        );
        let emoji: Vec<_> = ctx
            .collection
            .generic_families(fontique::GenericFamily::Emoji)
            .collect();
        assert_eq!(emoji, vec![twemoji]);
    }

    #[test]
    fn generic_font_works_without_system_fonts() {
        let named = [("serif".to_string(), test_font())];
        let mut ctx = build_font_context(&[], &named, false).unwrap();
        assert!(
            ctx.collection
                .generic_families(fontique::GenericFamily::Serif)
                .next()
                .is_some()
        );
        // Unmapped generics stay empty (no `fonts` were registered)
        assert!(
            ctx.collection
                .generic_families(fontique::GenericFamily::Monospace)
                .next()
                .is_none()
        );
    }

    #[test]
    fn custom_name_overrides_family_name() {
        let named = [("myfont".to_string(), test_font())];
        let mut ctx = build_font_context(&[], &named, false).unwrap();
        assert!(
            ctx.collection.family_by_name("myfont").is_some(),
            "font should be registered under the custom name"
        );
        assert!(
            ctx.collection.family_by_name("Silkscreen").is_none(),
            "the name inside the font file should be replaced, not added"
        );
    }

    const RED_BAR: &str = r#"<html><body style="margin:0"><div style="width:100%;height:20px;background:#f00"></div></body></html>"#;

    fn red_px_in_first_row(image: &RenderedImage) -> usize {
        image.data[..image.width as usize * 4]
            .chunks(4)
            .filter(|px| px == &[255, 0, 0, 255])
            .count()
    }

    #[tokio::test]
    async fn fractional_scale_lays_out_full_width() {
        for scale in [0.5, 1.5, 2.5] {
            let image = render(RED_BAR, RenderOptions::default().width(300).scale(scale))
                .await
                .unwrap();
            let expected = (300.0 * scale) as u32;
            assert_eq!(image.width, expected, "scale {scale}");
            assert_eq!(
                red_px_in_first_row(&image),
                expected as usize,
                "a width:100% bar should span the image at scale {scale}"
            );
        }
    }

    fn with_meta(meta: &str) -> String {
        format!("<html><head>{meta}</head><body>hi</body></html>")
    }

    #[tokio::test]
    async fn meta_tags_apply_when_options_unset() {
        let html = with_meta(
            r#"<meta name="dynimg:width" content="400">
               <meta name="dynimg:height" content="50">
               <meta name="dynimg:scale" content="1">
               <meta name="dynimg:quality" content="42">"#,
        );
        let image = render(&html, RenderOptions::default()).await.unwrap();
        assert_eq!((image.width, image.height, image.quality), (400, 50, 42));
    }

    #[tokio::test]
    async fn explicit_options_override_meta_tags() {
        let html = with_meta(
            r#"<meta name="dynimg:width" content="400">
               <meta name="dynimg:height" content="50">
               <meta name="dynimg:scale" content="1">
               <meta name="dynimg:quality" content="42">"#,
        );
        let options = RenderOptions::default()
            .width(200)
            .height(30)
            .scale(2.0)
            .quality(80);
        let image = render(&html, options).await.unwrap();
        assert_eq!((image.width, image.height, image.quality), (400, 60, 80));
    }

    #[tokio::test]
    async fn defaults_apply_without_options_or_meta() {
        let image = render(&with_meta(""), RenderOptions::default().height(10))
            .await
            .unwrap();
        assert_eq!((image.width, image.height, image.quality), (2400, 20, 90));
    }

    #[test]
    fn image_format_from_path() {
        let fmt = |p: &str| ImageFormat::from_path(Path::new(p)).ok();
        assert_eq!(fmt("a.png"), Some(ImageFormat::Png));
        assert_eq!(fmt("a.JPG"), Some(ImageFormat::Jpeg));
        assert_eq!(fmt("a.jpeg"), Some(ImageFormat::Jpeg));
        assert_eq!(fmt("a.webp"), Some(ImageFormat::WebP));
        assert_eq!(fmt("a.gif"), None);
        assert_eq!(fmt("noext"), None);
    }

    #[test]
    fn jpeg_defaults_to_white_background() {
        let jpeg = RenderOptions::default().for_format(ImageFormat::Jpeg);
        assert_eq!(jpeg.background.as_deref(), Some("#ffffff"));
        let custom = RenderOptions::default()
            .background("#000")
            .for_format(ImageFormat::Jpeg);
        assert_eq!(custom.background.as_deref(), Some("#000"));
        let png = RenderOptions::default().for_format(ImageFormat::Png);
        assert_eq!(png.background, None);
    }

    #[tokio::test]
    async fn encode_produces_each_format() {
        let image = render(RED_BAR, RenderOptions::default().width(10).scale(1.0))
            .await
            .unwrap();
        let png = image.encode(ImageFormat::Png).unwrap();
        let jpeg = image.encode(ImageFormat::Jpeg).unwrap();
        let webp = image.encode(ImageFormat::WebP).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        assert!(jpeg.starts_with(&[0xFF, 0xD8]));
        assert_eq!(&webp[8..12], b"WEBP");
    }

    #[test]
    fn background_accepts_css_colors() {
        let rgba = |s| parse_color(s).to_rgba8().to_u8_array();
        assert_eq!(rgba("#fff"), [255, 255, 255, 255]);
        assert_eq!(rgba("#ff000080"), [255, 0, 0, 128]);
        assert_eq!(rgba("white"), [255, 255, 255, 255]);
        assert_eq!(rgba("rgb(0, 128, 255)"), [0, 128, 255, 255]);
        assert_eq!(rgba("not a color"), [0, 0, 0, 0]);
    }

    /// Renders a 200x100 red box behind a render-blocking stylesheet that
    /// can never load, and checks the box still paints at full size.
    async fn render_with_unloadable_stylesheet(href: &str, options: RenderOptions) {
        let html = format!(
            r#"<html><head><link rel="stylesheet" href="{href}"></head>
            <body style="margin:0"><div style="width:200px;height:100px;background:#f00"></div></body></html>"#
        );
        let image = render(&html, options.width(200).scale(1.0))
            .await
            .unwrap_or_else(|e| panic!("render with stylesheet {href} failed: {e}"));
        assert_eq!((image.width, image.height), (200, 100), "href: {href}");
        assert_eq!(&image.data[..4], &[255, 0, 0, 255], "href: {href}");
    }

    #[tokio::test]
    async fn remote_stylesheet_without_network_does_not_block_render() {
        render_with_unloadable_stylesheet(
            "https://example.invalid/x.css",
            RenderOptions::default(),
        )
        .await;
    }

    #[tokio::test]
    async fn missing_asset_stylesheet_does_not_block_render() {
        let assets = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/assets"));
        render_with_unloadable_stylesheet(
            "missing.css",
            RenderOptions::default().assets_dir(assets),
        )
        .await;
    }

    #[tokio::test]
    async fn failed_fetch_stylesheet_does_not_block_render() {
        // Port 9 (discard) on loopback refuses the connection immediately
        render_with_unloadable_stylesheet(
            "http://127.0.0.1:9/x.css",
            RenderOptions::default().allow_net(),
        )
        .await;
    }
}
