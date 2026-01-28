use anyhow::{Context, Result, bail};
use anyrender::{PaintScene as _, render_to_buffer};
use anyrender_vello_cpu::VelloCpuImageRenderer;
use blitz_dom::{BaseDocument, DocumentConfig, util::Color};
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
use tracing_subscriber::EnvFilter;

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
    verbose: bool,
}

impl CombinedProvider {
    fn new(assets_dir: Option<PathBuf>, allow_net: bool, verbose: bool) -> Self {
        Self {
            assets: assets_dir.map(AssetProvider::new),
            network: if allow_net {
                Some(Arc::new(Provider::new(None)))
            } else {
                None
            },
            verbose,
        }
    }

    fn is_empty(&self) -> bool {
        self.network.as_ref().map(|n| n.is_empty()).unwrap_or(true)
    }
}

impl NetProvider for CombinedProvider {
    fn fetch(&self, doc_id: usize, request: Request, handler: Box<dyn NetHandler>) {
        let url = request.url.to_string();

        if self.verbose {
            eprintln!("[fetch] {} (method: {:?})", url, request.method);
        }

        // Check if it's a file URL or relative path that assets can handle
        if !url.starts_with("http://")
            && !url.starts_with("https://")
            && let Some(ref assets) = self.assets
        {
            if self.verbose {
                eprintln!("[fetch] -> assets provider");
            }
            assets.fetch(doc_id, request, handler);
            return;
        }

        // Otherwise try network
        if let Some(ref network) = self.network {
            if self.verbose {
                eprintln!("[fetch] -> network provider");
            }
            network.fetch(doc_id, request, handler);
        } else if self.verbose {
            eprintln!("[fetch] -> SKIPPED (no network provider)");
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

    /// Enable verbose logging
    #[arg(short = 'v', long)]
    verbose: bool,
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

    // Initialize tracing if verbose
    if args.verbose {
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::from_default_env()
                    .add_directive("blitz_dom=debug".parse().unwrap())
                    .add_directive("blitz_net=debug".parse().unwrap()),
            )
            .with_target(true)
            .init();
    }

    // Detect output format from extension
    let format = OutputFormat::from_path(&args.output)?;

    // Read HTML input and determine input directory for base URL
    let (html, input_dir) = if args.input == "-" {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        (buffer, None)
    } else {
        let content = fs::read_to_string(&args.input)
            .with_context(|| format!("Failed to read file: {}", args.input))?;
        let input_path = Path::new(&args.input);
        let dir = input_path
            .parent()
            .and_then(|p| p.canonicalize().ok())
            .or_else(|| std::env::current_dir().ok());
        (content, dir)
    };

    // Create provider for assets and/or network
    let has_provider = args.allow_net || args.assets.is_some();
    let provider = if has_provider {
        Some(Arc::new(CombinedProvider::new(
            args.assets.clone(),
            args.allow_net,
            args.verbose,
        )))
    } else {
        None
    };

    if args.verbose {
        eprintln!("[config] allow_net: {}", args.allow_net);
        eprintln!("[config] assets: {:?}", args.assets);
        eprintln!(
            "[config] width: {}, height: {:?}, scale: {}",
            args.width, args.height, args.scale
        );
    }

    // Build base URL for asset resolution
    // Priority: --assets flag > input file directory > none
    let base_url = args
        .assets
        .as_ref()
        .and_then(|p| p.canonicalize().ok())
        .or(input_dir)
        .map(|p| format!("file://{}/", p.display()));

    // Parse document once
    let mut document = HtmlDocument::from_html(
        &html,
        DocumentConfig {
            base_url,
            net_provider: provider.clone().map(|p| p as _),
            viewport: Some(Viewport::new(
                args.width * (args.scale as u32),
                800 * (args.scale as u32),
                args.scale,
                ColorScheme::Light,
            )),
            ..Default::default()
        },
    );

    // Extract meta options from parsed document
    let meta_options = extract_meta_options(document.as_ref());

    // Merge options: CLI args take precedence over meta tags
    let width = if args.width == 1200 {
        meta_options.width.unwrap_or(args.width)
    } else {
        args.width
    };
    let height = args.height.or(meta_options.height);
    let scale = if (args.scale - 2.0).abs() < 0.001 {
        meta_options.scale.unwrap_or(args.scale)
    } else {
        args.scale
    };
    let quality = if args.quality == 90 {
        meta_options.quality.unwrap_or(args.quality)
    } else {
        args.quality
    };

    // Render the document
    let buffer =
        render_document(&mut document, &provider, width, height, scale, args.verbose).await?;

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

/// Extract dynimg meta tags from a parsed document
fn extract_meta_options(doc: &BaseDocument) -> RenderOptions {
    let mut options = RenderOptions::default();

    // Use a stack-based traversal starting from root
    let mut stack = vec![0usize]; // Start from root node (id 0)

    while let Some(node_id) = stack.pop() {
        let Some(node) = doc.get_node(node_id) else {
            continue;
        };

        // Add children to stack for traversal
        stack.extend(node.children.iter().copied());

        // Check if this is an element node
        let Some(element) = node.element_data() else {
            continue;
        };

        // Check if it's a meta element
        if !element.name.local.eq_str_ignore_ascii_case("meta") {
            continue;
        }

        // Find name and content attributes by iterating
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

        // Parse dynimg options
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

struct RenderBuffer {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

async fn render_document(
    document: &mut HtmlDocument,
    provider: &Option<Arc<CombinedProvider>>,
    width: u32,
    height: Option<u32>,
    scale: f32,
    verbose: bool,
) -> Result<RenderBuffer> {
    // Resolve resource requests (images, stylesheets, fonts)
    if let Some(p) = provider {
        let mut resolve_count = 0;

        // Process network requests until all are complete
        loop {
            document.resolve(0.0);
            resolve_count += 1;
            if p.is_empty() {
                break;
            }
            // Brief async sleep to allow network I/O to progress
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }

        // Extra resolve cycles to process any resources that arrived
        // Resources like fonts need to be registered after fetching
        for _ in 0..3 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            document.resolve(0.0);
            resolve_count += 1;
        }

        if verbose {
            eprintln!("[resolve] completed {} cycles", resolve_count);
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

fn write_webp(path: &Path, buffer: &[u8], width: u32, height: u32, quality: u8) -> Result<()> {
    // Use webp crate for lossy encoding with quality control
    let encoder = webp::Encoder::from_rgba(buffer, width, height);
    let webp_data = encoder.encode(quality as f32);

    fs::write(path, &*webp_data)
        .with_context(|| format!("Failed to write file: {}", path.display()))?;

    Ok(())
}
