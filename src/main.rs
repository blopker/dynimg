use anyhow::{Context, Result, bail};
use anyrender::{PaintScene as _, render_to_buffer};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::{DocumentConfig, util::Color};
use blitz_html::HtmlDocument;
use blitz_net::Provider;
use blitz_paint::paint_scene;
use blitz_traits::net::{NetHandler, NetProvider, Request};
use blitz_traits::shell::{ColorScheme, Viewport};
use bytes::Bytes;
use clap::Parser;
use kurbo::Rect;
use peniko::Fill;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A NetProvider that serves files from a sandboxed assets directory
struct AssetProvider {
    assets_dir: PathBuf,
}

impl AssetProvider {
    fn new(assets_dir: PathBuf) -> Self {
        Self { assets_dir }
    }

    /// Validate and resolve a file path, ensuring it stays within the assets directory
    fn resolve_path(&self, url: &str) -> Option<PathBuf> {
        // Only handle file:// URLs or relative paths
        let path_str = if let Some(stripped) = url.strip_prefix("file://") {
            stripped
        } else if url.starts_with("http://") || url.starts_with("https://") {
            return None; // Network URLs not handled
        } else {
            url // Treat as relative path
        };

        // Build the full path
        let requested_path = Path::new(path_str);
        let full_path = if requested_path.is_absolute() {
            requested_path.to_path_buf()
        } else {
            self.assets_dir.join(requested_path)
        };

        // Canonicalize to resolve any .. or symlinks
        let canonical = full_path.canonicalize().ok()?;
        let assets_canonical = self.assets_dir.canonicalize().ok()?;

        // Ensure the resolved path is within the assets directory
        if canonical.starts_with(&assets_canonical) {
            Some(canonical)
        } else {
            eprintln!(
                "Warning: Blocked access to {} (outside assets directory)",
                url
            );
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

/// A combined provider that handles both assets and network requests
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
        let url = request.url.to_string();

        // Check if it's a file URL or relative path that assets can handle
        if !url.starts_with("http://")
            && !url.starts_with("https://")
            && let Some(ref assets) = self.assets
        {
            assets.fetch(doc_id, request, handler);
            return;
        }

        // Otherwise try network
        if let Some(ref network) = self.network {
            network.fetch(doc_id, request, handler);
        }
    }
}

/// A fast CLI tool for generating high-quality images from HTML/CSS
#[derive(Parser, Debug)]
#[command(name = "dynimg", version, about)]
struct Args {
    /// HTML file path or '-' for stdin
    input: String,

    /// Output image path (format detected from extension)
    #[arg(short, long)]
    output: PathBuf,

    /// Image width in pixels
    #[arg(short, long, default_value = "1200")]
    width: u32,

    /// Image height in pixels (defaults to document height)
    #[arg(short = 'H', long)]
    height: Option<u32>,

    /// Scale factor for high-DPI rendering
    #[arg(short, long, default_value = "2")]
    scale: f32,

    /// JPEG/WebP quality (1-100)
    #[arg(short, long, default_value = "90")]
    quality: u8,

    /// Allow network access for loading remote resources
    #[arg(long)]
    allow_net: bool,

    /// Asset directory for local resources (enables filesystem access)
    #[arg(long)]
    assets: Option<PathBuf>,
}

/// Options that can be set via meta tags or CLI
#[derive(Debug, Default)]
struct RenderOptions {
    width: Option<u32>,
    height: Option<u32>,
    scale: Option<f32>,
    quality: Option<u8>,
}

/// Output format detected from file extension
#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Png,
    Jpeg,
    WebP,
}

impl OutputFormat {
    fn from_path(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match ext.as_deref() {
            Some("png") => Ok(OutputFormat::Png),
            Some("jpg") | Some("jpeg") => Ok(OutputFormat::Jpeg),
            Some("webp") => Ok(OutputFormat::WebP),
            Some(ext) => bail!("Unsupported output format: .{}", ext),
            None => bail!("Output file must have an extension (.png, .jpg, .webp)"),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Detect output format from extension
    let format = OutputFormat::from_path(&args.output)?;

    // Read HTML input
    let html = if args.input == "-" {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        buffer
    } else {
        fs::read_to_string(&args.input)
            .with_context(|| format!("Failed to read file: {}", args.input))?
    };

    // Parse meta tags for options
    let meta_options = parse_meta_tags(&html);

    // Merge options: CLI args take precedence over meta tags
    let width = args.width; // CLI always provides a default
    let height = args.height.or(meta_options.height);
    let scale = args.scale; // CLI always provides a default
    let quality = args.quality; // CLI always provides a default

    // Override with meta tags only if CLI used defaults
    let width = if args.width == 1200 {
        meta_options.width.unwrap_or(width)
    } else {
        width
    };
    let scale = if (args.scale - 2.0).abs() < 0.001 {
        meta_options.scale.unwrap_or(scale)
    } else {
        scale
    };
    let quality = if args.quality == 90 {
        meta_options.quality.unwrap_or(quality)
    } else {
        quality
    };

    // Render the document
    let buffer = render_html(
        &html,
        width,
        height,
        scale,
        args.allow_net,
        args.assets.as_deref(),
    )?;

    // Encode and write output
    let render_width = buffer.width;
    let render_height = buffer.height;

    match format {
        OutputFormat::Png => {
            write_png(&args.output, &buffer.data, render_width, render_height)?;
        }
        OutputFormat::Jpeg => {
            write_jpeg(
                &args.output,
                &buffer.data,
                render_width,
                render_height,
                quality,
            )?;
        }
        OutputFormat::WebP => {
            write_webp(
                &args.output,
                &buffer.data,
                render_width,
                render_height,
                quality,
            )?;
        }
    }

    eprintln!(
        "Wrote {}x{} image to {}",
        render_width,
        render_height,
        args.output.display()
    );

    Ok(())
}

/// Parse dynimg meta tags from HTML
fn parse_meta_tags(html: &str) -> RenderOptions {
    let mut options = RenderOptions::default();

    // Simple regex-free parsing for meta tags
    for line in html.lines() {
        if let Some(content) = extract_meta_content(line, "dynimg:width") {
            options.width = content.parse().ok();
        }
        if let Some(content) = extract_meta_content(line, "dynimg:height") {
            options.height = content.parse().ok();
        }
        if let Some(content) = extract_meta_content(line, "dynimg:scale") {
            options.scale = content.parse().ok();
        }
        if let Some(content) = extract_meta_content(line, "dynimg:quality") {
            options.quality = content.parse().ok();
        }
    }

    options
}

/// Extract content from a meta tag like <meta name="dynimg:width" content="1200">
fn extract_meta_content(line: &str, name: &str) -> Option<String> {
    let line_lower = line.to_lowercase();
    if !line_lower.contains("<meta") || !line_lower.contains(name) {
        return None;
    }

    // Find content attribute value
    let content_start = line_lower.find("content=")?;
    let after_content = &line[content_start + 8..];

    // Handle both single and double quotes
    let quote_char = after_content.chars().next()?;
    if quote_char != '"' && quote_char != '\'' {
        return None;
    }

    let value_start = 1;
    let value_end = after_content[value_start..].find(quote_char)?;
    Some(after_content[value_start..value_start + value_end].to_string())
}

struct RenderBuffer {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

fn render_html(
    html: &str,
    width: u32,
    height: Option<u32>,
    scale: f32,
    allow_net: bool,
    assets_dir: Option<&Path>,
) -> Result<RenderBuffer> {
    // Create combined provider for assets and/or network
    let has_provider = allow_net || assets_dir.is_some();
    let provider = if has_provider {
        Some(Arc::new(CombinedProvider::new(
            assets_dir.map(|p| p.to_path_buf()),
            allow_net,
        )))
    } else {
        None
    };

    // Build base URL for asset resolution
    let base_url = assets_dir.map(|p| {
        format!(
            "file://{}/",
            p.canonicalize()
                .unwrap_or_else(|_| p.to_path_buf())
                .display()
        )
    });

    // Create document
    let mut document = HtmlDocument::from_html(
        html,
        DocumentConfig {
            base_url,
            net_provider: provider.clone().map(|p| p as _),
            viewport: Some(Viewport::new(
                width * (scale as u32),
                800 * (scale as u32), // Initial height, will be recalculated
                scale,
                ColorScheme::Light,
            )),
            ..Default::default()
        },
    );

    // Resolve resource requests
    if let Some(ref p) = provider {
        loop {
            document.resolve(0.0);
            if p.is_empty() {
                break;
            }
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
            // Render white background
            scene.fill(
                Fill::NonZero,
                Default::default(),
                Color::WHITE,
                Default::default(),
                &Rect::new(0.0, 0.0, render_width as f64, render_height_scaled as f64),
            );

            // Render document
            paint_scene(
                scene,
                document.as_ref(),
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

    Ok(RenderBuffer {
        data: buffer,
        width: render_width,
        height: render_height_scaled,
    })
}

fn write_png(path: &Path, buffer: &[u8], width: u32, height: u32) -> Result<()> {
    // Set pixels-per-meter for 144 DPI
    const PPM: u32 = (144.0 * 39.3701) as u32;

    let file = fs::File::create(path)
        .with_context(|| format!("Failed to create file: {}", path.display()))?;

    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_pixel_dims(Some(png::PixelDimensions {
        xppu: PPM,
        yppu: PPM,
        unit: png::Unit::Meter,
    }));

    let mut writer = encoder.write_header()?;
    writer.write_image_data(buffer)?;
    writer.finish()?;

    Ok(())
}

fn write_jpeg(path: &Path, buffer: &[u8], width: u32, height: u32, quality: u8) -> Result<()> {
    // Convert RGBA to RGB for JPEG
    let rgb_buffer: Vec<u8> = buffer
        .chunks(4)
        .flat_map(|rgba| [rgba[0], rgba[1], rgba[2]])
        .collect();

    let img = image::RgbImage::from_raw(width, height, rgb_buffer)
        .context("Failed to create image buffer")?;

    let mut file = fs::File::create(path)
        .with_context(|| format!("Failed to create file: {}", path.display()))?;

    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, quality);
    encoder.encode_image(&img)?;

    Ok(())
}

fn write_webp(path: &Path, buffer: &[u8], width: u32, height: u32, _quality: u8) -> Result<()> {
    // Note: image crate's webp encoder doesn't support quality setting for lossy
    // We use lossless encoding
    let img = image::RgbaImage::from_raw(width, height, buffer.to_vec())
        .context("Failed to create image buffer")?;

    let mut file = fs::File::create(path)
        .with_context(|| format!("Failed to create file: {}", path.display()))?;

    let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut file);
    // Note: image crate's webp encoder doesn't support quality setting for lossy
    // We use lossless for now
    encoder.encode(&img, width, height, image::ExtendedColorType::Rgba8)?;

    Ok(())
}
