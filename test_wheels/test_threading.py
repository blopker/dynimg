#!/usr/bin/env python3
"""Test concurrent rendering from multiple threads.

Reproduces the "already mutably borrowed" panic that occurs when
stylo's global rayon thread pool is accessed concurrently from
multiple render calls.
"""

import sys
import threading
import traceback

NUM_THREADS = 8
RENDERS_PER_THREAD = 10

HTML_TEMPLATES = [
    '<html><body style="background:blue;"><h1>Thread {n}</h1></body></html>',
    '<html><body style="background:linear-gradient(red,blue);"><h1>Thread {n}</h1></body></html>',
    '<html><body style="background:green; display:flex; justify-content:center;"><h1 style="color:white;">Thread {n}</h1></body></html>',
    '<html><body style="background:#333;"><h1 style="color:yellow; font-family:system-ui;">Thread {n}</h1><p style="color:white;">Concurrent render test</p></body></html>',
]


def render_worker(thread_id, results):
    """Render multiple images in a thread."""
    import dynimg

    try:
        for i in range(RENDERS_PER_THREAD):
            html = HTML_TEMPLATES[i % len(HTML_TEMPLATES)].format(n=thread_id)
            options = dynimg.RenderOptions(width=200, height=200, scale=1.0)
            img = dynimg.render(html, options)
            assert img.width == 200
            assert img.height == 200
        results[thread_id] = ("ok", RENDERS_PER_THREAD)
    except Exception as e:
        results[thread_id] = ("error", f"{type(e).__name__}: {e}")
        traceback.print_exc()


def main():
    print(f"Testing concurrent rendering: {NUM_THREADS} threads x {RENDERS_PER_THREAD} renders each")
    print(f"Total renders: {NUM_THREADS * RENDERS_PER_THREAD}")
    print()

    results = {}
    threads = []

    for i in range(NUM_THREADS):
        t = threading.Thread(target=render_worker, args=(i, results))
        threads.append(t)

    # Start all threads at once for maximum contention
    for t in threads:
        t.start()

    for t in threads:
        t.join(timeout=60)

    # Report results
    ok_count = sum(1 for status, _ in results.values() if status == "ok")
    err_count = sum(1 for status, _ in results.values() if status == "error")
    total_renders = sum(v for status, v in results.values() if status == "ok")

    print()
    print(f"Threads completed: {ok_count}/{NUM_THREADS}")
    print(f"Threads failed:    {err_count}/{NUM_THREADS}")
    print(f"Total renders:     {total_renders}/{NUM_THREADS * RENDERS_PER_THREAD}")

    if err_count > 0:
        print()
        print("ERRORS:")
        for tid, (status, msg) in sorted(results.items()):
            if status == "error":
                print(f"  Thread {tid}: {msg}")

    print()
    if err_count == 0:
        print("PASS: All concurrent renders succeeded")
    else:
        print("FAIL: Concurrent rendering panicked")

    return err_count == 0


if __name__ == "__main__":
    success = main()
    sys.exit(0 if success else 1)
