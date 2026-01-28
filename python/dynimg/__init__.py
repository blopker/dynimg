"""
dynimg - Fast HTML/CSS to image rendering

Example:
    >>> import dynimg
    >>> html = '''
    ... <html>
    ... <body style="background: linear-gradient(135deg, #667eea, #764ba2);
    ...              display: flex; justify-content: center; align-items: center;
    ...              height: 630px; margin: 0;">
    ...     <h1 style="color: white; font-family: system-ui; font-size: 64px;">
    ...         Hello World
    ...     </h1>
    ... </body>
    ... </html>
    ... '''
    >>> image = dynimg.render(html)
    >>> image.save_png("output.png")
"""

from dynimg._dynimg import (
    RenderOptions,
    Image,
    render,
    render_to_file,
)

__all__ = [
    "RenderOptions",
    "Image",
    "render",
    "render_to_file",
]

__version__ = "0.1.0"
