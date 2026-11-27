//! Phase 5 acceptance tests: editing Bangla and English text (SRS Module 6).
//!
//! After every edit the page text is read back and compared as Unicode, so
//! broken conjuncts, reordered vowel signs or lost characters fail the test.

use std::path::{Path, PathBuf};

use akkhor_pdf_editor_lib::engine::{
    extract::{document_text, page_lines},
    fonts::{bangla_faces, default_bangla, dev_fonts_dir, english_face, SANS, SERIF},
    layout::{lines_to_pdf, Align, LayoutOptions, Style},
    textedit::{wrap, Fit, Markup, TextBlock, TextPlacement},
    viewer::{DocInfo, PageEdit, Viewer},
};
use mupdf::pdf::PdfDocument;

const L1: &str = "আমি বাংলায় গান গাই।";
const L2: &str = "Invoice No. 2024 — মোট ১২০০ টাকা";
const L3: &str = "ক্ষ্ম বিজ্ঞান কৌতূহল";

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn work_dir(test: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-output").join("phase5").join(test);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sample(dir: &Path) -> PathBuf {
    let path = dir.join("sample.pdf");
    let pdf = lines_to_pdf(&[L1, L2, L3], &LayoutOptions::a4(default_bangla()), &dev_fonts_dir()).unwrap();
    std::fs::write(&path, pdf).unwrap();
    path
}

fn style(size: f32, english: &str) -> Style {
    Style {
        bangla: bangla_faces("Noto Sans Bengali", &dev_fonts_dir()),
        english: english_face(english),
        size,
        line_height: size * 1.3,
        bold: false,
        italic: false,
        color: [0, 0, 0],
        align: Align::Left,
    }
}

fn block_with(viewer: &Viewer, info: &DocInfo, words: &str) -> TextBlock {
    viewer
        .text_blocks(info.id, 0)
        .unwrap()
        .into_iter()
        .find(|b| squash(&b.text).contains(&squash(words)))
        .unwrap_or_else(|| panic!("no block with {words:?}"))
}

fn replace(block: &TextBlock, fit: Fit) -> TextPlacement {
    TextPlacement {
        left: block.origin[0],
        width: block.max_width,
        baseline: Some(block.origin[1]),
        top: None,
        max_height: Some(block.rect[3] - block.rect[1]),
        remove: block.lines.clone(),
        fit,
    }
}

fn page_text(viewer: &Viewer, info: &DocInfo) -> Vec<String> {
    viewer.text_lines(info.id, 0).unwrap().into_iter().map(|l| squash(&l.text)).collect()
}

#[test]
fn blocks_report_text_font_and_position() {
    let dir = work_dir("blocks");
    let viewer = Viewer::new();
    let info = viewer.open(&sample(&dir), None).unwrap();
    let blocks = viewer.text_blocks(info.id, 0).unwrap();
    assert_eq!(blocks.len(), 3, "{blocks:#?}");
    assert_eq!(squash(&blocks[0].text), squash(L1));
    assert_eq!(squash(&blocks[2].text), squash(L3));
    let b = &blocks[1];
    assert!(b.has_bangla && b.readable);
    assert!((b.size - 12.0).abs() < 0.6, "size {}", b.size);
    assert_eq!(b.color, "#000000");
    assert!((b.rect[0] - b.origin[0]).abs() < 2.0, "left edge {:?} vs origin {:?}", b.rect, b.origin);
    assert!(b.max_width > 400.0, "a single line may grow to the right margin: {}", b.max_width);
    assert!(blocks[0].origin[1] < blocks[1].origin[1] && blocks[1].origin[1] < blocks[2].origin[1]);
}

#[test]
fn editing_bangla_keeps_conjuncts_position_and_other_lines() {
    let dir = work_dir("edit");
    let viewer = Viewer::new();
    let path = sample(&dir);
    let info = viewer.open(&path, None).unwrap();
    let block = block_with(&viewer, &info, L1);
    let new = "আমরা বিজ্ঞান ও ক্ষমতা শিখি।";

    let info = viewer
        .edit(info.id, PageEdit::PlaceText { page: 0, text: new.into(), style: style(12.0, SANS), placement: replace(&block, Fit::Overflow) })
        .unwrap();
    let text = page_text(&viewer, &info);
    assert!(text.contains(&squash(new)), "new text must read back exactly: {text:?}");
    assert!(!text.iter().any(|l| l.contains(&squash("গান গাই"))), "old text must be gone: {text:?}");
    assert!(text.contains(&squash(L2)) && text.contains(&squash(L3)), "other lines stay: {text:?}");

    // Same baseline and left edge as the old line.
    let edited = block_with(&viewer, &info, new);
    assert!((edited.origin[1] - block.origin[1]).abs() < 1.5, "baseline {} vs {}", edited.origin[1], block.origin[1]);
    assert!((edited.origin[0] - block.origin[0]).abs() < 2.5, "left {} vs {}", edited.origin[0], block.origin[0]);

    // Saved, reopened and read by plain text extraction (copy / search).
    let out = dir.join("edited.pdf");
    viewer.save(info.id, Some(out.clone())).unwrap();
    let all = squash(&document_text(&std::fs::read(&out).unwrap()).unwrap());
    assert!(all.contains(&squash(new)) && all.contains(&squash(L3)), "{all}");

    // Edit the edited text again: the first edit's text is removed too.
    let again = block_with(&viewer, &info, new);
    let newer = "দ্বিতীয়বার বদল — second edit";
    let info = viewer
        .edit(info.id, PageEdit::PlaceText { page: 0, text: newer.into(), style: style(12.0, SANS), placement: replace(&again, Fit::Overflow) })
        .unwrap();
    let text = page_text(&viewer, &info);
    assert!(text.contains(&squash(newer)), "{text:?}");
    assert!(!text.iter().any(|l| l.contains(&squash("ক্ষমতা"))), "the first edit is gone: {text:?}");
    assert_eq!(text.len(), 3, "{text:?}");

    // Undo twice puts the original text back.
    viewer.undo(info.id).unwrap();
    let info = viewer.undo(info.id).unwrap();
    assert!(page_text(&viewer, &info).contains(&squash(L1)));
}

#[test]
fn mixed_line_uses_chosen_english_font_and_colour() {
    let dir = work_dir("style");
    let viewer = Viewer::new();
    let info = viewer.open(&sample(&dir), None).unwrap();
    let block = block_with(&viewer, &info, L2);
    let mut st = style(14.0, SERIF);
    st.bold = true;
    st.color = [200, 0, 0];
    let new = "Total মোট 1500 টাকা";
    let info = viewer
        .edit(info.id, PageEdit::PlaceText { page: 0, text: new.into(), style: st, placement: replace(&block, Fit::Overflow) })
        .unwrap();
    assert!(page_text(&viewer, &info).contains(&squash(new)));

    let out = dir.join("styled.pdf");
    viewer.save(info.id, Some(out.clone())).unwrap();
    let doc = PdfDocument::open(out.to_str().unwrap()).unwrap();
    let line = page_lines(&doc, 0).unwrap().into_iter().find(|l| squash(&l.text).contains("Total")).unwrap();
    let name = line.font.name.to_lowercase();
    assert!(name.contains("times") || name.contains("roman"), "English part should be a Times-width serif, got {name}");
    assert!(line.font.bold, "bold");
    assert_eq!(line.argb & 0xFFFFFF, 0xC80000, "colour");
}

#[test]
fn long_text_wraps_inside_its_box_or_asks() {
    let dir = work_dir("wrap");
    let viewer = Viewer::new();
    let info = viewer.open(&sample(&dir), None).unwrap();
    let block = block_with(&viewer, &info, L3);
    let long = "বাংলা ও English লেখা একসাথে লিখলে লাইন ভাঙবে ঠিক শব্দের শেষে, যুক্তাক্ষর ক্ষ্ম ভাঙবে না";

    // A narrow box: the old line's own width.
    let block = TextBlock { max_width: block.rect[2] - block.origin[0] + 4.0, ..block };
    // Check first: it does not fit in one line, so the editor is told.
    let err = viewer
        .edit(info.id, PageEdit::PlaceText { page: 0, text: long.into(), style: style(12.0, SANS), placement: replace(&block, Fit::Check) })
        .unwrap_err()
        .to_string();
    assert!(err.starts_with("TEXT_TOO_LONG:"), "{err}");
    assert!(page_text(&viewer, &info).contains(&squash(L3)), "a refused edit changes nothing");

    // Let it run on: it wraps at word ends inside the box's width.
    let place = replace(&block, Fit::Overflow);
    let right = place.left + place.width;
    let info = viewer
        .edit(info.id, PageEdit::PlaceText { page: 0, text: long.into(), style: style(12.0, SANS), placement: place })
        .unwrap();
    let lines = viewer.text_lines(info.id, 0).unwrap();
    let new_lines: Vec<_> = lines.iter().filter(|l| l.rect[1] >= block.rect[1] - 2.0).collect();
    assert!(new_lines.len() >= 2, "should wrap: {new_lines:#?}");
    for l in &new_lines {
        assert!(l.rect[2] <= right + 1.5, "line runs past the box ({} > {right}): {}", l.rect[2], l.text);
    }
    let joined: String = new_lines.iter().map(|l| squash(&l.text)).collect();
    assert_eq!(joined, squash(long), "every word, in order, nothing broken");

    // Shrink to fit: smaller text, inside the old height.
    let info = viewer.undo(info.id).unwrap();
    let info = viewer
        .edit(info.id, PageEdit::PlaceText { page: 0, text: long.into(), style: style(12.0, SANS), placement: replace(&block, Fit::Shrink) })
        .unwrap();
    let lines = viewer.text_lines(info.id, 0).unwrap();
    let shrunk: Vec<_> = lines.iter().filter(|l| l.rect[1] >= block.rect[1] - 2.0).collect();
    assert!(shrunk.iter().all(|l| l.size < 11.5), "text should be smaller: {shrunk:#?}");
    assert!(shrunk.iter().all(|l| l.rect[3] <= block.rect[3] + 6.0), "and stay in the box: {shrunk:#?}");
}

#[test]
fn wrap_breaks_only_between_words() {
    let st = style(12.0, SANS);
    let text = "এক দুই তিন চার পাঁচ ছয় সাত আট নয় দশ\n\nএগারো বারো";
    let lines = wrap(text, &st, 80.0).unwrap();
    assert!(lines.len() >= 4, "{lines:?}");
    assert!(lines.contains(&String::new()), "the blank line between paragraphs stays");
    let words: Vec<&str> = lines.iter().flat_map(|l| l.split_whitespace()).collect();
    assert_eq!(words, text.split_whitespace().collect::<Vec<_>>());
}

#[test]
fn new_text_box_anywhere() {
    let dir = work_dir("add");
    let viewer = Viewer::new();
    let info = viewer.open(&sample(&dir), None).unwrap();
    let text = "নতুন লেখা — New text box";
    let place = TextPlacement { left: 100.0, width: 300.0, baseline: None, top: Some(400.0), max_height: None, remove: vec![], fit: Fit::Overflow };
    let info = viewer.edit(info.id, PageEdit::PlaceText { page: 0, text: text.into(), style: style(16.0, SANS), placement: place }).unwrap();
    let lines = viewer.text_lines(info.id, 0).unwrap();
    let added = lines
        .iter()
        .find(|l| squash(&l.text) == squash(text))
        .unwrap_or_else(|| panic!("the new text should read back exactly: {lines:#?}"));
    assert!(added.rect[0] >= 99.0 && added.rect[0] < 106.0, "{:?}", added.rect);
    assert!(added.rect[1] >= 400.0 && added.rect[3] < 430.0, "{:?}", added.rect);
    assert_eq!(lines.len(), 4, "the old lines are untouched");

    // An empty new box is refused.
    let place = TextPlacement { left: 10.0, width: 100.0, baseline: None, top: Some(10.0), max_height: None, remove: vec![], fit: Fit::Overflow };
    assert!(viewer.edit(info.id, PageEdit::PlaceText { page: 0, text: "  ".into(), style: style(12.0, SANS), placement: place }).is_err());
}

#[test]
fn highlight_underline_and_strike() {
    let dir = work_dir("markup");
    let viewer = Viewer::new();
    let path = sample(&dir);
    let info = viewer.open(&path, None).unwrap();
    let b = block_with(&viewer, &info, L1);
    let id = info.id;
    viewer.edit(id, PageEdit::Markup { page: 0, markup: Markup::Highlight, rect: b.rect, color: [255, 230, 0] }).unwrap();
    let b2 = block_with(&viewer, &info, L2);
    viewer.edit(id, PageEdit::Markup { page: 0, markup: Markup::Underline, rect: b2.rect, color: [0, 0, 255] }).unwrap();
    viewer.edit(id, PageEdit::Markup { page: 0, markup: Markup::Strike, rect: block_with(&viewer, &info, L3).rect, color: [255, 0, 0] }).unwrap();
    let err = viewer
        .edit(id, PageEdit::Markup { page: 0, markup: Markup::Highlight, rect: [300.0, 600.0, 400.0, 700.0], color: [255, 230, 0] })
        .unwrap_err();
    assert!(err.to_string().contains("NO_TEXT_THERE"));

    let out = dir.join("marked.pdf");
    viewer.save(id, Some(out.clone())).unwrap();
    let doc = PdfDocument::open(out.to_str().unwrap()).unwrap();
    let page = doc.load_pdf_page(0).unwrap();
    let kinds: Vec<String> = page.annotations().map(|a| format!("{:?}", a.r#type().unwrap())).collect();
    assert_eq!(kinds, ["Highlight", "Underline", "StrikeOut"], "{kinds:?}");
    // Text is unchanged by marking it.
    let all = squash(&document_text(&std::fs::read(&out).unwrap()).unwrap());
    assert!(all.contains(&squash(L1)) && all.contains(&squash(L2)));
}

#[test]
fn redaction_removes_text_for_good() {
    let dir = work_dir("redact");
    let viewer = Viewer::new();
    let info = viewer.open(&sample(&dir), None).unwrap();
    let b = block_with(&viewer, &info, L2);
    let rect = [b.rect[0] - 2.0, b.rect[1] - 2.0, b.rect[2] + 2.0, b.rect[3] + 2.0];
    let info = viewer.edit(info.id, PageEdit::Redact { page: 0, rect }).unwrap();

    let text = page_text(&viewer, &info);
    assert!(!text.iter().any(|l| l.contains("Invoice")), "{text:?}");
    assert!(text.contains(&squash(L1)) && text.contains(&squash(L3)), "{text:?}");
    // A black box covers the area.
    let rgba = viewer.render_rgba(info.id, 0, 1.0).unwrap();
    let w = u32::from_le_bytes(rgba[0..4].try_into().unwrap()) as usize;
    let (x, y) = (((b.rect[0] + b.rect[2]) / 2.0) as usize, ((b.rect[1] + b.rect[3]) / 2.0) as usize);
    let i = 8 + (y * w + x) * 4;
    assert!(rgba[i] < 40 && rgba[i + 1] < 40 && rgba[i + 2] < 40, "black box expected");

    // The saved file does not contain the removed words at all.
    let out = dir.join("redacted.pdf");
    viewer.save(info.id, Some(out.clone())).unwrap();
    let all = document_text(&std::fs::read(&out).unwrap()).unwrap();
    assert!(!all.contains("Invoice") && !all.contains("2024"), "{all}");
}
