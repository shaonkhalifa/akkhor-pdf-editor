//! Phase 3 acceptance tests: page tools (SRS Module 3).
//!
//! Every page carries its own Bangla label ("পাতা 7 বিজ্ঞান"), so after each
//! change the tests read the text back and check the exact page order. That
//! also proves Bangla text survives rotating, moving, merging and saving.

use std::path::{Path, PathBuf};
use std::time::Instant;

use akkhor_pdf_editor_lib::engine::{
    fonts::{default_bangla, dev_fonts_dir},
    layout::{lines_to_pdf, LayoutOptions},
    pages::{self, MergeInput},
    viewer::{DocInfo, PageEdit, Viewer},
};
use mupdf::document::Location;
use mupdf::link::LinkDestination;
use mupdf::pdf::{Encryption, PdfDocument, PdfWriteOptions};
use mupdf::{DestinationKind, Outline};

const SLOWDOWN: u128 = if cfg!(debug_assertions) { 3 } else { 1 };

fn work_dir(test: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-output").join("phase3").join(test);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn label(n: usize) -> String {
    format!("পাতা {n} বিজ্ঞান")
}

/// A PDF whose page i (zero-based) says "পাতা {first + i} বিজ্ঞান", with a
/// bookmark on every page.
fn labelled_pdf(path: &Path, first: usize, count: usize) {
    let opts = LayoutOptions::a4(default_bangla());
    let mut out = PdfDocument::new();
    for i in 0..count {
        let one = lines_to_pdf(&[label(first + i).as_str()], &opts, &dev_fonts_dir()).unwrap();
        let page = PdfDocument::from_bytes(&one).unwrap();
        pages::insert_document(&mut out, i as i32, &page).unwrap();
    }
    let marks: Vec<Outline> = (0..count)
        .map(|i| Outline {
            title: label(first + i),
            uri: None,
            dest: Some(LinkDestination {
                loc: Location { chapter: 0, page_in_chapter: i as u32, page_number: i as u32 },
                kind: DestinationKind::Fit,
            }),
            down: Vec::new(),
        })
        .collect();
    out.set_outlines(&marks).unwrap();
    out.save(path.to_str().unwrap()).unwrap();
}

/// The label number on every page, read back from the text.
fn order(viewer: &Viewer, info: &DocInfo) -> Vec<usize> {
    (0..info.page_count)
        .map(|p| {
            let lines = viewer.text_lines(info.id, p).unwrap();
            let text = lines.first().map(|l| l.text.clone()).unwrap_or_default();
            assert!(text.contains("পাতা") && text.contains("বিজ্ঞান"), "page {p}: Bangla text damaged: {text:?}");
            text.chars().filter(char::is_ascii_digit).collect::<String>().parse().unwrap()
        })
        .collect()
}

fn file_order(path: &Path) -> Vec<usize> {
    let viewer = Viewer::new();
    let info = viewer.open(path, None).unwrap();
    order(&viewer, &info)
}

#[test]
fn rotate_delete_move_with_undo_redo_and_save() {
    let dir = work_dir("edit");
    let path = dir.join("six.pdf");
    labelled_pdf(&path, 1, 6);
    let viewer = Viewer::new();
    let info = viewer.open(&path, None).unwrap();
    let id = info.id;
    assert!(!info.dirty && !info.can_undo);
    let [w, h] = info.pages[1];

    // Rotate page 2 clockwise: its width and height swap, text is still right.
    let info = viewer.edit(id, PageEdit::Rotate { pages: vec![1], degrees: 90 }).unwrap();
    assert_eq!(info.pages[1], [h, w]);
    assert!(info.dirty && info.can_undo);
    assert_eq!(order(&viewer, &info), [1, 2, 3, 4, 5, 6]);
    let lines = viewer.text_lines(id, 1).unwrap();
    assert_eq!(lines[0].angle, 90, "text on the turned page runs top to bottom");
    // Search finds the word on the turned page with an upright box along the text.
    let hits = viewer.search(id, info.page_count, "বিজ্ঞান").unwrap();
    let hit = hits.iter().find(|h| h.page == 1).expect("a hit on the turned page");
    let [x0, y0, x1, y1] = hit.rect;
    assert!(y1 - y0 > x1 - x0 && y1 - y0 > 20.0, "highlight should run down the page: {:?}", hit.rect);

    // Delete pages 1 and 3.
    let info = viewer.edit(id, PageEdit::Delete { pages: vec![0, 2] }).unwrap();
    assert_eq!(order(&viewer, &info), [2, 4, 5, 6]);

    // Drag the last page to the front.
    let info = viewer.edit(id, PageEdit::Move { pages: vec![3], before: 0 }).unwrap();
    assert_eq!(order(&viewer, &info), [6, 2, 4, 5]);

    // Bookmarks follow their pages; bookmarks of deleted pages are removed.
    let marks: Vec<(String, Option<i32>)> = info.outline.iter().map(|o| (o.title.clone(), o.page)).collect();
    assert_eq!(
        marks,
        [(label(2), Some(1)), (label(4), Some(2)), (label(5), Some(3)), (label(6), Some(0))],
        "bookmark order stays, targets follow the pages"
    );

    // Undo three times: back to the original; redo once.
    let info = viewer.undo(id).unwrap();
    assert_eq!(order(&viewer, &info), [2, 4, 5, 6]);
    viewer.undo(id).unwrap();
    let info = viewer.undo(id).unwrap();
    assert_eq!(order(&viewer, &info), [1, 2, 3, 4, 5, 6]);
    assert_eq!(info.pages[1], [w, h]);
    assert!(!info.dirty, "undoing every change means nothing is unsaved");
    assert!(!info.can_undo && info.can_redo);
    assert!(viewer.undo(id).is_err());
    let info = viewer.redo(id).unwrap();
    assert_eq!(info.pages[1], [h, w]);

    // A new change clears redo.
    let info = viewer.edit(id, PageEdit::Reorder { order: vec![5, 4, 3, 2, 1, 0] }).unwrap();
    assert!(!info.can_redo);
    assert_eq!(order(&viewer, &info), [6, 5, 4, 3, 2, 1]);

    // A PDF must keep one page; the failed change leaves the document as it was.
    let err = viewer.edit(id, PageEdit::Delete { pages: (0..6).collect() }).unwrap_err();
    assert!(err.to_string().contains("CANNOT_DELETE_ALL"));
    assert!(viewer.edit(id, PageEdit::Reorder { order: vec![0, 0, 1, 2, 3, 4] }).is_err());
    let info = viewer.edit(id, PageEdit::Rotate { pages: vec![0], degrees: 0 }).unwrap();
    assert_eq!(order(&viewer, &info), [6, 5, 4, 3, 2, 1]);

    // Save over the open file itself (the app must not keep it locked).
    let info = viewer.save(id, None).unwrap();
    assert!(!info.dirty);
    assert_eq!(file_order(&path), [6, 5, 4, 3, 2, 1]);
    let rotated = Viewer::new();
    let reopened = rotated.open(&path, None).unwrap();
    assert_eq!(reopened.pages[4], [h, w], "the rotation is saved in the file");

    // Save As: the document now belongs to the new file.
    let copy = dir.join("copy.pdf");
    viewer.edit(id, PageEdit::Delete { pages: vec![0] }).unwrap();
    let info = viewer.save(id, Some(copy.clone())).unwrap();
    assert_eq!(info.name, "copy.pdf");
    assert_eq!(file_order(&copy), [5, 4, 3, 2, 1]);
    assert_eq!(file_order(&path), [6, 5, 4, 3, 2, 1], "Save As leaves the old file alone");
    assert!(!dir.read_dir().unwrap().any(|e| e.unwrap().file_name().to_string_lossy().ends_with(".tmp")));
}

#[test]
fn insert_blank_page_and_pages_from_another_file() {
    let dir = work_dir("insert");
    let (a, b) = (dir.join("a.pdf"), dir.join("b.pdf"));
    labelled_pdf(&a, 1, 3);
    labelled_pdf(&b, 10, 2);
    let viewer = Viewer::new();
    let info = viewer.open(&a, None).unwrap();

    let info = viewer.edit(info.id, PageEdit::InsertFile { at: 1, path: b, password: None }).unwrap();
    assert_eq!(order(&viewer, &info), [1, 10, 11, 2, 3]);

    let info = viewer.edit(info.id, PageEdit::InsertBlank { at: 5, size: [300.0, 400.0] }).unwrap();
    assert_eq!(info.page_count, 6);
    assert_eq!(info.pages[5], [300.0, 400.0]);
    assert!(viewer.text_lines(info.id, 5).unwrap().is_empty());

    // A file that is not a PDF is refused and nothing changes.
    let junk = dir.join("junk.pdf");
    std::fs::write(&junk, b"not a pdf").unwrap();
    assert!(viewer.edit(info.id, PageEdit::InsertFile { at: 0, path: junk, password: None }).is_err());
    assert_eq!(viewer.undo(info.id).unwrap().page_count, 5);
}

#[test]
fn merge_two_hundred_page_files_in_order() {
    let dir = work_dir("merge");
    let (a, b) = (dir.join("a.pdf"), dir.join("b.pdf"));
    labelled_pdf(&a, 1, 100);
    labelled_pdf(&b, 101, 100);

    let started = Instant::now();
    let bytes = pages::merge(&[
        MergeInput { path: a.clone(), password: None },
        MergeInput { path: b.clone(), password: None },
    ])
    .unwrap();
    let merged = dir.join("merged.pdf");
    pages::write_file_safely(&merged, &bytes).unwrap();
    let took = started.elapsed();
    assert_eq!(file_order(&merged), (1..=200).collect::<Vec<_>>());
    assert!(took.as_millis() < 3000 * SLOWDOWN, "merging 2 x 100 pages took {took:?}");

    // The other order too.
    let bytes = pages::merge(&[MergeInput { path: b, password: None }, MergeInput { path: a, password: None }]).unwrap();
    let doc = PdfDocument::from_bytes(&bytes).unwrap();
    assert_eq!(doc.page_count().unwrap(), 200);
}

#[test]
fn split_and_extract() {
    let dir = work_dir("split");
    let path = dir.join("ten.pdf");
    labelled_pdf(&path, 1, 10);
    let viewer = Viewer::new();
    let info = viewer.open(&path, None).unwrap();

    // Every 4 pages: 4 + 4 + 2.
    let out = dir.join("every");
    std::fs::create_dir_all(&out).unwrap();
    let files = viewer.split_to(info.id, &pages::every_n(10, 4), &out, "ten").unwrap();
    assert_eq!(files.len(), 3);
    assert_eq!(file_order(&files[0]), [1, 2, 3, 4]);
    assert_eq!(file_order(&files[2]), [9, 10]);

    // Typed ranges, with Bangla digits. Running it again never overwrites.
    let groups = pages::parse_ranges("১-২, 5, 8-", 10).unwrap();
    let first = viewer.split_to(info.id, &groups, &out, "ten").unwrap();
    assert_eq!(first[0].file_name().unwrap(), "ten part 1 (2).pdf");
    assert_eq!(file_order(&first[1]), [5]);
    assert_eq!(file_order(&first[2]), [8, 9, 10]);
    assert_eq!(file_order(&files[0]), [1, 2, 3, 4], "the earlier file is untouched");

    // Extract uses the document as it is now, including unsaved changes.
    viewer.edit(info.id, PageEdit::Move { pages: vec![9], before: 0 }).unwrap();
    let picked = dir.join("picked.pdf");
    viewer.extract_to(info.id, &[0, 3, 4], &picked).unwrap();
    assert_eq!(file_order(&picked), [10, 3, 4]);

    // Splitting drops what the other pages used, so the parts are smaller.
    let whole = std::fs::metadata(&path).unwrap().len();
    let part = std::fs::metadata(&first[1]).unwrap().len();
    assert!(part < whole, "a 1-page part ({part} bytes) should be smaller than the file ({whole} bytes)");
}

#[test]
fn password_protected_file_stays_protected_after_editing() {
    let dir = work_dir("locked");
    let plain = dir.join("plain.pdf");
    labelled_pdf(&plain, 1, 3);
    let locked = dir.join("locked.pdf");
    let doc = PdfDocument::open(plain.to_str().unwrap()).unwrap();
    let mut opts = PdfWriteOptions::default();
    opts.set_encryption(Encryption::Aes256).set_user_password("গোপন123").set_owner_password("owner");
    doc.save_with_options(locked.to_str().unwrap(), opts).unwrap();

    let viewer = Viewer::new();
    let info = viewer.open(&locked, Some("গোপন123".into())).unwrap();
    let info = viewer.edit(info.id, PageEdit::Delete { pages: vec![1] }).unwrap();
    assert_eq!(order(&viewer, &info), [1, 3]);
    let info = viewer.undo(info.id).unwrap();
    assert_eq!(order(&viewer, &info), [1, 2, 3]);
    let info = viewer.redo(info.id).unwrap();
    viewer.save(info.id, None).unwrap();

    let check = Viewer::new();
    assert!(check.open(&locked, None).is_err(), "the saved file must still need its password");
    let info = check.open(&locked, Some("গোপন123".into())).unwrap();
    assert_eq!(order(&check, &info), [1, 3]);
}

#[test]
fn page_changes_on_a_hundred_page_file_are_quick() {
    let dir = work_dir("speed");
    let path = dir.join("hundred.pdf");
    labelled_pdf(&path, 1, 100);
    let viewer = Viewer::new();
    let info = viewer.open(&path, None).unwrap();

    let started = Instant::now();
    viewer.edit(info.id, PageEdit::Rotate { pages: vec![10], degrees: -90 }).unwrap();
    let rotate = started.elapsed();
    let started = Instant::now();
    viewer.edit(info.id, PageEdit::Move { pages: vec![0, 1, 2], before: 100 }).unwrap();
    let mov = started.elapsed();
    let started = Instant::now();
    let info = viewer.undo(info.id).unwrap();
    let undo = started.elapsed();
    println!("rotate: {rotate:?}, move: {mov:?}, undo: {undo:?}");
    assert_eq!(info.page_count, 100);
    for (what, took) in [("rotate", rotate), ("move", mov), ("undo", undo)] {
        assert!(took.as_millis() < 500 * SLOWDOWN, "{what} took {took:?} (limit 500 ms)");
    }
}
