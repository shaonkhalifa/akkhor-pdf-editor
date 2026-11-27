//! Phase 2 acceptance tests: the viewer engine (SRS Modules 1 and 2).
//!
//! - A 100-page PDF opens with its first page drawn in under 1 second
//!   (NFR-1.2) and a page renders at 100% in under 100 ms (NFR-1.3).
//! - Search finds Bangla words on every page (FR-2.5).
//! - Text comes back as correct Unicode for copying (FR-2.6).
//! - Password-protected files need the right password (FR-1.5).
//! - A damaged file gives an error instead of crashing (FR-1.8).

use std::path::{Path, PathBuf};
use std::time::Instant;

use akkhor_pdf_editor_lib::engine::{
    fonts::{default_bangla, dev_fonts_dir},
    layout::{lines_to_pdf, LayoutOptions},
    viewer::{OpenError, Viewer},
};
use mupdf::pdf::{Encryption, PdfDocument, PdfWriteOptions};

/// The SRS limits apply to the optimised app. Debug test builds are much
/// slower, so they get 3x the time; CI also runs these tests in release mode.
const SLOWDOWN: u128 = if cfg!(debug_assertions) { 3 } else { 1 };

const LINE: &str = "পাতা — আমি বাংলায় গান গাই। Invoice ক্ষ্ম বিজ্ঞান";

fn work_dir() -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-output").join("phase2");
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A 100-page PDF: one Bangla page duplicated 99 times. Each test gets its
/// own file because tests run at the same time.
fn hundred_page_pdf(name: &str) -> PathBuf {
    let path = work_dir().join(format!("hundred-{name}.pdf"));
    let lines: Vec<&str> = std::iter::repeat_n(LINE, 30).collect();
    let one = lines_to_pdf(&lines, &LayoutOptions::a4(default_bangla()), &dev_fonts_dir()).unwrap();
    let mut doc = PdfDocument::from_bytes(&one).unwrap();
    for _ in 1..100 {
        doc.duplicate_page(0).unwrap();
    }
    doc.save(path.to_str().unwrap()).unwrap();
    path
}

#[test]
fn hundred_page_pdf_opens_fast_and_renders_fast() {
    let path = hundred_page_pdf("speed");
    let viewer = Viewer::new();

    let started = Instant::now();
    let info = viewer.open(&path, None).unwrap();
    let first = viewer.render_rgba(info.id, 0, 96.0 / 72.0).unwrap();
    let open_and_first_page = started.elapsed();

    assert_eq!(info.page_count, 100);
    assert_eq!(info.pages.len(), 100);
    let (w, h) = (
        u32::from_le_bytes(first[0..4].try_into().unwrap()),
        u32::from_le_bytes(first[4..8].try_into().unwrap()),
    );
    assert_eq!(first.len(), 8 + (w * h * 4) as usize);
    assert!((790..=800).contains(&w), "A4 at 100% should be ~794 px wide, got {w}");
    assert!(
        open_and_first_page.as_millis() < 1000 * SLOWDOWN,
        "NFR-1.2: open + first page took {open_and_first_page:?} (limit 1 s)"
    );

    // NFR-1.3: render a page at 100% zoom (including recording it) in < 100 ms.
    let started = Instant::now();
    viewer.render_rgba(info.id, 57, 96.0 / 72.0).unwrap();
    let one_page = started.elapsed();
    assert!(
        one_page.as_millis() < 100 * SLOWDOWN,
        "NFR-1.3: page render took {one_page:?} (limit {} ms)",
        100 * SLOWDOWN
    );
    println!("open + first page: {open_and_first_page:?}, one page at 100%: {one_page:?}");
}

#[test]
fn search_finds_bangla_on_every_page_and_text_is_correct_unicode() {
    let path = hundred_page_pdf("search");
    let viewer = Viewer::new();
    let info = viewer.open(&path, None).unwrap();

    let hits = viewer.search(info.id, info.page_count, "বিজ্ঞান").unwrap();
    let pages: std::collections::BTreeSet<i32> = hits.iter().map(|h| h.page).collect();
    assert_eq!(pages.len(), 100, "every page should contain the word; got hits on {} pages", pages.len());
    for h in &hits {
        let [x0, y0, x1, y1] = h.rect;
        assert!(x1 > x0 && y1 > y0, "hit has an empty box: {:?}", h.rect);
        // Regression: highlights once covered only the last letter. At 12 pt
        // "বিজ্ঞান" is about 35 pt wide; a box under 20 pt misses most of it.
        assert!(x1 - x0 > 20.0, "highlight too narrow to cover the word: {:?}", h.rect);
    }

    let lines = viewer.text_lines(info.id, 3).unwrap();
    assert_eq!(lines.len(), 30);
    let squash = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    assert_eq!(squash(&lines[0].text), squash(LINE));
}

#[test]
fn password_protected_pdf_needs_the_right_password() {
    let plain = lines_to_pdf(&[LINE], &LayoutOptions::a4(default_bangla()), &dev_fonts_dir()).unwrap();
    let doc = PdfDocument::from_bytes(&plain).unwrap();
    let path = work_dir().join("locked.pdf");
    let mut opts = PdfWriteOptions::default();
    opts.set_encryption(Encryption::Aes256).set_user_password("গোপন123").set_owner_password("owner");
    doc.save_with_options(path.to_str().unwrap(), opts).unwrap();

    let viewer = Viewer::new();
    assert!(matches!(viewer.open(&path, None), Err(OpenError::NeedsPassword)));
    assert!(matches!(viewer.open(&path, Some("wrong".into())), Err(OpenError::WrongPassword)));
    let info = viewer.open(&path, Some("গোপন123".into())).unwrap();
    assert_eq!(info.page_count, 1);
}

#[test]
fn damaged_file_gives_an_error_not_a_crash() {
    let path = work_dir().join("damaged.pdf");
    std::fs::write(&path, b"%PDF-1.7\nthis is not really a PDF \x00\xff\x13").unwrap();
    let viewer = Viewer::new();
    match viewer.open(&path, None) {
        Err(OpenError::Failed(_)) => {}
        Ok(info) => assert_eq!(info.page_count, 0, "a broken file must not show pages"),
        Err(other) => panic!("unexpected error: {other}"),
    }
    // The engine thread must still work after the failure.
    let good = hundred_page_pdf("after-damage");
    assert_eq!(viewer.open(&good, None).unwrap().page_count, 100);
}
