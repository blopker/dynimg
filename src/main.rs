use anyhow::{Context, Result, bail};
use clap::Parser;
use dynimg::{RenderOptions, render};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use tracing_subscriber::EnvFilter;

/// A fast CLI tool for generating high-quality images from HTML/CSS
#[derive(Parser, Debug)]
#[command(name = "dynimg", version, about)]
struct Args {
    /// HTML file path or '-' for stdin
    input: String,

    /// Output image path (format detected from extension). Omit or use '-' for stdout.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Output format when writing to stdout (default: png)
    #[arg(short, long)]
    format: Option<String>,

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

    /// Custom fonts (TTF/OTF/WOFF/WOFF2), repeatable. PATH is a font file or
    /// a directory of fonts; family names are read from the font files.
    /// NAME=FILE registers under a CSS name instead: a generic (sans-serif,
    /// emoji, ...) maps that generic to the font, any other name becomes the
    /// font-family name.
    #[arg(long = "font", value_name = "[NAME=]PATH")]
    fonts: Vec<String>,

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

    // Determine output destination:
    // - `-o path`  → write to that file
    // - `-o -`     → always write to stdout
    // - no `-o`    → if stdout is piped/redirected, write to stdout (png default)
    //              → otherwise, default to $INPUT_STEM.jpg in cwd
    let (output_path, stdout_mode) = match &args.output {
        Some(p) if p.as_os_str() == "-" => (None, true),
        Some(p) => (Some(p.clone()), false),
        None if !io::stdout().is_terminal() => (None, true),
        None => {
            // Default to input_stem.jpg in cwd
            let stem = Path::new(&args.input)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            (Some(PathBuf::from(format!("{}.jpg", stem))), false)
        }
    };

    let format = if stdout_mode {
        match args.format.as_deref() {
            None | Some("png") => OutputFormat::Png,
            Some("jpg") | Some("jpeg") => OutputFormat::Jpeg,
            Some("webp") => OutputFormat::WebP,
            Some(f) => bail!("Unsupported format: {}", f),
        }
    } else {
        OutputFormat::from_path(output_path.as_ref().unwrap())?
    };

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
    // Use white background for JPEG (no transparency support)
    let background = match format {
        OutputFormat::Jpeg => Some("#ffffff".to_string()),
        _ => None,
    };

    let mut options = RenderOptions {
        width: args.width,
        height: args.height,
        scale: args.scale,
        allow_net: args.allow_net,
        assets_dir: args.assets.clone(),
        base_url: None,
        background,
        verbose: args.verbose,
        fonts: Vec::new(),
        named_fonts: Vec::new(),
    };

    for font_arg in &args.fonts {
        let path = Path::new(font_arg);
        // A real path wins over NAME=FILE parsing, so paths containing '='
        // still work as long as they exist.
        options = if path.is_dir() {
            options.font_dir(path)
        } else if path.is_file() {
            options.font_file(path)
        } else if let Some((name, file)) = font_arg.split_once('=') {
            options.named_font_file(name, file)
        } else {
            bail!("Font not found: {font_arg} (expected a font file, directory, or NAME=FILE)");
        }
        .with_context(|| format!("Failed to load font(s) from: {font_arg}"))?;
    }

    // Set base URL from input file directory if not using assets
    if args.assets.is_none() && args.input != "-" {
        let input_path = Path::new(&args.input);
        if let Some(dir) = input_path.parent().and_then(|p| p.canonicalize().ok()) {
            options.base_url = Some(format!("file://{}/", dir.display()));
        }
    }

    if args.verbose && stdout_mode {
        eprintln!("[config] output: stdout ({:?})", format);
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

    // Output the image
    if stdout_mode {
        let bytes = match format {
            OutputFormat::Png => image.to_png()?,
            OutputFormat::Jpeg => image.to_jpeg(args.quality)?,
            OutputFormat::WebP => image.to_webp(),
        };
        io::stdout()
            .write_all(&bytes)
            .context("Failed to write to stdout")?;
        eprintln!("Wrote {}x{} image to stdout", image.width, image.height);
    } else {
        let output = output_path.as_ref().unwrap();
        match format {
            OutputFormat::Png => image.save_png(output)?,
            OutputFormat::Jpeg => image.save_jpeg(output, args.quality)?,
            OutputFormat::WebP => image.save_webp(output)?,
        }
        eprintln!(
            "Wrote {}x{} image to {}",
            image.width,
            image.height,
            output.display()
        );
    }

    Ok(())
}
