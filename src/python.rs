//! Python bindings for dynimg

use crate::{RenderOptions as RustRenderOptions, RenderedImage, render as rust_render};
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::path::PathBuf;

/// RAII guard to temporarily suppress stdout (blitz prints parse warnings to stdout)
struct SuppressStdout {
    saved_fd: libc::c_int,
}

impl SuppressStdout {
    fn new() -> Option<Self> {
        unsafe {
            // Save current stdout
            let saved_fd = libc::dup(libc::STDOUT_FILENO);
            if saved_fd < 0 {
                return None;
            }

            // Open /dev/null
            let null_fd = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY);
            if null_fd < 0 {
                libc::close(saved_fd);
                return None;
            }

            // Redirect stdout to /dev/null
            if libc::dup2(null_fd, libc::STDOUT_FILENO) < 0 {
                libc::close(null_fd);
                libc::close(saved_fd);
                return None;
            }

            libc::close(null_fd);
            Some(Self { saved_fd })
        }
    }
}

impl Drop for SuppressStdout {
    fn drop(&mut self) {
        unsafe {
            // Restore original stdout
            libc::dup2(self.saved_fd, libc::STDOUT_FILENO);
            libc::close(self.saved_fd);
        }
    }
}

/// Options for rendering HTML to an image
#[pyclass]
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
}

#[pymethods]
impl RenderOptions {
    #[new]
    #[pyo3(signature = (width=1200, height=None, scale=2.0, allow_net=false, assets_dir=None, base_url=None))]
    fn new(
        width: u32,
        height: Option<u32>,
        scale: f32,
        allow_net: bool,
        assets_dir: Option<String>,
        base_url: Option<String>,
    ) -> Self {
        Self {
            width,
            height,
            scale,
            allow_net,
            assets_dir,
            base_url,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "RenderOptions(width={}, height={:?}, scale={}, allow_net={}, assets_dir={:?})",
            self.width, self.height, self.scale, self.allow_net, self.assets_dir
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
    let opts: RustRenderOptions = options
        .unwrap_or_else(|| RenderOptions::new(1200, None, 2.0, false, None, None))
        .into();

    // Run the async render function (release GIL during blocking operation)
    py.detach(|| {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        // Suppress stdout during rendering (blitz prints parse errors to stdout)
        let _suppress = SuppressStdout::new();
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
    let opts: RustRenderOptions = options
        .unwrap_or_else(|| RenderOptions::new(1200, None, 2.0, false, None, None))
        .into();

    py.detach(|| {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        // Suppress stdout during rendering (blitz prints parse errors to stdout)
        let _suppress = SuppressStdout::new();
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
