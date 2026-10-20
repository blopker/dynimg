//! Perceptual snapshot comparison using zensim (SSIMULACRA2-style metric).
//!
//! Byte-exact comparison is too strict for snapshots: JPEG encoding and, on
//! gradient/mask-heavy pages, vello_cpu rasterization differ slightly across
//! CPU architectures (SIMD rounding), and encoder version bumps can change
//! compressed bytes without changing pixels. This compares decoded pixels
//! perceptually instead. 100 = identical; rendering regressions (moved text,
//! wrong font, missing element) score far lower than encoder-level
//! differences.
//!
//! Usage: snapcmp <expected> <actual> --min-score <score>
//! Prints the score; exits 0 if score >= min-score, 1 otherwise.

use std::process::ExitCode;
use zensim::{RgbaSlice, Zensim, ZensimProfile};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (a, b, min_score) = match args.as_slice() {
        [_, a, b, flag, min] if flag == "--min-score" => {
            let min: f64 = min.parse().expect("min-score must be a number");
            (a, b, min)
        }
        _ => {
            eprintln!("usage: snapcmp <expected> <actual> --min-score <score>");
            return ExitCode::from(2);
        }
    };

    let (pixels_a, wa, ha) = decode(a);
    let (pixels_b, wb, hb) = decode(b);

    if (wa, ha) != (wb, hb) {
        println!("dimensions differ: {wa}x{ha} vs {wb}x{hb}");
        return ExitCode::FAILURE;
    }

    let z = Zensim::new(ZensimProfile::latest());
    let result = z
        .compute(
            &RgbaSlice::new(&pixels_a, wa, ha),
            &RgbaSlice::new(&pixels_b, wb, hb),
        )
        .expect("zensim compute failed");

    let score = result.score();
    println!("{score:.2}");
    if score >= min_score {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Decode an image file (PNG/JPEG/WebP) to RGBA pixels.
fn decode(path: &str) -> (Vec<[u8; 4]>, usize, usize) {
    let image = image::ImageReader::open(path)
        .unwrap_or_else(|e| panic!("failed to open {path}: {e}"))
        .decode()
        .unwrap_or_else(|e| panic!("failed to decode {path}: {e}"))
        .into_rgba8();
    let (width, height) = (image.width() as usize, image.height() as usize);
    let pixels = image
        .into_raw()
        .chunks_exact(4)
        .map(|px| [px[0], px[1], px[2], px[3]])
        .collect();
    (pixels, width, height)
}
