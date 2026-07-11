//! Concurrent render() calls must not panic. Style resolution runs with
//! StyleThreading::Sequential, which bypasses Stylo's global rayon pool —
//! two documents resolving on that shared pool panic with "already mutably
//! borrowed" (https://github.com/DioxusLabs/blitz/issues/430).
//!
//! render()'s future is not Send, so concurrency means one runtime per OS
//! thread — the same shape as multi-threaded Python callers.

use dynimg::{RenderOptions, render};

#[test]
fn concurrent_renders_do_not_panic() {
    let html = r#"
        <html>
        <body style="background: #4f46e5; padding: 40px;">
            <h1 style="color: white; font-family: sans-serif;">Concurrent</h1>
            <p style="color: white;">Lorem ipsum dolor sit amet, consectetur.</p>
        </body>
        </html>
    "#;

    let handles: Vec<_> = (0..8)
        .map(|_| {
            std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build runtime");
                rt.block_on(render(html, RenderOptions::with_size(400, 300)))
                    .expect("render failed")
            })
        })
        .collect();

    for handle in handles {
        let image = handle.join().expect("render thread panicked");
        assert_eq!(image.width, 800); // 400 x 2.0 default scale
        assert_eq!(image.height, 600);
    }
}
