//! Phase 7 acceptance tests: Bijoy to Unicode (SRS Module 9) and OCR
//! (Module 10).

use std::path::{Path, PathBuf};

use akkhor_pdf_editor_lib::engine::{
    bijoy, convert, ocr,
    extract::document_text,
    fonts::{bangla_faces, default_bangla, dev_fonts_dir, english_face, SANS},
    layout::{lines_to_pdf, Align, LayoutOptions, Style},
    viewer::{PageEdit, Viewer},
};
use mupdf::pdf::{PdfDocument, PdfObject};

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn work_dir(test: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-output").join("phase7").join(test);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn style() -> Style {
    Style {
        bangla: bangla_faces("Noto Sans Bengali", &dev_fonts_dir()),
        english: english_face(SANS),
        size: 12.0,
        line_height: 15.0,
        bold: false,
        italic: false,
        color: [0, 0, 0],
        align: Align::Left,
    }
}

/// A PDF that looks like an old Bijoy document: Latin-coded Bangla in a
/// font named SutonnyMJ, plus one ordinary Unicode line.
fn bijoy_pdf(path: &Path, bijoy_lines: &[&str], unicode_line: &str) {
    let mut lines: Vec<&str> = bijoy_lines.to_vec();
    lines.push(unicode_line);
    let pdf = lines_to_pdf(&lines, &LayoutOptions::a4(default_bangla()), &dev_fonts_dir()).unwrap();
    let doc = PdfDocument::from_bytes(&pdf).unwrap();
    // The Latin-only lines use the built-in sans font; rename it SutonnyMJ.
    let page = doc.load_pdf_page(0).unwrap();
    let fonts = page.resources().unwrap().get_dict("Font").unwrap().unwrap();
    for i in 0..fonts.dict_len().unwrap() {
        let mut f = fonts.get_dict_val(i as i32).unwrap().unwrap();
        let base = f.get_dict("BaseFont").unwrap().map(|b| String::from_utf8_lossy(&b.as_name().unwrap()).into_owned()).unwrap_or_default();
        if base.contains("Bengali") {
            continue;
        }
        let name = PdfObject::new_name("SutonnyMJ").unwrap();
        f.dict_put("BaseFont", name.clone()).unwrap();
        // The name also lives in the descendant font and font descriptor.
        let mut targets = vec![f.clone()];
        if let Some(d) = f.get_dict("DescendantFonts").unwrap() {
            if let Some(mut df) = d.get_array(0).unwrap() {
                df.dict_put("BaseFont", name.clone()).unwrap();
                targets.push(df);
            }
        }
        for t in targets {
            if let Some(mut fd) = t.get_dict("FontDescriptor").unwrap() {
                fd.dict_put("FontName", name.clone()).unwrap();
            }
        }
    }
    doc.save(path.to_str().unwrap()).unwrap();
}

#[test]
fn bijoy_document_converts_to_unicode() {
    let dir = work_dir("bijoy");
    let path = dir.join("bijoy.pdf");
    let unicode = "এটি ইউনিকোড লেখা";
    bijoy_pdf(&path, &["Avwg evsjvq Mvb MvB", "evsjv‡`‡ki ¯^vaxbZv"], unicode);

    let viewer = Viewer::new();
    let info = viewer.open(&path, None).unwrap();
    let blocks = viewer.text_blocks(info.id, 0).unwrap();
    let flags: Vec<(String, bool)> = blocks.iter().map(|b| (b.text.clone(), b.bijoy)).collect();
    assert_eq!(flags.iter().filter(|(_, b)| *b).count(), 2, "both Bijoy lines are found by their font: {flags:?}");
    assert!(flags.iter().any(|(t, b)| squash(t) == squash(unicode) && !b), "the Unicode line is not Bijoy");

    let info = viewer.edit(info.id, PageEdit::ConvertBijoy { pages: vec![], style: style() }).unwrap();
    let text: Vec<String> = viewer.text_lines(info.id, 0).unwrap().into_iter().map(|l| squash(&l.text)).collect();
    assert!(text.contains(&squash("আমি বাংলায় গান গাই")), "{text:?}");
    assert!(text.contains(&squash("বাংলাদেশের স্বাধীনতা")), "{text:?}");
    assert!(text.contains(&squash(unicode)), "the Unicode line is untouched: {text:?}");
    assert!(!text.iter().any(|t| t.contains("Avwg")), "{text:?}");

    // Searchable after saving.
    let out = dir.join("converted.pdf");
    viewer.save(info.id, Some(out.clone())).unwrap();
    let all = squash(&document_text(&std::fs::read(&out).unwrap()).unwrap());
    assert!(all.contains(&squash("বাংলাদেশের")), "{all}");

    // Nothing left to convert; undo brings the Bijoy text back.
    let err = viewer.edit(info.id, PageEdit::ConvertBijoy { pages: vec![], style: style() }).unwrap_err();
    assert!(err.to_string().contains("NO_BIJOY_TEXT"));
    let info = viewer.undo(info.id).unwrap();
    assert!(viewer.text_lines(info.id, 0).unwrap().iter().any(|l| l.text.contains("Avwg")));
}

#[test]
fn pasted_bijoy_text_is_recognised_and_converted() {
    let pasted = "Avgv‡`i †`k evsjv‡`k";
    assert!(bijoy::looks_like_bijoy(pasted));
    assert_eq!(bijoy::to_unicode(pasted), "আমাদের দেশ বাংলাদেশ");
    assert!(!bijoy::looks_like_bijoy("Our country Bangladesh"));
    assert!(!bijoy::looks_like_bijoy("আমাদের দেশ"));
}


/// Language data for OCR tests: AKKHOR_TESSDATA, or the local copy. The
/// OCR tests are skipped (with a note) when it is not there.
fn tessdata() -> Option<PathBuf> {
    let dir = std::env::var_os("AKKHOR_TESSDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"D:\dev\akkhor-tessdata\best"));
    if dir.join("eng.traineddata").exists() && dir.join("ben.traineddata").exists() {
        Some(dir)
    } else {
        eprintln!("OCR test skipped: no language data at {}", dir.display());
        None
    }
}

/// A "scanned" page: Bangla and English text drawn as a picture only.
fn scanned_pdf(path: &Path, lines: &[&str]) {
    let mut opts = LayoutOptions::a4(default_bangla());
    opts.font_size = 20.0;
    let text_pdf = lines_to_pdf(lines, &opts, &dev_fonts_dir()).unwrap();
    let viewer = Viewer::new();
    let tmp = path.with_extension("text.pdf");
    std::fs::write(&tmp, &text_pdf).unwrap();
    let info = viewer.open(&tmp, None).unwrap();
    let png = convert::page_picture(&viewer.page(info.id, 0).unwrap(), 200.0, convert::PictureFormat::Png).unwrap();
    let pic = path.with_extension("png");
    std::fs::write(&pic, png).unwrap();
    std::fs::write(path, convert::pictures_to_pdf(&[pic], convert::PictureLayout::A4).unwrap()).unwrap();
}

#[test]
fn ocr_makes_a_scanned_page_searchable() {
    let Some(data) = tessdata() else { return };
    let dir = work_dir("ocr");
    let path = dir.join("scan.pdf");
    scanned_pdf(&path, &["Invoice Number 2024", "আমার সোনার বাংলা"]);
    let viewer = Viewer::new();
    let info = viewer.open(&path, None).unwrap();
    assert!(viewer.text_lines(info.id, 0).unwrap().is_empty(), "a scan has no text to start with");

    // Read one area.
    let b = ocr::status(&[data.clone()]);
    assert_eq!(b.languages, ["ben", "eng"]);
    let words = viewer.ocr_area(info.id, 0, [0.0, 0.0, 595.0, 420.0], &data).unwrap();
    assert!(words.contains("Invoice") && words.contains("2024"), "{words}");

    // Make the whole page searchable.
    let info = viewer.edit(info.id, PageEdit::Ocr { page: 0, datadir: data.clone(), dpi: 200 }).unwrap();
    assert_eq!(info.page_count, 1);
    let text: String = viewer.text_lines(info.id, 0).unwrap().iter().map(|l| squash(&l.text)).collect();
    assert!(text.contains("Invoice") && text.contains("Number"), "English found: {text}");
    assert!(text.contains("বাংলা") || text.contains("সোনার"), "Bangla found: {text}");
    let hits = viewer.search(info.id, 1, "invoice").unwrap();
    assert_eq!(hits.len(), 1, "searchable");
    // Looks the same: still a picture with dark text at the top.
    let out = dir.join("searchable.pdf");
    viewer.save(info.id, Some(out.clone())).unwrap();
    assert!(squash(&document_text(&std::fs::read(&out).unwrap()).unwrap()).contains("Invoice"));

    // Without language data, a clear message.
    let empty = dir.join("no-data");
    std::fs::create_dir_all(&empty).unwrap();
    let err = viewer.ocr_area(info.id, 0, [0.0, 0.0, 100.0, 100.0], &empty).unwrap_err();
    assert!(err.to_string().contains("OCR_DATA_MISSING"), "{err}");
}
