//! Python bindings for dynimg

use crate::{RenderOptions as RustRenderOptions, RenderedImage, render as rust_render};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::path::PathBuf;

/// Options for rendering HTML to an image
#[pyclass(from_py_object)]
#[derive(Clone)]
pub struct RenderOptions {
    /// Viewport width in CSS pixels (default: 1200)
    #[pyo3(get, set)]
    pub width: u32,

    /// Viewport height in CSS pixels. If None, auto-sizes to content height.
    #[pyo3(get, set)]
    pub height: Option<u32>,

    /// Scale factor for output resolution (default: 2.0 for retina displays).
    #[pyo3(get, set)]
    pub scale: f32,

    /// Allow network requests for loading remote resources
    #[pyo3(get, set)]
    pub allow_net: bool,

    /// Directory for loading local assets
    #[pyo3(get, set)]
    pub assets_dir: Option<String>,

    /// Base URL for resolving relative paths
    #[pyo3(get, set)]
    pub base_url: Option<String>,

    /// Background color as CSS hex string (e.g. "#ffffff"). Default: transparent.
    #[pyo3(get, set)]
    pub background: Option<String>,

    /// Enable verbose output. When false (default), dependency output is suppressed.
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
    #[pyo3(signature = (*, width=1200, height=None, scale=2.0, allow_net=false, assets_dir=None, base_url=None, background=None, verbose=false, fonts=None))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        width: u32,
        height: Option<u32>,
        scale: f32,
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
            "RenderOptions(width={}, height={:?}, scale={}, allow_net={}, assets_dir={:?}, background={:?}, verbose={}, fonts=<{} font(s), {} named>)",
            self.width,
            self.height,
            self.scale,
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

    /// Save the image as PNG
    fn save_png(&self, path: &str) -> PyResult<()> {
        self.inner
            .save_png(path)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))
    }

    /// Save the image as JPEG with the specified quality (1-100)
    #[pyo3(signature = (path, quality=90))]
    fn save_jpeg(&self, path: &str, quality: u8) -> PyResult<()> {
        self.inner
            .save_jpeg(path, quality)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))
    }

    /// Save the image as lossless WebP
    fn save_webp(&self, path: &str) -> PyResult<()> {
        self.inner
            .save_webp(path)
            .map_err(|e| pyo3::exceptions::PyIOError::new_err(e.to_string()))
    }

    /// Encode the image as PNG bytes
    fn to_png<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let data = self
            .inner
            .to_png()
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(PyBytes::new(py, &data))
    }

    /// Encode the image as JPEG bytes with the specified quality (1-100)
    #[pyo3(signature = (quality=90))]
    fn to_jpeg<'py>(&self, py: Python<'py>, quality: u8) -> PyResult<Bound<'py, PyBytes>> {
        let data = self
            .inner
            .to_jpeg(quality)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
        Ok(PyBytes::new(py, &data))
    }

    /// Encode the image as lossless WebP bytes
    fn to_webp<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        let data = self.inner.to_webp();
        PyBytes::new(py, &data)
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

    // Release GIL during rendering so other Python threads aren't blocked.
    // Concurrent renders are safe: style resolution runs sequentially per
    // document (StyleThreading::Sequential in lib.rs).
    py.detach(|| {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let result = rt.block_on(rust_render(html, opts));

        match result {
            Ok(image) => Ok(Image { inner: image }),
            Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
        }
    })
}

/// Render HTML and save directly to a file.
/// The output format is detected from the file extension.
///
/// Args:
///     html: The HTML content to render
///     path: Output file path (.png, .jpg, .webp)
///     options: Rendering options (optional)
///     quality: JPEG/WebP quality 1-100 (default: 90)
///
/// Example:
///     >>> import dynimg
///     >>> html = '<html><body><h1>Hello</h1></body></html>'
///     >>> dynimg.render_to_file(html, "output.png")
#[pyfunction]
#[pyo3(signature = (html, path, options=None, quality=90))]
fn render_to_file(
    py: Python<'_>,
    html: &str,
    path: &str,
    options: Option<RenderOptions>,
    quality: u8,
) -> PyResult<()> {
    let opts: RustRenderOptions = options.map(Into::into).unwrap_or_default();

    py.detach(|| {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        let result = rt.block_on(crate::render_to_file(html, path, opts, quality));

        match result {
            Ok(()) => Ok(()),
            Err(e) => Err(pyo3::exceptions::PyRuntimeError::new_err(e.to_string())),
        }
    })
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
