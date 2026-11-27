//! The PDF viewer engine (SRS Modules 1 and 2).
//!
//! MuPDF documents can only be used from the thread that opened them, but a
//! recorded page (a `DisplayList`) can be drawn, searched and read from any
//! thread. So one engine thread owns every open document and records pages on
//! request; drawing at any zoom, text extraction and search then run in
//! parallel on other threads (SRS 4.1: render on background threads).

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::thread;

use anyhow::{anyhow, Context, Result};
use mupdf::{Colorspace, DisplayList, Document, ImageFormat, Matrix, Outline, Quad};
use serde::Serialize;

use super::extract::display_list_lines;
use super::layout::mupdf_path;

/// Recorded pages kept per document. A page is re-recorded if evicted.
const LISTS_PER_DOC: usize = 96;
/// Search stops collecting hits on a page after this many.
const HITS_PER_PAGE: u32 = 500;

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
}

struct Entry {
    doc: Document,
    page_count: i32,
    lists: HashMap<i32, Arc<DisplayList>>,
    recent: VecDeque<i32>,
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
                            let result = open_document(&path, password.as_deref()).map(|(doc, mut info)| {
                                info.id = next_id;
                                next_id += 1;
                                docs.insert(
                                    info.id,
                                    Entry {
                                        doc,
                                        page_count: info.page_count,
                                        lists: HashMap::new(),
                                        recent: VecDeque::new(),
                                    },
                                );
                                info
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
                            let result = docs
                                .get(&id)
                                .ok_or_else(|| anyhow!("document {id} is not open"))
                                .and_then(|e| page_links(&e.doc, page));
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
                for rect in line_matches(&line.text, &needle, line.bbox) {
                    let better = exact.iter().find(|q| {
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
/// the line's characters evenly across its glyph box.
fn line_matches(text: &str, needle: &[char], bbox: mupdf::Rect) -> Vec<[f32; 4]> {
    let hay = fold(text);
    let n = hay.len();
    if needle.len() > n || n == 0 {
        return Vec::new();
    }
    let width = bbox.x1 - bbox.x0;
    let mut out = Vec::new();
    let mut i = 0;
    while i + needle.len() <= n {
        if hay[i..i + needle.len()] == *needle {
            let x0 = bbox.x0 + width * i as f32 / n as f32;
            let x1 = bbox.x0 + width * (i + needle.len()) as f32 / n as f32;
            out.push([x0, bbox.y0, x1, bbox.y1]);
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

fn open_document(path: &Path, password: Option<&str>) -> Result<(Document, DocInfo), OpenError> {
    let file_size = std::fs::metadata(path)
        .with_context(|| format!("reading {}", path.display()))
        .map_err(OpenError::Failed)?
        .len();
    let mut doc = Document::open(&mupdf_path(path).map_err(OpenError::Failed)?)
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
    let info = describe(&doc, path, file_size).map_err(OpenError::Failed)?;
    Ok((doc, info))
}

fn describe(doc: &Document, path: &Path, file_size: u64) -> Result<DocInfo> {
    let page_count = doc.page_count()?;
    let mut pages = Vec::with_capacity(page_count.max(0) as usize);
    for i in 0..page_count {
        let b = doc.load_page(i)?.bounds()?;
        pages.push([b.x1 - b.x0, b.y1 - b.y0]);
    }
    let outline = doc.outlines().map(|o| convert_outline(&o)).unwrap_or_default();
    Ok(DocInfo {
        id: 0,
        path: path.to_string_lossy().into_owned(),
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "document.pdf".into()),
        page_count,
        pages,
        outline,
        file_size,
    })
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
