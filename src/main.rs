use anyhow::{Context, Result, bail};
use clap::Parser;
use dynimg::{RenderOptions, render};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;

/// A fast CLI tool for generating high-quality images from HTML/CSS
#[derive(Parser, Debug)]
#[command(name = "dynimg", version, about)]
struct Args {
    /// HTML file path or '-' for stdin
    input: String,

    /// Output image path (format detected from extension)
    #[arg(short, long)]
    output: PathBuf,

    /// Viewport width in CSS pixels
    #[arg(short, long, default_value = "1200")]
    width: u32,

    /// Viewport height in CSS pixels (defaults to document height)
    #[arg(short = 'H', long)]
    height: Option<u32>,

    /// Scale factor for high-DPI rendering
    #[arg(short, long, default_value = "2")]
    scale: f32,

    /// JPEG quality (1-100)
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

    // Build render options
    let mut options = RenderOptions {
        width: args.width,
        height: args.height,
        scale: args.scale,
        allow_net: args.allow_net,
        assets_dir: args.assets.clone(),
        base_url: None,
    };

    // Set base URL from input file directory if not using assets
    if args.assets.is_none() && args.input != "-" {
        let input_path = Path::new(&args.input);
        if let Some(dir) = input_path.parent().and_then(|p| p.canonicalize().ok()) {
            options.base_url = Some(format!("file://{}/", dir.display()));
        }
    }

    if args.verbose {
        eprintln!("[config] allow_net: {}", args.allow_net);
        eprintln!("[config] assets: {:?}", args.assets);
        eprintln!(
            "[config] width: {}, height: {:?}, scale: {}",
            args.width, args.height, args.scale
        );
    }

    // Render the document
    let image = render(&html, options).await?;

    // Save to file
    match format {
        OutputFormat::Png => image.save_png(&args.output)?,
        OutputFormat::Jpeg => image.save_jpeg(&args.output, args.quality)?,
        OutputFormat::WebP => image.save_webp(&args.output)?,
    }

    eprintln!(
        "Wrote {}x{} image to {}",
        image.width,
        image.height,
        args.output.display()
    );

    Ok(())
}
