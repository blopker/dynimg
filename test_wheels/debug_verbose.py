#!/usr/bin/env python3
"""Debug with verbose output."""
import os
os.environ["RUST_LOG"] = "debug"

import dynimg

# Simple test
html = "<html><body></body></html>"
options = dynimg.RenderOptions(width=100, height=100, scale=1.0)
img = dynimg.render(html, options)
print(f"Done: {img.width}x{img.height}")
