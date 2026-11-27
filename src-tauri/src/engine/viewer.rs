//! The PDF viewer engine (SRS Modules 1 and 2) and page tools (Module 3).
//!
//! MuPDF documents can only be used from the thread that opened them, but a
//! recorded page (a `DisplayList`) can be drawn, searched and read from any
//! thread. So one engine thread owns every open document and records pages on
//! request; drawing at any zoom, text extraction and search then run in
//! parallel on other threads (SRS 4.1: render on background threads).
//!
//! Page changes also run on the engine thread. Before each change the
//! document is saved to memory, so every change can be undone. Files are read
//! into memory when opened, so the app never keeps the file open and can save
//! over it safely.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;

use anyhow::{anyhow, bail, Context, Result};
use mupdf::pdf::{PdfDocument, PdfWriteOptions};
use mupdf::{Colorspace, DisplayList, Document, ImageFormat, Matrix, Outline, Quad};
use serde::{Deserialize, Serialize};

use super::extract::display_list_lines;
use super::images::{self, PageImage};
use super::layout::Style;
use super::forms::{self, FormField};
use super::textedit::{self, Decoration, Markup, TextBlock, TextPlacement};
use super::pages;

/// Recorded pages kept per document. A page is re-recorded if evicted.
const LISTS_PER_DOC: usize = 96;
/// Search stops collecting hits on a page after this many.
const HITS_PER_PAGE: u32 = 500;
/// Undo steps kept per document.
const UNDO_STEPS: usize = 50;
/// Undo history is trimmed (oldest first) once it uses more memory than this.
const UNDO_BYTES: usize = 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct OutlineItem {
    pub title: String,
    /// Zero-based page number, if the bookmark points inside the document.
    pub page: Option<i32>,
    pub children: Vec<OutlineItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DocInfo {
    pub id: u64,
    pub path: String,
    pub name: String,
    pub page_count: i32,
    /// Width and height of every page, in points, after page rotation.
    pub pages: Vec<[f32; 2]>,
    pub outline: Vec<OutlineItem>,
    pub file_size: u64,
    /// Goes up on every change, so the UI knows to redraw pages.
    pub version: u64,
    /// There are changes not yet saved to `path`.
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// A new document that has never been saved (Save asks for a name).
    pub untitled: bool,
    /// The document has form fields to fill in.
    pub has_form: bool,
}

/// A change to the pages of an open document. Page numbers are zero-based.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PageEdit {
    /// Turn pages clockwise by a multiple of 90 degrees (negative: anticlockwise).
    Rotate { pages: Vec<i32>, degrees: i32 },
    Delete { pages: Vec<i32> },
    /// Move pages (keeping their order) to just before the page now at `before`.
    Move { pages: Vec<i32>, before: i32 },
    /// A full new order: `order[i]` is the current page that goes to position i.
    Reorder { order: Vec<i32> },
    InsertBlank { at: i32, size: [f32; 2] },
    InsertFile { at: i32, path: PathBuf, password: Option<String> },
    /// Put a picture (PNG or JPEG bytes) on a page.
    PlaceImage { page: i32, bytes: Vec<u8>, placement: images::Placement },
    DeleteImage { page: i32, xref: i32 },
    ReplaceImage { page: i32, xref: i32, bytes: Vec<u8> },
    /// Replace a block of text, or add a new text box (no `remove` boxes).
    PlaceText { page: i32, text: String, style: Style, placement: TextPlacement },
    Markup { page: i32, markup: Markup, rect: [f32; 4], color: [u8; 3] },
    /// Remove everything in an area for good, under a black box.
    Redact { page: i32, rect: [f32; 4] },
    /// Page numbers, header, footer or watermark.
    Decorate { item: Decoration, style: Style },
    /// Fill in a form field.
    SetField { page: i32, xref: i32, value: String, style: Style },
    /// Make form fields part of the pages.
    Flatten,
    /// Bijoy text to Unicode on the given pages (all when empty).
    ConvertBijoy { pages: Vec<i32>, style: Style },
    /// Replace a page with a searchable (OCR) copy.
    Ocr { page: i32, datadir: PathBuf, dpi: u32 },
}

#[derive(Debug, Clone, Serialize)]
pub struct PageLink {
    /// x0, y0, x1, y1 in page points.
    pub rect: [f32; 4],
    /// Target page for links inside the document.
    pub page: Option<i32>,
    /// Web or file address for other links (shown, never opened: no network).
    pub uri: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextLine {
    pub text: String,
    /// x0, y0, x1, y1 in page points.
    pub rect: [f32; 4],
    pub size: f32,
    /// Writing direction, clockwise: 0 (left to right), 90 (top to bottom), 180 or 270.
    pub angle: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub page: i32,
    /// One rectangle per hit: x0, y0, x1, y1 in page points.
    pub rect: [f32; 4],
}

/// Why a document could not be opened.
#[derive(Debug)]
pub enum OpenError {
    NeedsPassword,
    WrongPassword,
    Failed(anyhow::Error),
}

impl std::fmt::Display for OpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Stable codes the UI can match on.
            OpenError::NeedsPassword => write!(f, "NEEDS_PASSWORD"),
            OpenError::WrongPassword => write!(f, "WRONG_PASSWORD"),
            OpenError::Failed(e) => write!(f, "{e:#}"),
        }
    }
}

enum Job {
    Open {
        path: PathBuf,
        password: Option<String>,
        reply: Sender<Result<DocInfo, OpenError>>,
    },
    Close {
        id: u64,
    },
    List {
        id: u64,
        page: i32,
        reply: Sender<Result<Arc<DisplayList>>>,
    },
    Links {
        id: u64,
        page: i32,
        reply: Sender<Result<Vec<PageLink>>>,
    },
    Edit {
        id: u64,
        edit: PageEdit,
        reply: Sender<Result<DocInfo>>,
    },
    Step {
        id: u64,
        redo: bool,
        reply: Sender<Result<DocInfo>>,
    },
    Save {
        id: u64,
        dest: Option<PathBuf>,
        reply: Sender<Result<DocInfo>>,
    },
    Snapshot {
        id: u64,
        reply: Sender<Result<(Arc<Vec<u8>>, Option<String>)>>,
    },
    Images {
        id: u64,
        page: i32,
        reply: Sender<Result<Vec<PageImage>>>,
    },
    Blocks {
        id: u64,
        page: i32,
        reply: Sender<Result<Vec<TextBlock>>>,
    },
    Fields {
        id: u64,
        page: i32,
        reply: Sender<Result<Vec<FormField>>>,
    },
    OcrArea {
        id: u64,
        page: i32,
        rect: [f32; 4],
        datadir: PathBuf,
        reply: Sender<Result<String>>,
    },
    /// A new document made in memory (blank, from pictures, …).
    OpenBytes {
        bytes: Vec<u8>,
        name: String,
        reply: Sender<Result<DocInfo>>,
    },
}

/// One earlier state of a document, for undo and redo.
struct Snapshot {
    state: u64,
    bytes: Arc<Vec<u8>>,
}

struct Entry {
    doc: PdfDocument,
    path: PathBuf,
    password: Option<String>,
    file_size: u64,
    page_count: i32,
    lists: HashMap<i32, Arc<DisplayList>>,
    recent: VecDeque<i32>,
    /// Which state the document is in; every change makes a new one.
    state: u64,
    /// The state last saved to (or opened from) `path`.
    saved_state: u64,
    /// The current state as bytes, if already known (saves writing it again).
    current: Option<Arc<Vec<u8>>>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    version: u64,
    untitled: bool,
}

/// Handle to the engine thread. Cheap to share between threads.
#[derive(Clone)]
pub struct Viewer {
    tx: Sender<Job>,
}

impl Default for Viewer {
    fn default() -> Self {
        Self::new()
    }
}

impl Viewer {
    pub fn new() -> Self {
        let (tx, rx) = channel::<Job>();
        thread::Builder::new()
            .name("akkhor-pdf-engine".into())
            .spawn(move || {
                let mut docs: HashMap<u64, Entry> = HashMap::new();
                let mut next_id: u64 = 1;
                for job in rx {
                    match job {
                        Job::Open { path, password, reply } => {
                            let result = open_document(&path, password.as_deref()).and_then(|(doc, bytes)| {
                                let id = next_id;
                                next_id += 1;
                                let state = new_state();
                                let entry = docs.entry(id).or_insert(Entry {
                                    doc,
                                    path,
                                    password,
                                    file_size: bytes.len() as u64,
                                    page_count: 0,
                                    lists: HashMap::new(),
                                    recent: VecDeque::new(),
                                    state,
                                    saved_state: state,
                                    current: Some(Arc::new(bytes)),
                                    undo: Vec::new(),
                                    redo: Vec::new(),
                                    version: 0,
                                    untitled: false,
                                });
                                let info = entry.info(id);
                                if info.is_err() {
                                    docs.remove(&id);
                                }
                                info.map_err(OpenError::Failed)
                            });
                            let _ = reply.send(result);
                        }
                        Job::Close { id } => {
                            docs.remove(&id);
                        }
                        Job::List { id, page, reply } => {
                            let _ = reply.send(record_page(&mut docs, id, page));
                        }
                        Job::Links { id, page, reply } => {
                            let result = entry(&mut docs, id).and_then(|e| page_links(&e.doc, page));
                            let _ = reply.send(result);
                        }
                        Job::Edit { id, edit, reply } => {
                            let _ = reply.send(entry(&mut docs, id).and_then(|e| e.apply(edit, id)));
                        }
                        Job::Step { id, redo, reply } => {
                            let _ = reply.send(entry(&mut docs, id).and_then(|e| e.step(redo, id)));
                        }
                        Job::Save { id, dest, reply } => {
                            let _ = reply.send(entry(&mut docs, id).and_then(|e| e.save(dest, id)));
                        }
                        Job::Images { id, page, reply } => {
                            let result = entry(&mut docs, id).and_then(|e| images::page_images(&e.doc, page));
                            let _ = reply.send(result);
                        }
                        Job::Blocks { id, page, reply } => {
                            let result = entry(&mut docs, id).and_then(|e| textedit::text_blocks(&e.doc, page));
                            let _ = reply.send(result);
                        }
                        Job::Fields { id, page, reply } => {
                            let result = entry(&mut docs, id).and_then(|e| forms::fields(&e.doc, page));
                            let _ = reply.send(result);
                        }
                        Job::OcrArea { id, page, rect, datadir, reply } => {
                            let result = entry(&mut docs, id).and_then(|e| super::ocr::read_area(&e.doc, page, rect, &datadir));
                            let _ = reply.send(result);
                        }
                        Job::OpenBytes { bytes, name, reply } => {
                            let result = PdfDocument::from_bytes(&bytes).map_err(anyhow::Error::from).and_then(|doc| {
                                let id = next_id;
                                next_id += 1;
                                let state = new_state();
                                let entry = docs.entry(id).or_insert(Entry {
                                    doc,
                                    path: PathBuf::from(name),
                                    password: None,
                                    file_size: bytes.len() as u64,
                                    page_count: 0,
                                    lists: HashMap::new(),
                                    recent: VecDeque::new(),
                                    // Unsaved from the start.
                                    state: new_state(),
                                    saved_state: state,
                                    current: Some(Arc::new(bytes)),
                                    undo: Vec::new(),
                                    redo: Vec::new(),
                                    version: 0,
                                    untitled: true,
                                });
                                let info = entry.info(id);
                                if info.is_err() {
                                    docs.remove(&id);
                                }
                                info
                            });
                            let _ = reply.send(result);
                        }
                        Job::Snapshot { id, reply } => {
                            let result =
                                entry(&mut docs, id).and_then(|e| Ok((e.current_bytes()?, e.password.clone())));
                            let _ = reply.send(result);
                        }
                    }
                }
            })
            .expect("starting the PDF engine thread");
        Self { tx }
    }

    fn ask<T>(&self, make: impl FnOnce(Sender<T>) -> Job) -> Result<T> {
        let (reply, rx) = channel();
        self.tx.send(make(reply)).map_err(|_| anyhow!("PDF engine stopped"))?;
        rx.recv().map_err(|_| anyhow!("PDF engine stopped"))
    }

    /// Change the pages of an open document. Every change can be undone.
    pub fn edit(&self, id: u64, edit: PageEdit) -> Result<DocInfo> {
        self.ask(|reply| Job::Edit { id, edit, reply })?
    }

    pub fn undo(&self, id: u64) -> Result<DocInfo> {
        self.ask(|reply| Job::Step { id, redo: false, reply })?
    }

    pub fn redo(&self, id: u64) -> Result<DocInfo> {
        self.ask(|reply| Job::Step { id, redo: true, reply })?
    }

    /// Save to the document's own file, or to `dest` (Save As: the document
    /// then belongs to `dest`).
    pub fn save(&self, id: u64, dest: Option<PathBuf>) -> Result<DocInfo> {
        self.ask(|reply| Job::Save { id, dest, reply })?
    }

    /// Images drawn on a page, and which of them can be deleted or replaced.
    pub fn images(&self, id: u64, page: i32) -> Result<Vec<PageImage>> {
        self.ask(|reply| Job::Images { id, page, reply })?
    }

    /// Open a document made in memory (not yet saved anywhere).
    pub fn open_bytes(&self, bytes: Vec<u8>, name: &str) -> Result<DocInfo> {
        self.ask(|reply| Job::OpenBytes { bytes, name: name.to_string(), reply })?
    }

    /// Recognise the text in an area of a page (OCR).
    pub fn ocr_area(&self, id: u64, page: i32, rect: [f32; 4], datadir: &Path) -> Result<String> {
        self.ask(|reply| Job::OcrArea { id, page, rect, datadir: datadir.to_path_buf(), reply })?
    }

    /// Form fields on a page.
    pub fn fields(&self, id: u64, page: i32) -> Result<Vec<FormField>> {
        self.ask(|reply| Job::Fields { id, page, reply })?
    }

    /// The document as it is now, as bytes, with its password (for exports).
    pub fn snapshot(&self, id: u64) -> Result<(Arc<Vec<u8>>, Option<String>)> {
        self.ask(|reply| Job::Snapshot { id, reply })?
    }

    /// The text blocks on a page, for editing.
    pub fn text_blocks(&self, id: u64, page: i32) -> Result<Vec<TextBlock>> {
        self.ask(|reply| Job::Blocks { id, page, reply })?
    }

    /// Copy some pages of the document, as it is now, into a new PDF file.
    pub fn extract_to(&self, id: u64, pages: &[i32], dest: &Path) -> Result<()> {
        let (bytes, password) = self.ask(|reply| Job::Snapshot { id, reply })??;
        let out = pages::extract(&bytes, password.as_deref(), pages)?;
        pages::write_file_safely(dest, &out)
    }

    /// Split the document, as it is now, into one file per group of pages.
    pub fn split_to(&self, id: u64, groups: &[Vec<i32>], dir: &Path, stem: &str) -> Result<Vec<PathBuf>> {
        let (bytes, password) = self.ask(|reply| Job::Snapshot { id, reply })??;
        pages::split(&bytes, password.as_deref(), groups, dir, stem)
    }

    pub fn open(&self, path: &Path, password: Option<String>) -> Result<DocInfo, OpenError> {
        self.ask(|reply| Job::Open { path: path.to_path_buf(), password, reply })
            .map_err(OpenError::Failed)?
    }

    pub fn close(&self, id: u64) {
        let _ = self.tx.send(Job::Close { id });
    }

    /// The recorded page, ready to draw or read on any thread.
    pub fn page(&self, id: u64, page: i32) -> Result<Arc<DisplayList>> {
        self.ask(|reply| Job::List { id, page, reply })?
    }

    pub fn links(&self, id: u64, page: i32) -> Result<Vec<PageLink>> {
        self.ask(|reply| Job::Links { id, page, reply })?
    }

    /// Draw a page as raw RGBA pixels. The first 8 bytes are the width and
    /// height (u32, little-endian); the pixels follow, row by row.
    pub fn render_rgba(&self, id: u64, page: i32, scale: f32) -> Result<Vec<u8>> {
        let list = self.page(id, page)?;
        render_rgba(&list, scale)
    }

    /// Draw a page as a PNG (used for printing).
    pub fn render_png(&self, id: u64, page: i32, scale: f32) -> Result<Vec<u8>> {
        let list = self.page(id, page)?;
        let pixmap = list.to_pixmap(&Matrix::new_scale(scale, scale), &Colorspace::device_rgb(), false)?;
        let mut png = Vec::new();
        pixmap.write_to(&mut png, ImageFormat::PNG)?;
        Ok(png)
    }

    /// Visual lines of text on a page, with their boxes.
    pub fn text_lines(&self, id: u64, page: i32) -> Result<Vec<TextLine>> {
        let list = self.page(id, page)?;
        Ok(display_list_lines(&list)?
            .into_iter()
            .filter(|l| !l.text.trim().is_empty())
            .map(|l| TextLine {
                text: l.text,
                rect: [l.bbox.x0, l.bbox.y0, l.bbox.x1, l.bbox.y1],
                size: l.size,
                angle: l.angle,
            })
            .collect())
    }

    /// Find `needle` (case-insensitive) on every page.
    ///
    /// Matches are found in each line's text (correct Unicode, including
    /// ActualText) and placed using the line's real glyph box. Where MuPDF's
    /// own search gives a box that clearly covers the word, that exact box is
    /// used instead; for shaped Bangla stored as ActualText MuPDF's boxes are
    /// unreliable, so the estimate from the line is kept.
    pub fn search(&self, id: u64, page_count: i32, needle: &str) -> Result<Vec<SearchHit>> {
        let needle: Vec<char> = fold(needle.trim());
        if needle.is_empty() {
            return Ok(Vec::new());
        }
        let needle_str: String = needle.iter().collect();
        let mut hits = Vec::new();
        for page in 0..page_count {
            let list = self.page(id, page)?;
            let exact: Vec<[f32; 4]> = list
                .search(&needle_str, HITS_PER_PAGE)
                .map(|q| q.iter().map(quad_rect).collect())
                .unwrap_or_default();
            for line in display_list_lines(&list)? {
                for rect in line_matches(&line.text, &needle, line.bbox, line.angle) {
                    let better = exact.iter().filter(|_| line.angle == 0).find(|q| {
                        let overlap_x = q[2].min(rect[2]) - q[0].max(rect[0]);
                        let same_row = q[1] < rect[3] && q[3] > rect[1];
                        same_row && overlap_x > 0.0 && (q[2] - q[0]) >= 0.6 * (rect[2] - rect[0])
                    });
                    hits.push(SearchHit { page, rect: better.copied().unwrap_or(rect) });
                }
            }
        }
        Ok(hits)
    }
}

/// Lower-case characters for case-insensitive matching (one char per char,
/// so positions stay the same).
fn fold(s: &str) -> Vec<char> {
    s.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect()
}

/// Boxes for every occurrence of `needle` in a line, estimated by spreading
/// the line's characters evenly along its glyph box, in its writing direction.
fn line_matches(text: &str, needle: &[char], bbox: mupdf::Rect, angle: i32) -> Vec<[f32; 4]> {
    let hay = fold(text);
    let n = hay.len();
    if needle.len() > n || n == 0 {
        return Vec::new();
    }
    let vertical = angle == 90 || angle == 270;
    let reversed = angle == 180 || angle == 270;
    let (start, len) = if vertical { (bbox.y0, bbox.y1 - bbox.y0) } else { (bbox.x0, bbox.x1 - bbox.x0) };
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= n {
        if hay[i..i + needle.len()] == *needle {
            let (mut a, mut b) = (i as f32 / n as f32, (i + needle.len()) as f32 / n as f32);
            if reversed {
                (a, b) = (1.0 - b, 1.0 - a);
            }
            let (lo, hi) = (start + len * a, start + len * b);
            out.push(if vertical { [bbox.x0, lo, bbox.x1, hi] } else { [lo, bbox.y0, hi, bbox.y1] });
            i += needle.len();
        } else {
            i += 1;
        }
    }
    out
}

fn quad_rect(q: &Quad) -> [f32; 4] {
    let xs = [q.ul.x, q.ur.x, q.ll.x, q.lr.x];
    let ys = [q.ul.y, q.ur.y, q.ll.y, q.lr.y];
    [
        xs.iter().copied().fold(f32::INFINITY, f32::min),
        ys.iter().copied().fold(f32::INFINITY, f32::min),
        xs.iter().copied().fold(f32::NEG_INFINITY, f32::max),
        ys.iter().copied().fold(f32::NEG_INFINITY, f32::max),
    ]
}

pub fn render_rgba(list: &DisplayList, scale: f32) -> Result<Vec<u8>> {
    let pixmap = list.to_pixmap(&Matrix::new_scale(scale, scale), &Colorspace::device_rgb(), false)?;
    let (w, h, n) = (pixmap.width(), pixmap.height(), pixmap.n() as usize);
    let stride = pixmap.stride() as usize;
    let samples = pixmap.samples();
    let (wu, hu) = (w as usize, h as usize);
    let mut out = vec![255u8; 8 + wu * hu * 4];
    out[0..4].copy_from_slice(&w.to_le_bytes());
    out[4..8].copy_from_slice(&h.to_le_bytes());
    // Copy RGB into RGBA (alpha already 255), one row at a time.
    for (y, dst_row) in out[8..].chunks_exact_mut(wu * 4).enumerate() {
        let src_row = &samples[y * stride..y * stride + wu * n];
        for (dst, src) in dst_row.chunks_exact_mut(4).zip(src_row.chunks_exact(n)) {
            dst[..3].copy_from_slice(&src[..3]);
        }
    }
    Ok(out)
}

fn open_document(path: &Path, password: Option<&str>) -> Result<(PdfDocument, Vec<u8>), OpenError> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading {}", path.display()))
        .map_err(OpenError::Failed)?;
    let mut doc = PdfDocument::from_bytes(&bytes)
        .map_err(|e| OpenError::Failed(anyhow!("this file could not be opened as a PDF ({e})")))?;
    if doc.needs_password().map_err(|e| OpenError::Failed(e.into()))? {
        match password {
            None => return Err(OpenError::NeedsPassword),
            Some(pw) => {
                if !doc.authenticate(pw).map_err(|e| OpenError::Failed(e.into()))? {
                    return Err(OpenError::WrongPassword);
                }
            }
        }
    }
    Ok((doc, bytes))
}

/// Load an earlier state back (undo, redo, or after a failed change).
fn load_state(bytes: &[u8], password: Option<&str>) -> Result<PdfDocument> {
    let mut doc = PdfDocument::from_bytes(bytes)?;
    if doc.needs_password()? && !password.is_some_and(|pw| doc.authenticate(pw).unwrap_or(false)) {
        bail!("the saved state could not be unlocked");
    }
    Ok(doc)
}

fn entry(docs: &mut HashMap<u64, Entry>, id: u64) -> Result<&mut Entry> {
    docs.get_mut(&id).ok_or_else(|| anyhow!("document {id} is not open"))
}

static STATE_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn new_state() -> u64 {
    STATE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Entry {
    fn info(&mut self, id: u64) -> Result<DocInfo> {
        let doc: &Document = &self.doc;
        let page_count = doc.page_count()?;
        let mut sizes = Vec::with_capacity(page_count.max(0) as usize);
        for i in 0..page_count {
            let b = doc.load_page(i)?.bounds()?;
            sizes.push([b.x1 - b.x0, b.y1 - b.y0]);
        }
        let outline = doc.outlines().map(|o| convert_outline(&o)).unwrap_or_default();
        self.page_count = page_count;
        Ok(DocInfo {
            id,
            path: self.path.to_string_lossy().into_owned(),
            name: self
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "document.pdf".into()),
            page_count,
            pages: sizes,
            outline,
            file_size: self.file_size,
            version: self.version,
            dirty: self.state != self.saved_state,
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
            untitled: self.untitled,
            has_form: forms::has_form(&self.doc).unwrap_or(false),
        })
    }

    /// The current state as bytes. Written quickly (no clean-up or
    /// compression); only used in memory.
    fn current_bytes(&mut self) -> Result<Arc<Vec<u8>>> {
        if let Some(b) = &self.current {
            return Ok(Arc::clone(b));
        }
        let bytes = Arc::new(pages::to_bytes(&self.doc, PdfWriteOptions::default())?);
        self.current = Some(Arc::clone(&bytes));
        Ok(bytes)
    }

    fn check_page(&self, page: i32) -> Result<()> {
        if page < 0 || page >= self.page_count {
            bail!("page {} does not exist", page + 1);
        }
        Ok(())
    }

    /// Forget recorded pages and tell the UI to redraw.
    fn changed(&mut self) {
        self.lists.clear();
        self.recent.clear();
        self.version += 1;
    }

    fn trim_undo(&mut self) {
        let total = |u: &[Snapshot]| u.iter().map(|s| s.bytes.len()).sum::<usize>();
        while self.undo.len() > UNDO_STEPS || (self.undo.len() > 1 && total(&self.undo) > UNDO_BYTES) {
            self.undo.remove(0);
        }
    }

    fn apply(&mut self, edit: PageEdit, id: u64) -> Result<DocInfo> {
        let before = self.current_bytes()?;
        let result = match edit {
            PageEdit::Rotate { pages, degrees } => pages::rotate(&mut self.doc, &pages, degrees),
            PageEdit::Delete { pages } => pages::delete(&mut self.doc, &pages),
            PageEdit::Move { pages, before } => {
                let order = pages::order_for_move(self.page_count, &pages, before);
                pages::reorder(&mut self.doc, &order)
            }
            PageEdit::Reorder { order } => pages::reorder(&mut self.doc, &order),
            PageEdit::InsertBlank { at, size } => pages::insert_blank(&mut self.doc, at, size),
            PageEdit::InsertFile { at, path, password } => pages::load(&path, password.as_deref())
                .and_then(|other| pages::insert_document(&mut self.doc, at, &other))
                .map(|_| ()),
            PageEdit::PlaceImage { page, bytes, placement } => {
                self.check_page(page).and_then(|_| images::place(&mut self.doc, page, &bytes, &placement))
            }
            PageEdit::DeleteImage { page, xref } => {
                self.check_page(page).and_then(|_| images::delete_image(&mut self.doc, page, xref))
            }
            PageEdit::ReplaceImage { page, xref, bytes } => {
                self.check_page(page).and_then(|_| images::replace_image(&mut self.doc, page, xref, &bytes))
            }
            PageEdit::PlaceText { page, text, style, placement } => self
                .check_page(page)
                .and_then(|_| textedit::place_text(&mut self.doc, page, &text, &style, &placement))
                .map(|_| ()),
            PageEdit::Markup { page, markup, rect, color } => self
                .check_page(page)
                .and_then(|_| textedit::markup(&mut self.doc, page, markup, rect, color))
                .map(|_| ()),
            PageEdit::Redact { page, rect } => {
                self.check_page(page).and_then(|_| textedit::redact(&mut self.doc, page, rect))
            }
            PageEdit::Decorate { item, style } => textedit::decorate(&mut self.doc, &item, &style).map(|_| ()),
            PageEdit::SetField { page, xref, value, style } => {
                self.check_page(page).and_then(|_| forms::set_field(&mut self.doc, page, xref, &value, &style))
            }
            PageEdit::Flatten => forms::flatten(&mut self.doc),
            PageEdit::ConvertBijoy { pages, style } => textedit::convert_bijoy(&mut self.doc, &pages, &style).map(|_| ()),
            PageEdit::Ocr { page, datadir, dpi } => {
                self.check_page(page).and_then(|_| super::ocr::make_searchable(&mut self.doc, page, &datadir, dpi)).map(|_| ())
            }
        };
        if let Err(e) = result {
            // A change that fails half way must not leave a half-changed document.
            self.doc = load_state(&before, self.password.as_deref())?;
            self.changed();
            return Err(e);
        }
        self.undo.push(Snapshot { state: self.state, bytes: before });
        self.trim_undo();
        self.redo.clear();
        self.state = new_state();
        self.current = None;
        self.changed();
        self.info(id)
    }

    fn step(&mut self, redo: bool, id: u64) -> Result<DocInfo> {
        let Some(target) = (if redo { self.redo.pop() } else { self.undo.pop() }) else {
            bail!(if redo { "NOTHING_TO_REDO" } else { "NOTHING_TO_UNDO" });
        };
        let now = Snapshot { state: self.state, bytes: self.current_bytes()? };
        match load_state(&target.bytes, self.password.as_deref()) {
            Ok(doc) => self.doc = doc,
            Err(e) => {
                if redo { self.redo.push(target) } else { self.undo.push(target) }
                return Err(e);
            }
        }
        if redo { self.undo.push(now) } else { self.redo.push(now) }
        self.state = target.state;
        self.current = Some(target.bytes);
        self.changed();
        self.info(id)
    }

    fn save(&mut self, dest: Option<PathBuf>, id: u64) -> Result<DocInfo> {
        if dest.is_none() && self.untitled {
            bail!("NEEDS_SAVE_AS");
        }
        let dest = dest.unwrap_or_else(|| self.path.clone());
        let bytes = pages::to_bytes(&self.doc, pages::save_options())?;
        pages::write_file_safely(&dest, &bytes)?;
        self.path = dest;
        self.untitled = false;
        self.file_size = bytes.len() as u64;
        self.saved_state = self.state;
        self.info(id)
    }
}

fn convert_outline(items: &[Outline]) -> Vec<OutlineItem> {
    items
        .iter()
        .map(|o| OutlineItem {
            title: o.title.clone(),
            page: o.dest.map(|d| d.loc.page_number as i32),
            children: convert_outline(&o.down),
        })
        .collect()
}

fn record_page(docs: &mut HashMap<u64, Entry>, id: u64, page: i32) -> Result<Arc<DisplayList>> {
    let entry = docs.get_mut(&id).ok_or_else(|| anyhow!("document {id} is not open"))?;
    if page < 0 || page >= entry.page_count {
        return Err(anyhow!("page {} does not exist", page + 1));
    }
    if let Some(list) = entry.lists.get(&page) {
        let list = Arc::clone(list);
        entry.recent.retain(|p| *p != page);
        entry.recent.push_back(page);
        return Ok(list);
    }
    let list = Arc::new(entry.doc.load_page(page)?.to_display_list(true)?);
    entry.lists.insert(page, Arc::clone(&list));
    entry.recent.push_back(page);
    while entry.recent.len() > LISTS_PER_DOC {
        if let Some(old) = entry.recent.pop_front() {
            entry.lists.remove(&old);
        }
    }
    Ok(list)
}

fn page_links(doc: &Document, page: i32) -> Result<Vec<PageLink>> {
    let p = doc.load_page(page)?;
    let mut out = Vec::new();
    for link in p.links()? {
        let internal = link.dest.map(|d| d.loc.page_number as i32);
        out.push(PageLink {
            rect: [link.bounds.x0, link.bounds.y0, link.bounds.x1, link.bounds.y1],
            page: internal,
            uri: if internal.is_none() && !link.uri.is_empty() { Some(link.uri.clone()) } else { None },
        });
    }
    Ok(out)
}
