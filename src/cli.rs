//! The `dynimg` command line, shared by the Rust binary (src/main.rs) and
//! the Python console script (`dynimg.main`).

use crate::{ImageFormat, RenderOptions, render};
use anyhow::{Context, Result, bail};
use clap::Parser;
use std::ffi::OsString;
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

    /// Viewport width in CSS pixels [default: 1200]
    #[arg(short, long)]
    width: Option<u32>,

    /// Viewport height in CSS pixels (defaults to document height)
    #[arg(short = 'H', long)]
    height: Option<u32>,

    /// Scale factor for high-DPI rendering [default: 2]
    #[arg(short, long)]
    scale: Option<f32>,

    /// JPEG quality (1-100) [default: 90]
    #[arg(short, long)]
    quality: Option<u8>,

    /// Allow network access for loading remote resources
    #[arg(long)]
    allow_net: bool,

    /// Asset directory for local resources (enables filesystem access)
    #[arg(long)]
    assets: Option<PathBuf>,

    /// Custom font files (TTF/OTF/WOFF/WOFF2), repeatable. FILE registers
    /// the font under the family name inside the file. NAME=FILE registers
    /// under a CSS name instead: a generic (sans-serif, emoji, ...) maps
    /// that generic with priority over the platform mapping, any other name
    /// becomes the font-family name.
    #[arg(long = "font", value_name = "[NAME=]FILE")]
    fonts: Vec<String>,

    /// Enable verbose logging
    #[arg(short = 'v', long)]
    verbose: bool,
}

/// Run the CLI with `argv` (including the program name) and return the
/// process exit code. Never calls `process::exit` itself, so it is safe to
/// run inside a host process like the Python interpreter.
pub fn run<I, T>(argv: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let args = match Args::try_parse_from(argv) {
        Ok(args) => args,
        Err(e) => {
            // Prints help/version to stdout, usage errors to stderr
            let _ = e.print();
            return e.exit_code();
        }
    };
    let result = tokio::runtime::Runtime::new()
        .context("Failed to start async runtime")
        .and_then(|rt| rt.block_on(run_args(args)));
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {e:?}");
            1
        }
    }
}

async fn run_args(args: Args) -> Result<()> {
    // Initialize tracing if verbose. try_init: a host process may have
    // installed a subscriber already.
    if args.verbose {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::from_default_env().add_directive("blitz_dom=debug".parse().unwrap()),
            )
            .with_target(true)
            .try_init();
    }

    // Determine output destination (`None` means stdout):
    // - `-o path`  → write to that file
    // - `-o -`     → always write to stdout
    // - no `-o`    → if stdout is piped/redirected, write to stdout (png default)
    //              → otherwise, default to $INPUT_STEM.jpg in cwd
    let output_path = match &args.output {
        Some(p) if p.as_os_str() == "-" => None,
        Some(p) => Some(p.clone()),
        None if !io::stdout().is_terminal() => None,
        None => {
            // Default to input_stem.jpg in cwd
            let stem = Path::new(&args.input)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("output");
            Some(PathBuf::from(format!("{}.jpg", stem)))
        }
    };

    let format = match (&output_path, args.format.as_deref()) {
        (Some(path), _) => ImageFormat::from_path(path)?,
        (None, None) => ImageFormat::Png,
        (None, Some(f)) => {
            ImageFormat::from_extension(f).with_context(|| format!("Unsupported format: {f}"))?
        }
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

    // Unset width/height/scale/quality fall back to meta tags, then defaults
    let mut options = RenderOptions {
        width: args.width,
        height: args.height,
        scale: args.scale,
        quality: args.quality,
        allow_net: args.allow_net,
        assets_dir: args.assets.clone(),
        verbose: args.verbose,
        ..Default::default()
    }
    .for_format(format);

    for font_arg in &args.fonts {
        // A real file wins over NAME=FILE parsing, so paths containing '='
        // still work as long as they exist. Files are read at render time.
        options = if Path::new(font_arg).is_file() {
            options.font_file(font_arg)
        } else if let Some((name, file)) = font_arg.split_once('=') {
            options.named_font_file(name, file)
        } else {
            bail!("Font file not found: {font_arg} (expected FILE or NAME=FILE)");
        };
    }

    // Set base URL from input file directory if not using assets
    if args.assets.is_none() && args.input != "-" {
        let input_path = Path::new(&args.input);
        if let Some(dir) = input_path.parent().and_then(|p| p.canonicalize().ok()) {
            options.base_url = Some(format!("file://{}/", dir.display()));
        }
    }

    if args.verbose {
        eprintln!("[config] output: {format:?}");
        eprintln!("[config] allow_net: {}", args.allow_net);
        eprintln!("[config] assets: {:?}", args.assets);
        eprintln!(
            "[config] width: {:?}, height: {:?}, scale: {:?}, quality: {:?} (None = meta tag or default)",
            args.width, args.height, args.scale, args.quality
        );
    }

    let image = render(&html, options).await?;
    let bytes = image.encode(format)?;

    let destination = match &output_path {
        Some(path) => {
            fs::write(path, &bytes)
                .with_context(|| format!("Failed to write {}", path.display()))?;
            path.display().to_string()
        }
        None => {
            // Explicit flush: when embedded in Python, Rust's stdout buffer
            // is not flushed at interpreter exit.
            let mut stdout = io::stdout();
            stdout
                .write_all(&bytes)
                .and_then(|()| stdout.flush())
                .context("Failed to write to stdout")?;
            "stdout".to_string()
        }
    };
    eprintln!(
        "Wrote {}x{} image to {destination}",
        image.width, image.height
    );

    Ok(())
}
