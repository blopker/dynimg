"""Type stubs for dynimg"""

from typing import Mapping, Optional, Sequence, Union

class RenderOptions:
    """Options for rendering HTML to an image"""

    width: int
    height: Optional[int]
    scale: float
    allow_net: bool
    assets_dir: Optional[str]
    base_url: Optional[str]
    background: Optional[str]
    verbose: bool

    def __init__(
        self,
        *,
        width: int = 1200,
        height: Optional[int] = None,
        scale: float = 2.0,
        allow_net: bool = False,
        assets_dir: Optional[str] = None,
        base_url: Optional[str] = None,
        background: Optional[str] = None,
        verbose: bool = False,
        fonts: Optional[
            Union[
                str,
                bytes,
                Mapping[str, Union[str, bytes]],
                Sequence[Union[str, bytes, Mapping[str, Union[str, bytes]]]],
            ]
        ] = None,
    ) -> None:
        """
        Args:
            width: Viewport width in CSS pixels (default: 1200)
            height: Viewport height in CSS pixels (default: auto-sizes to content)
            scale: Scale factor for output resolution (default: 2.0 for retina)
            allow_net: Allow network requests for remote resources
            assets_dir: Directory for loading local assets
            base_url: Base URL for resolving relative paths
            background: Background color as CSS hex string, e.g. "#ffffff" (default: transparent)
            verbose: Enable verbose output (default: False). When True, dependency output is forwarded to stderr.
            fonts: Custom fonts. A font is a file path, a directory of font
                files (scanned recursively), or raw TTF/OTF/WOFF/WOFF2 bytes.
                Pass one font, a list of fonts, a mapping of CSS name -> font,
                or a list mixing fonts and mappings.

                Unnamed fonts register under the family names inside the font
                files, matching CSS font-family; they take priority over system
                fonts with the same name, and back generic families (sans-serif,
                ...) on hosts with no discoverable system fonts (e.g. minimal
                Docker containers without fontconfig).

                Mapping keys that are CSS generics ("serif", "sans-serif",
                "monospace", "cursive", "fantasy", "system-ui", "emoji", "math")
                map that generic to the font ahead of the platform mapping —
                "emoji" replaces the platform emoji font, so emoji render
                identically across hosts. Any other key registers the font
                under that family name instead of the name inside the file.

                Example: fonts=["./fonts", {"sans-serif": "./Inter.ttf",
                "emoji": "./Twemoji.ttf", "brand": "./Custom.ttf"}]
        """
        ...

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
