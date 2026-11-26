//! Phase 1 acceptance tests (SRS 6.4 and the text-edit spike).
//!
//! - Every bundled Bangla font must produce a PDF whose text reads back as
//!   exactly the same Unicode as the input.
//! - Page renders must match the reference images in `tests/reference/`.
//!   To create or refresh them after checking the output by eye, run:
//!   `AKKHOR_UPDATE_REFS=1 cargo test --test phase1`
//! - The spike must replace Bangla words in an existing PDF and survive a
//!   save and reopen.
//!
//! Generated PDFs and PNGs are written to `test-output/` for inspection.

use std::path::{Path, PathBuf};

use akkhor_pdf_editor_lib::engine::{
    fonts::{default_bangla, dev_fonts_dir, BANGLA_FONTS},
    render::{page_png, page_rgb},
    selftest::{check_font, run_edit_spike},
};

const ZOOM: f32 = 1.5;
/// Share of pixels allowed to differ (anti-aliasing changes between builds).
const MAX_DIFF_RATIO: f64 = 0.005;
/// A pixel "differs" when any channel moves by more than this.
const CHANNEL_TOLERANCE: u8 = 48;

fn out_dir() -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-output");
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn ref_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("reference")
}

fn slug(name: &str) -> String {
    name.to_lowercase().replace(' ', "-")
}

fn decode_png(path: &Path) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgb, "reference must be RGB");
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

/// Compare a rendered page with its reference image, or create the reference.
fn check_against_reference(pdf: &[u8], name: &str) {
    let png_bytes = page_png(pdf, 0, ZOOM).unwrap();
    std::fs::write(out_dir().join(format!("{name}.png")), &png_bytes).unwrap();
    std::fs::write(out_dir().join(format!("{name}.pdf")), pdf).unwrap();

    let ref_path = ref_dir().join(format!("{name}.png"));
    if std::env::var_os("AKKHOR_UPDATE_REFS").is_some() {
        std::fs::create_dir_all(ref_dir()).unwrap();
        std::fs::write(&ref_path, &png_bytes).unwrap();
        return;
    }
    assert!(
        ref_path.exists(),
        "missing reference image {}; run with AKKHOR_UPDATE_REFS=1 after checking test-output/{name}.png",
        ref_path.display()
    );

    let (w, h, actual) = page_rgb(pdf, 0, ZOOM).unwrap();
    let (rw, rh, expected) = decode_png(&ref_path);
    assert_eq!((w, h), (rw, rh), "{name}: image size changed");
    let differing = actual
        .chunks_exact(3)
        .zip(expected.chunks_exact(3))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > CHANNEL_TOLERANCE))
        .count();
    let ratio = differing as f64 / (w as f64 * h as f64);
    assert!(
        ratio <= MAX_DIFF_RATIO,
        "{name}: {:.3}% of pixels differ from the reference (limit {:.3}%)",
        ratio * 100.0,
        MAX_DIFF_RATIO * 100.0
    );
}

#[test]
fn shaping_text_round_trips_for_every_bundled_font() {
    let mut failures = Vec::new();
    for font in BANGLA_FONTS {
        let r = check_font(*font, &dev_fonts_dir()).unwrap();
        if !r.passed {
            failures.push(format!(
                "{}:\n  expected: {:?}\n  got:      {:?}",
                r.font, r.expected, r.extracted
            ));
        }
    }
    assert!(failures.is_empty(), "text did not round-trip:\n{}", failures.join("\n"));
}

#[test]
fn shaping_renders_match_reference_images() {
    for font in BANGLA_FONTS {
        let r = check_font(*font, &dev_fonts_dir()).unwrap();
        check_against_reference(&r.pdf, &format!("shaping-{}", slug(font.name)));
    }
}

#[test]
fn edit_spike_replaces_bangla_text_and_survives_reopen() {
    let r = run_edit_spike(default_bangla(), &dev_fonts_dir()).unwrap();
    // Save the output first, so a failure can be inspected by eye.
    std::fs::write(out_dir().join("spike-before.pdf"), &r.before_pdf).unwrap();
    std::fs::write(out_dir().join("spike-after.pdf"), &r.after_pdf).unwrap();
    std::fs::write(out_dir().join("spike-after.png"), page_png(&r.after_pdf, 0, ZOOM).unwrap()).unwrap();
    assert!(
        r.passed,
        "spike failed.\nbefore: {:?}\nafter:  {:?}",
        r.before_text, r.after_text
    );
    check_against_reference(&r.after_pdf, "spike-after");
    std::fs::write(out_dir().join("spike-before.pdf"), &r.before_pdf).unwrap();
}
