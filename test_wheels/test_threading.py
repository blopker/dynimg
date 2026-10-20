"""Test concurrent rendering from multiple threads.

Guards against the "already mutably borrowed" panic that occurs when
stylo's global rayon thread pool is accessed concurrently from
multiple render calls.
"""

import threading

import dynimg

NUM_THREADS = 8
RENDERS_PER_THREAD = 10
TIMEOUT_SECONDS = 60

HTML_TEMPLATES = [
    '<html><body style="background:blue;"><h1>Thread {n}</h1></body></html>',
    '<html><body style="background:linear-gradient(red,blue);"><h1>Thread {n}</h1></body></html>',
    '<html><body style="background:green; display:flex; justify-content:center;"><h1 style="color:white;">Thread {n}</h1></body></html>',
    '<html><body style="background:#333;"><h1 style="color:yellow; font-family:system-ui;">Thread {n}</h1><p style="color:white;">Concurrent render test</p></body></html>',
]


def test_concurrent_renders():
    results = {}

    def render_many(thread_id):
        try:
            for i in range(RENDERS_PER_THREAD):
                html = HTML_TEMPLATES[i % len(HTML_TEMPLATES)].format(n=thread_id)
                options = dynimg.RenderOptions(width=200, height=200, scale=1.0)
                img = dynimg.render(html, options)
                assert (img.width, img.height) == (200, 200)
            results[thread_id] = "ok"
        except Exception as e:
            results[thread_id] = f"{type(e).__name__}: {e}"

    # Daemon threads, so a hung render fails the test instead of blocking exit
    threads = [
        threading.Thread(target=render_many, args=(n,), daemon=True)
        for n in range(NUM_THREADS)
    ]
    for t in threads:
        t.start()
    for t in threads:
        t.join(timeout=TIMEOUT_SECONDS)

    # A thread missing from results hung past the timeout
    assert results == dict.fromkeys(range(NUM_THREADS), "ok")
