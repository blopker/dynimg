//! Python bindings for dynimg

use crate::{Error, RenderOptions as RustRenderOptions, RenderedImage, render as rust_render};
use pyo3::exceptions::{PyIOError, PyRuntimeError};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::path::PathBuf;

impl From<Error> for PyErr {
    fn from(err: Error) -> Self {
        match err {
            Error::Io(_) => PyIOError::new_err(err.to_string()),
            _ => PyRuntimeError::new_err(err.to_string()),
        }
    }
}

/// Run an async render on a fresh runtime with the GIL released, so other
/// Python threads aren't blocked. Concurrent renders are safe: style
/// resolution runs sequentially per document (StyleThreading::Sequential in
/// lib.rs). Takes a closure because render futures aren't `Send`, so they
/// must be created on the thread that runs them.
fn block_on<T: Send, Fut: Future<Output = Result<T, Error>>>(
    py: Python<'_>,
    make_future: impl FnOnce() -> Fut + Send,
) -> PyResult<T> {
    py.detach(|| {
        let rt = tokio::runtime::Runtime::new()?;
        Ok(rt.block_on(make_future())?)
    })
}

/// Options for rendering HTML to an image
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct RenderOptions {
    /// Viewport width in CSS pixels (default: meta tag, then 1200)
    #[pyo3(get, set)]
    pub width: Option<u32>,

    /// Viewport height in CSS pixels. If None, auto-sizes to content height.
    #[pyo3(get, set)]
    pub height: Option<u32>,

    /// Scale factor for output resolution (default: meta tag, then 2.0 for
    /// retina displays).
    #[pyo3(get, set)]
    pub scale: Option<f32>,

    /// JPEG quality 1-100 (default: meta tag, then 90)
    #[pyo3(get, set)]
    pub quality: Option<u8>,

    /// Allow network requests for loading remote resources
    #[pyo3(get, set)]
    pub allow_net: bool,

    /// Directory for loading local assets
    #[pyo3(get, set)]
    pub assets_dir: Option<String>,

    /// Base URL for resolving relative paths
    #[pyo3(get, set)]
    pub base_url: Option<String>,

    /// Background color as any CSS color (e.g. "#ffffff", "white"). Default: transparent.
    #[pyo3(get, set)]
    pub background: Option<String>,

    /// Reserved for future diagnostic output; currently has no effect.
    #[pyo3(get, set)]
    pub verbose: bool,

    /// Custom font file paths (TTF/OTF/WOFF/WOFF2), read at render time
    pub fonts: Vec<String>,

    /// CSS name -> font file path. Generic names map that generic; other
    /// names register the font under that family name.
    pub named_fonts: Vec<(String, String)>,
}

/// The `fonts` argument: one path, a mapping of CSS name -> path, or a list
/// mixing both. Map must come first (a dict extracts only as a dict), and
/// Single before List (a str would otherwise extract as a list of chars).
#[derive(FromPyObject)]
enum FontsArg {
    #[pyo3(transparent)]
    Map(std::collections::HashMap<String, String>),
    #[pyo3(transparent)]
    Single(String),
    #[pyo3(transparent)]
    List(Vec<FontListEntry>),
}

/// A `fonts` list element: a font file path or a name -> path mapping
#[derive(FromPyObject)]
enum FontListEntry {
    #[pyo3(transparent)]
    Map(std::collections::HashMap<String, String>),
    #[pyo3(transparent)]
    Path(String),
}

#[pymethods]
impl RenderOptions {
    #[new]
    #[pyo3(signature = (*, width=None, height=None, scale=None, quality=None, allow_net=false, assets_dir=None, base_url=None, background=None, verbose=false, fonts=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        width: Option<u32>,
        height: Option<u32>,
        scale: Option<f32>,
        quality: Option<u8>,
        allow_net: bool,
        assets_dir: Option<String>,
        base_url: Option<String>,
        background: Option<String>,
        verbose: bool,
        fonts: Option<FontsArg>,
    ) -> Self {
        let mut plain: Vec<String> = Vec::new();
        let mut named: Vec<(String, String)> = Vec::new();
        match fonts {
            None => {}
            Some(FontsArg::Single(path)) => plain.push(path),
            Some(FontsArg::Map(map)) => named.extend(map),
            Some(FontsArg::List(entries)) => {
                for entry in entries {
                    match entry {
                        FontListEntry::Path(path) => plain.push(path),
                        FontListEntry::Map(map) => named.extend(map),
                    }
                }
            }
        }

        Self {
            width,
            height,
            scale,
            quality,
            allow_net,
            assets_dir,
            base_url,
            background,
            verbose,
            fonts: plain,
            named_fonts: named,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "RenderOptions(width={:?}, height={:?}, scale={:?}, quality={:?}, allow_net={}, assets_dir={:?}, background={:?}, verbose={}, fonts=<{} font(s), {} named>)",
            self.width,
            self.height,
            self.scale,
            self.quality,
            self.allow_net,
            self.assets_dir,
            self.background,
            self.verbose,
            self.fonts.len(),
            self.named_fonts.len()
        )
    }
}

impl From<RenderOptions> for RustRenderOptions {
    fn from(opts: RenderOptions) -> Self {
        RustRenderOptions {
            width: opts.width,
            height: opts.height,
            scale: opts.scale,
            quality: opts.quality,
            allow_net: opts.allow_net,
            assets_dir: opts.assets_dir.map(PathBuf::from),
            base_url: opts.base_url,
            background: opts.background,
            verbose: opts.verbose,
            fonts: opts.fonts.into_iter().map(PathBuf::from).collect(),
            named_fonts: opts
                .named_fonts
                .into_iter()
                .map(|(name, path)| (name, PathBuf::from(path)))
                .collect(),
        }
    }
}

/// A rendered image with RGBA pixel data
#[pyclass]
pub struct Image {
    inner: RenderedImage,
}

#[pymethods]
impl Image {
    /// Image width in pixels
    #[getter]
    fn width(&self) -> u32 {
        self.inner.width
    }

    /// Image height in pixels
    #[getter]
    fn height(&self) -> u32 {
        self.inner.height
    }

    /// Raw RGBA pixel data as bytes
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.data)
    }

    /// JPEG quality resolved from the render options or the page's
    /// `dynimg:quality` meta tag (default 90)
    #[getter]
    fn quality(&self) -> u8 {
        self.inner.quality
    }

    /// Save the image as PNG
    fn save_png(&self, path: &str) -> PyResult<()> {
        Ok(self.inner.save_png(path)?)
    }

    /// Save the image as JPEG (quality 1-100, default: the image's quality)
    #[pyo3(signature = (path, quality=None))]
    fn save_jpeg(&self, path: &str, quality: Option<u8>) -> PyResult<()> {
        let quality = quality.unwrap_or(self.inner.quality);
        Ok(self.inner.save_jpeg(path, quality)?)
    }

    /// Save the image as lossless WebP
    fn save_webp(&self, path: &str) -> PyResult<()> {
        Ok(self.inner.save_webp(path)?)
    }

    /// Encode the image as PNG bytes
    fn to_png<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        Ok(PyBytes::new(py, &self.inner.to_png()?))
    }

    /// Encode the image as JPEG bytes (quality 1-100, default: the image's quality)
    #[pyo3(signature = (quality=None))]
    fn to_jpeg<'py>(&self, py: Python<'py>, quality: Option<u8>) -> PyResult<Bound<'py, PyBytes>> {
        let quality = quality.unwrap_or(self.inner.quality);
        Ok(PyBytes::new(py, &self.inner.to_jpeg(quality)?))
    }

    /// Encode the image as lossless WebP bytes
    fn to_webp<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.to_webp())
    }

    fn __repr__(&self) -> String {
        format!(
            "Image(width={}, height={})",
            self.inner.width, self.inner.height
        )
    }
}

/// Render HTML to an image
///
/// Args:
///     html: The HTML content to render
///     options: Rendering options (optional, uses defaults if not provided)
///
/// Returns:
///     Image: The rendered image
///
/// Example:
///     >>> import dynimg
///     >>> html = '<html><body style="background: blue;"><h1>Hello</h1></body></html>'
///     >>> image = dynimg.render(html)
///     >>> image.save_png("output.png")
#[pyfunction]
#[pyo3(signature = (html, options=None))]
fn render(py: Python<'_>, html: &str, options: Option<RenderOptions>) -> PyResult<Image> {
    let opts: RustRenderOptions = options.map(Into::into).unwrap_or_default();
    let inner = block_on(py, || rust_render(html, opts))?;
    Ok(Image { inner })
}

/// Render HTML and save directly to a file.
/// The output format is detected from the file extension.
///
/// Args:
///     html: The HTML content to render
///     path: Output file path (.png, .jpg, .webp)
///     options: Rendering options (optional)
///     quality: JPEG quality 1-100, overriding options.quality (default:
///         options.quality, then the dynimg:quality meta tag, then 90)
///
/// Example:
///     >>> import dynimg
///     >>> html = '<html><body><h1>Hello</h1></body></html>'
///     >>> dynimg.render_to_file(html, "output.png")
#[pyfunction]
#[pyo3(signature = (html, path, options=None, quality=None))]
fn render_to_file(
    py: Python<'_>,
    html: &str,
    path: &str,
    options: Option<RenderOptions>,
    quality: Option<u8>,
) -> PyResult<()> {
    let mut opts: RustRenderOptions = options.map(Into::into).unwrap_or_default();
    opts.quality = quality.or(opts.quality);
    block_on(py, || crate::render_to_file(html, path, opts))
}

/// Python module
#[pymodule]
pub fn _dynimg(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RenderOptions>()?;
    m.add_class::<Image>()?;
    m.add_function(wrap_pyfunction!(render, m)?)?;
    m.add_function(wrap_pyfunction!(render_to_file, m)?)?;
    Ok(())
}
