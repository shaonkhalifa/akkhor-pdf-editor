//! Read text back out of a PDF, line by line, with positions.

use anyhow::{Context, Result};
use mupdf::text_page::TextPageFlags;
use mupdf::{Document, Point, Rect};

/// One visual line of text on a page. Coordinates are MuPDF page space:
/// origin at the top-left of the page, y grows downward, units are points.
#[derive(Debug, Clone)]
pub struct LineInfo {
    /// The line's text, in typing order (uses `/ActualText` where present).
    pub text: String,
    /// Box around the glyphs actually drawn on this line.
    pub bbox: Rect,
    /// Baseline origin of the leftmost glyph.
    pub origin: Point,
    /// Font size of the line's glyphs, in points.
    pub size: f32,
}

pub fn open_pdf(bytes: &[u8]) -> Result<Document> {
    Document::from_bytes(bytes, "application/pdf").context("opening PDF")
}

/// Raw MuPDF text lines with the given extraction flags.
fn raw_lines(doc: &Document, page_no: i32, flags: TextPageFlags) -> Result<Vec<LineInfo>> {
    let page = doc.load_page(page_no)?;
    let tp = page.to_text_page(flags)?;
    let mut out = Vec::new();
    for block in tp.blocks() {
        for line in block.lines() {
            let mut text = String::new();
            let mut origin: Option<Point> = None;
            let mut size = 0.0f32;
            for ch in line.chars() {
                let o = ch.origin();
                if origin.is_none_or(|cur| o.x < cur.x) {
                    origin = Some(o);
                }
                size = size.max(ch.size());
                if let Some(c) = ch.char() {
                    text.push(c);
                }
            }
            if let Some(origin) = origin {
                out.push(LineInfo { text, bbox: line.bounds(), origin, size });
            }
        }
    }
    Ok(merge_rows(out))
}

/// Join consecutive fragments that sit on the same baseline into one line.
///
/// MuPDF starts a new "line" whenever the pen moves left, which happens inside
/// Bangla words: a pre-base vowel such as ি is typed after its consonant but
/// drawn before it. Fragments of one visual row share a baseline.
fn merge_rows(lines: Vec<LineInfo>) -> Vec<LineInfo> {
    let mut rows: Vec<LineInfo> = Vec::new();
    for line in lines {
        if let Some(row) = rows.last_mut() {
            if same_row(row, &line) {
                row.text.push_str(&line.text);
                row.bbox = row.bbox.union(&line.bbox);
                if line.origin.x < row.origin.x {
                    row.origin = line.origin;
                }
                row.size = row.size.max(line.size);
                continue;
            }
        }
        rows.push(line);
    }
    rows
}

fn same_row(a: &LineInfo, b: &LineInfo) -> bool {
    let tolerance = 0.5 * a.size.max(b.size).max(1.0);
    (a.origin.y - b.origin.y).abs() <= tolerance
}

/// All visual lines on one page, in reading order.
///
/// Text and geometry come from two passes. With `/ActualText` (how Akkhor
/// stores shaped Bangla) the text pass gives the right characters but places
/// them only roughly; the geometry pass ignores ActualText and sees every
/// glyph where it is really drawn. Rows are matched by baseline.
pub fn page_lines(doc: &Document, page_no: i32) -> Result<Vec<LineInfo>> {
    let text_rows = raw_lines(doc, page_no, TextPageFlags::PRESERVE_WHITESPACE)?;
    let geometry = raw_lines(
        doc,
        page_no,
        TextPageFlags::PRESERVE_WHITESPACE | TextPageFlags::IGNORE_ACTUALTEXT,
    )?;
    Ok(text_rows
        .into_iter()
        .map(|mut row| {
            if let Some(g) = geometry.iter().find(|g| same_row(g, &row)) {
                row.bbox = g.bbox;
                row.origin = g.origin;
                row.size = g.size;
            }
            row
        })
        .collect::<Vec<_>>())
    .map(|mut rows: Vec<LineInfo>| {
        // Top to bottom, then left to right. Content added later (for
        // example an edited line) is drawn last but read in its place.
        rows.sort_by(|a, b| {
            if same_row(a, b) {
                a.origin.x.total_cmp(&b.origin.x)
            } else {
                a.origin.y.total_cmp(&b.origin.y)
            }
        });
        rows
    })
}

/// Whole-document text: one line per visual line, pages separated by a blank line.
pub fn document_text(bytes: &[u8]) -> Result<String> {
    let doc = open_pdf(bytes)?;
    let mut pages = Vec::new();
    for i in 0..doc.page_count()? {
        let lines: Vec<String> = page_lines(&doc, i)?.into_iter().map(|l| l.text).collect();
        pages.push(lines.join("\n"));
    }
    Ok(pages.join("\n\n"))
}
