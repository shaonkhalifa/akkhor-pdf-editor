//! Read text back out of a PDF, line by line, with positions.

use anyhow::{Context, Result};
use mupdf::text_page::{TextPage, TextPageFlags};
use mupdf::{DisplayList, Document, Point, Rect};

/// One visual line of text on a page. Coordinates are MuPDF page space:
/// origin at the top-left of the page, y grows downward, units are points.
#[derive(Debug, Clone)]
pub struct LineInfo {
    /// The line's text, in typing order (uses `/ActualText` where present).
    pub text: String,
    /// Box around the glyphs actually drawn on this line.
    pub bbox: Rect,
    /// Baseline origin of the first glyph along the line.
    pub origin: Point,
    /// Font size of the line's glyphs, in points.
    pub size: f32,
    /// Writing direction, clockwise from left-to-right: 0, 90, 180 or 270
    /// degrees. Text on a page rotated by 90 degrees runs top to bottom.
    pub angle: i32,
    /// The font of the line's first glyph, as named in the PDF (subset prefix
    /// such as "ABCDEF+" removed), with its style.
    pub font: FontInfo,
    /// Colour of the first glyph: 0xAARRGGBB.
    pub argb: u32,
}

/// What a PDF says about a font used for text.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FontInfo {
    pub name: String,
    pub bold: bool,
    pub italic: bool,
    pub serif: bool,
    pub mono: bool,
}

impl FontInfo {
    fn of(font: &mupdf::Font) -> Self {
        let raw = font.name();
        let name = match raw.split_once('+') {
            Some((prefix, rest)) if prefix.len() == 6 && prefix.chars().all(|c| c.is_ascii_uppercase()) => rest,
            _ => raw,
        }
        .to_string();
        let lower = name.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
        Self {
            bold: font.is_bold() || has(&["bold", "black", "heavy", "semibold", "demi"]),
            italic: font.is_italic() || has(&["italic", "oblique"]),
            serif: font.is_serif() || has(&["times", "roman", "serif", "georgia", "garamond", "cambria", "book"]) && !has(&["sans"]),
            mono: font.is_monospaced() || has(&["courier", "mono", "consolas"]),
            name,
        }
    }
}

impl LineInfo {
    /// Unit vector of the writing direction.
    fn dir(&self) -> (f32, f32) {
        dir_of(self.angle)
    }
    /// Distance along the line (x for upright text).
    fn along(&self, p: Point) -> f32 {
        let (dx, dy) = self.dir();
        p.x * dx + p.y * dy
    }
    /// Distance across the line, growing in reading order of rows (y for upright text).
    fn across(&self, p: Point) -> f32 {
        let (dx, dy) = self.dir();
        -p.x * dy + p.y * dx
    }
}

fn dir_of(angle: i32) -> (f32, f32) {
    match angle.rem_euclid(360) {
        90 => (0.0, 1.0),
        180 => (-1.0, 0.0),
        270 => (0.0, -1.0),
        _ => (1.0, 0.0),
    }
}

/// The nearest of the four directions to the vector `(dx, dy)`.
fn snap_angle(dx: f32, dy: f32) -> i32 {
    if dx.abs() >= dy.abs() {
        if dx >= 0.0 { 0 } else { 180 }
    } else if dy >= 0.0 {
        90
    } else {
        270
    }
}

pub fn open_pdf(bytes: &[u8]) -> Result<Document> {
    Document::from_bytes(bytes, "application/pdf").context("opening PDF")
}

const TEXT_FLAGS: TextPageFlags = TextPageFlags::PRESERVE_WHITESPACE;
const GEOMETRY_FLAGS: TextPageFlags =
    TextPageFlags::PRESERVE_WHITESPACE.union(TextPageFlags::IGNORE_ACTUALTEXT);

/// Visual rows from one MuPDF text page.
fn rows_of(tp: &TextPage) -> Vec<LineInfo> {
    let mut out = Vec::new();
    for block in tp.blocks() {
        for line in block.lines() {
            let mut text = String::new();
            let mut origin: Option<Point> = None;
            let mut size = 0.0f32;
            // A glyph's quad runs along the writing direction.
            let angle = line
                .chars()
                .next()
                .map(|c| {
                    let q = c.quad();
                    snap_angle(q.ur.x - q.ul.x, q.ur.y - q.ul.y)
                })
                .unwrap_or(0);
            let (dx, dy) = dir_of(angle);
            let mut font = FontInfo::default();
            let mut argb = 0xFF00_0000;
            if let Some(first) = line.chars().next() {
                if let Some(f) = first.font() {
                    font = FontInfo::of(&f);
                }
                argb = first.argb();
            }
            for ch in line.chars() {
                let o = ch.origin();
                if origin.is_none_or(|cur| o.x * dx + o.y * dy < cur.x * dx + cur.y * dy) {
                    origin = Some(o);
                }
                size = size.max(ch.size());
                if let Some(c) = ch.char() {
                    text.push(c);
                }
            }
            if let Some(origin) = origin {
                out.push(LineInfo { text, bbox: line.bounds(), origin, size, angle, font, argb });
            }
        }
    }
    out
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
            // Far-apart text on one baseline (table cells, columns) stays apart.
            let size = row.size.max(line.size).max(1.0);
            if same_row(row, &line) && gap_along(row, &line) < 2.5 * size {
                row.text.push_str(&line.text);
                row.bbox = row.bbox.union(&line.bbox);
                if row.along(line.origin) < row.along(row.origin) {
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
    a.angle == b.angle && (a.across(a.origin) - a.across(b.origin)).abs() <= tolerance
}

/// Space along the line between the boxes of `a` and `b` (negative when
/// they overlap, as Bangla fragments do).
fn gap_along(a: &LineInfo, b: &LineInfo) -> f32 {
    let extent = |l: &LineInfo| {
        let u = a.along(Point::new(l.bbox.x0, l.bbox.y0));
        let v = a.along(Point::new(l.bbox.x1, l.bbox.y1));
        (u.min(v), u.max(v))
    };
    let ((a0, a1), (b0, b1)) = (extent(a), extent(b));
    (b0 - a1).max(a0 - b1)
}

/// Combine the text pass and the geometry pass, then sort top to bottom.
///
/// With `/ActualText` (how Akkhor stores shaped Bangla) the text pass gives
/// the right characters but places them only roughly; the geometry pass
/// ignores ActualText and sees every glyph where it is really drawn. So the
/// rows come from the geometry pass, and each piece of text from the text
/// pass joins the row on its baseline that its first character falls in.
fn combine(text_frags: Vec<LineInfo>, geometry_frags: Vec<LineInfo>) -> Vec<LineInfo> {
    let geometry = merge_rows(geometry_frags);
    let mut texts: Vec<Option<String>> = vec![None; geometry.len()];
    let mut orphans = Vec::new();
    for f in text_frags {
        let home = geometry.iter().position(|g| {
            let pad = g.size.max(1.0);
            let at = g.along(f.origin);
            let (a, b) = (g.along(Point::new(g.bbox.x0, g.bbox.y0)), g.along(Point::new(g.bbox.x1, g.bbox.y1)));
            same_row(g, &f) && at >= a.min(b) - pad && at <= a.max(b) + pad
        });
        match home {
            Some(i) => texts[i].get_or_insert_with(String::new).push_str(&f.text),
            None => orphans.push(f),
        }
    }
    let mut rows: Vec<LineInfo> = geometry
        .into_iter()
        .zip(texts)
        .map(|(mut g, t)| {
            if let Some(t) = t {
                g.text = t;
            }
            g
        })
        .collect();
    // Text with no drawn glyphs under it (rare) is kept as it was read.
    rows.extend(merge_rows(orphans));
    // Top to bottom, then left to right. Content added later (for example an
    // edited line) is drawn last but read in its place.
    // Pages turned by 90 degrees read the same way in their own direction.
    rows.sort_by(|a, b| {
        if a.angle != b.angle {
            a.angle.cmp(&b.angle)
        } else if same_row(a, b) {
            a.along(a.origin).total_cmp(&a.along(b.origin))
        } else {
            a.across(a.origin).total_cmp(&a.across(b.origin))
        }
    });
    rows
}

/// All visual lines on one page of an open document, in reading order.
pub fn page_lines(doc: &Document, page_no: i32) -> Result<Vec<LineInfo>> {
    let page = doc.load_page(page_no)?;
    Ok(combine(
        rows_of(&page.to_text_page(TEXT_FLAGS)?),
        rows_of(&page.to_text_page(GEOMETRY_FLAGS)?),
    ))
}

/// All visual lines of a recorded page (safe to call from any thread).
pub fn display_list_lines(list: &DisplayList) -> Result<Vec<LineInfo>> {
    Ok(combine(
        rows_of(&list.to_text_page(TEXT_FLAGS)?),
        rows_of(&list.to_text_page(GEOMETRY_FLAGS)?),
    ))
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
