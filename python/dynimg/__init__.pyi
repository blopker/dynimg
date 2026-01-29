"""Type stubs for dynimg"""

from typing import Optional

class RenderOptions:
    """Options for rendering HTML to an image"""

    width: int
    height: Optional[int]
    scale: float
    allow_net: bool
    assets_dir: Optional[str]
    base_url: Optional[str]

    def __init__(
        self,
        width: int = 1200,
        height: Optional[int] = None,
        scale: float = 2.0,
        allow_net: bool = False,
        assets_dir: Optional[str] = None,
        base_url: Optional[str] = None,
    ) -> None: ...

class Image:
    """A rendered image with RGBA pixel data"""

    @property
    def width(self) -> int:
        """Image width in pixels"""
        ...

    @property
    def height(self) -> int:
        """Image height in pixels"""
        ...

    @property
    def data(self) -> bytes:
        """Raw RGBA pixel data"""
        ...

    def save_png(self, path: str) -> None:
        """Save the image as PNG"""
        ...

    def save_jpeg(self, path: str, quality: int = 90) -> None:
        """Save the image as JPEG with the specified quality (1-100)"""
        ...

    def save_webp(self, path: str) -> None:
        """Save the image as lossless WebP"""
        ...

    def to_png(self) -> bytes:
        """Encode the image as PNG bytes"""
        ...

    def to_jpeg(self, quality: int = 90) -> bytes:
        """Encode the image as JPEG bytes with the specified quality (1-100)"""
        ...

    def to_webp(self) -> bytes:
        """Encode the image as lossless WebP bytes"""
        ...

def render(html: str, options: Optional[RenderOptions] = None) -> Image:
    """
    Render HTML to an image.

    Args:
        html: The HTML content to render
        options: Rendering options (optional, uses defaults if not provided)

    Returns:
        The rendered image

    Example:
        >>> import dynimg
        >>> html = '<html><body style="background: blue;"><h1>Hello</h1></body></html>'
        >>> image = dynimg.render(html)
        >>> image.save_png("output.png")
    """
    ...

def render_to_file(
    html: str,
    path: str,
    options: Optional[RenderOptions] = None,
    quality: int = 90,
) -> None:
    """
    Render HTML and save directly to a file.

    The output format is detected from the file extension.

    Args:
        html: The HTML content to render
        path: Output file path (.png, .jpg, .webp)
        options: Rendering options (optional)
        quality: JPEG quality 1-100 (default: 90, ignored for PNG/WebP)

    Example:
        >>> import dynimg
        >>> html = '<html><body><h1>Hello</h1></body></html>'
        >>> dynimg.render_to_file(html, "output.png")
    """
    ...

__version__: str
__all__: list[str]
