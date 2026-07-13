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
/// JPEG goes through zune-jpeg 0.5 directly: the `image` crate's bundled
/// zune-jpeg 0.4 rejects zenjpeg's (our encoder's) output with "Marker SOS
/// found in bitstream".
fn decode(path: &str) -> (Vec<[u8; 4]>, usize, usize) {
    if path.to_lowercase().ends_with(".jpg") || path.to_lowercase().ends_with(".jpeg") {
        return decode_jpeg(path);
    }
    let image = image::ImageReader::open(path)
        .unwrap_or_else(|e| panic!("failed to open {path}: {e}"))
        .decode()
        .unwrap_or_else(|e| panic!("failed to decode {path}: {e}"))
        .into_rgba8();
    let (width, height) = (image.width() as usize, image.height() as usize);
    let pixels = rgba_pixels(image.into_raw(), 4);
    (pixels, width, height)
}

fn decode_jpeg(path: &str) -> (Vec<[u8; 4]>, usize, usize) {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;

    let data = std::fs::read(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
    let mut decoder = JpegDecoder::new_with_options(std::io::Cursor::new(data), options);
    let pixels = decoder
        .decode()
        .unwrap_or_else(|e| panic!("failed to decode {path}: {e:?}"));
    let (width, height) = decoder.dimensions().expect("no jpeg dimensions");
    (rgba_pixels(pixels, 3), width, height)
}

fn rgba_pixels(data: Vec<u8>, channels: usize) -> Vec<[u8; 4]> {
    data.chunks_exact(channels)
        .map(|px| [px[0], px[1], px[2], if channels == 4 { px[3] } else { 255 }])
        .collect()
}
