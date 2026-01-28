#!/usr/bin/env python3
"""Debug script to find which CSS causes parser errors."""

import dynimg

options = dynimg.RenderOptions(width=100, height=100, scale=1.0)

tests = [
    ("Empty body", "<html><body></body></html>"),
    ("Background color", '<html><body style="background: blue;"></body></html>'),
    ("Background shorthand", '<html><body style="background: #667eea;"></body></html>'),
    ("Background-color", '<html><body style="background-color: blue;"></body></html>'),
    ("Width/height", '<html><body style="width: 100px; height: 100px;"></body></html>'),
    ("Margin", '<html><body style="margin: 0;"></body></html>'),
    ("Display flex", '<html><body style="display: flex;"></body></html>'),
    ("Justify-content", '<html><body style="display: flex; justify-content: center;"></body></html>'),
    ("Align-items", '<html><body style="display: flex; align-items: center;"></body></html>'),
    ("Linear gradient", '<html><body style="background: linear-gradient(135deg, #667eea, #764ba2);"></body></html>'),
    ("Font-family system-ui", '<html><body style="font-family: system-ui;"></body></html>'),
    ("Font-family sans-serif", '<html><body style="font-family: sans-serif;"></body></html>'),
    ("Font-size", '<html><body style="font-size: 64px;"></body></html>'),
    ("Color", '<html><body style="color: white;"></body></html>'),
    ("H1 tag", "<html><body><h1>Test</h1></body></html>"),
    ("H1 with style", '<html><body><h1 style="color: white;">Test</h1></body></html>'),
]

print("Testing CSS properties to find parser error source...")
print("=" * 60)

for name, html in tests:
    print(f"\n[{name}]")
    print(f"HTML: {html[:80]}{'...' if len(html) > 80 else ''}")
    try:
        img = dynimg.render(html, options)
        print(f"Result: {img.width}x{img.height}")
    except Exception as e:
        print(f"Exception: {e}")
