//! Editing text on PDF pages (SRS Module 6, the core feature).
//!
//! Editing existing text (section 3.6):
//! 1. `text_blocks` groups the lines of a page into blocks (paragraphs) and
//!    reports their text, position, font, size and colour.
//! 2. The user edits the text; `place_text` removes only the old text (a
//!    redaction that leaves images and lines alone), lays out the new text
//!    with HarfBuzz in the chosen fonts, wraps it inside the block's width
//!    (FR-6.12) and puts it where the old text was. New fonts are embedded as
//!    fresh subsets, and every run carries `/ActualText`, so copy and search
//!    give back exactly what was typed (FR-6.9).
//!
//! The same code adds new text boxes (FR-6.3). Highlight, underline,
//! strikethrough (FR-6.7) and permanent redaction (FR-6.8) are here too.

use anyhow::{anyhow, bail, Context, Result};
use mupdf::pdf::{
    PdfDocument, PdfObject, PdfRedactImageMethod, PdfRedactLineArtMethod, PdfRedactOptions, PdfRedactTextMethod,
};
use mupdf::text_page::TextPageFlags;
use mupdf::color::AnnotationColor;
use mupdf::{Buffer, Matrix, Quad, Rect};
use serde::{Deserialize, Serialize};

use super::extract::{page_lines, LineInfo};
use super::layout::{styled_pdf, Style};
use super::script::{classify, Script};

// ---------------------------------------------------------------------------
// Reading blocks
// ---------------------------------------------------------------------------

/// A block of text on a page, ready to edit.
#[derive(Debug, Clone, Serialize)]
pub struct TextBlock {
    /// Lines joined with "\n".
    pub text: String,
    /// x0, y0, x1, y1 around all lines, in page points.
    pub rect: [f32; 4],
    /// Each line's box: the parts removed when the block is replaced.
    pub lines: Vec<[f32; 4]>,
    /// Baseline of the first line: x (left edge of the text) and y.
    pub origin: [f32; 2],
    pub size: f32,
    /// Distance between baselines (the font size × 1.2 for one line).
    pub line_height: f32,
    /// How wide the text may become: a paragraph keeps its width; a single
    /// line may grow to the page margin or to the next text on its right.
    pub max_width: f32,
    /// The font as named in the PDF, and its style.
    pub font: String,
    pub bold: bool,
    pub italic: bool,
    /// #rrggbb
    pub color: String,
    /// Suggested fonts for editing (SRS 6.3).
    pub bangla_font: String,
    pub english_font: String,
    pub has_bangla: bool,
    /// False when the text was probably not read correctly (a broken font
    /// map, or legacy Bijoy encoding); the user should check or retype it.
    pub readable: bool,
}

/// Does this look like text that was not decoded properly?
fn looks_unreadable(text: &str, font: &str) -> bool {
    let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    if chars.is_empty() {
        return false;
    }
    let bad = chars
        .iter()
        .filter(|&&c| c == '\u{FFFD}' || ('\u{E000}'..='\u{F8FF}').contains(&c) || (c as u32) < 0x20)
        .count();
    // Bijoy fonts (SutonnyMJ and other "MJ" fonts) store Bangla as Latin
    // letters and symbols such as ‡ ¨ Š.
    let lower = font.to_lowercase();
    let bijoy_font = lower.contains("sutonny") || lower.ends_with("mj") || lower.contains("mj-") || lower.contains("mj,");
    // Marks Bijoy text is full of but English text rarely uses (not © ® °,
    // dashes or curly quotes).
    let bijoy_marks = chars.iter().filter(|&&c| "‡†ˆ‰Š‹ŒŽš›œžŸ¨ª«¬¯±²³´µ¶·¸¹º»¼½¾¿".contains(c)).count();
    let has_bangla = chars.iter().any(|&c| classify(c) == Script::Bangla);
    let bijoy_text = !has_bangla && bijoy_marks >= 2 && bijoy_marks * 8 >= chars.len();
    bad * 10 > chars.len() || bijoy_font || bijoy_text
}

fn hex(argb: u32) -> String {
    format!("#{:06x}", argb & 0x00FF_FFFF)
}

fn same_block(prev: &LineInfo, next: &LineInfo) -> bool {
    let size = prev.size.max(next.size).max(1.0);
    let step = next.origin.y - prev.origin.y;
    let similar_size = (prev.size - next.size).abs() <= 0.2 * size;
    let overlap = next.bbox.x0.min(prev.bbox.x1.max(next.bbox.x1)) < prev.bbox.x1 && next.bbox.x1 > prev.bbox.x0;
    next.angle == 0 && similar_size && overlap && step > 0.7 * size && step < 1.9 * size
}

/// The text blocks on a page (upright text only).
pub fn text_blocks(doc: &PdfDocument, page_no: i32) -> Result<Vec<TextBlock>> {
    let lines: Vec<LineInfo> = page_lines(doc, page_no)?
        .into_iter()
        .filter(|l| l.angle == 0 && !l.text.trim().is_empty())
        .collect();
    let mut groups: Vec<Vec<LineInfo>> = Vec::new();
    for line in lines {
        // Join the most recent group this line continues (text in columns
        // is read row by row, so the previous line may be in another column).
        if let Some(g) = groups.iter_mut().rev().take(4).find(|g| same_block(g.last().unwrap(), &line)) {
            g.push(line);
        } else {
            groups.push(vec![line]);
        }
    }
    let page_w = {
        let b = doc.load_pdf_page(page_no)?.bounds()?;
        b.x1 - b.x0
    };
    let mut blocks: Vec<TextBlock> = groups.into_iter().map(block_of).collect();
    // Assume the right margin matches the left one (the usual layout).
    let left_margin = blocks.iter().map(|b| b.rect[0]).fold(f32::INFINITY, f32::min).max(18.0);
    let right_limit = (page_w - left_margin).max(blocks.iter().map(|b| b.rect[2]).fold(0.0, f32::max));
    let rects: Vec<[f32; 4]> = blocks.iter().map(|b| b.rect).collect();
    for b in &mut blocks {
        let paragraph = b.lines.len() > 1;
        let mut right = if paragraph { b.rect[2] } else { right_limit };
        // Text to the right on the same rows (table cells, columns) is a wall.
        for r in &rects {
            let same_rows = r[1] < b.rect[3] && r[3] > b.rect[1];
            if same_rows && r[0] > b.rect[2] - 0.5 {
                right = right.min(r[0] - b.size * 0.8);
            }
        }
        b.max_width = (right.max(b.rect[2]) - b.origin[0] + 2.0).max(b.size * 2.0);
    }
    Ok(blocks)
}

fn block_of(lines: Vec<LineInfo>) -> TextBlock {
    let first = &lines[0];
    let mut rect = first.bbox;
    for l in &lines[1..] {
        rect = rect.union(&l.bbox);
    }
    let size = lines.iter().map(|l| l.size).fold(0.0, f32::max).max(1.0);
    let line_height = if lines.len() > 1 {
        (lines.last().unwrap().origin.y - first.origin.y) / (lines.len() - 1) as f32
    } else {
        size * 1.2
    };
    let text = lines.iter().map(|l| l.text.trim_end().to_string()).collect::<Vec<_>>().join("\n");
    let has_bangla = text.chars().any(|c| classify(c) == Script::Bangla);
    let (bangla_font, english_font) = super::fonts::match_original(&first.font.name, first.font.serif, first.font.mono);
    let left = lines.iter().map(|l| l.origin.x).fold(f32::INFINITY, f32::min);
    TextBlock {
        readable: !looks_unreadable(&text, &first.font.name),
        lines: lines.iter().map(|l| [l.bbox.x0, l.bbox.y0, l.bbox.x1, l.bbox.y1]).collect(),
        rect: [rect.x0, rect.y0, rect.x1, rect.y1],
        origin: [left, first.origin.y],
        size,
        line_height,
        max_width: 0.0,
        font: first.font.name.clone(),
        bold: first.font.bold,
        italic: first.font.italic,
        color: hex(first.argb),
        bangla_font,
        english_font,
        has_bangla,
        text,
    }
}

// ---------------------------------------------------------------------------
// Wrapping (FR-6.12)
// ---------------------------------------------------------------------------

/// Widths of each item (one line each), measured from the left edge to the
/// right edge of the drawn glyphs, with real shaping.
fn measure(items: &[String], style: &Style) -> Result<Vec<f32>> {
    let shown: Vec<String> = items.iter().filter(|s| !s.trim().is_empty()).cloned().collect();
    if shown.is_empty() {
        return Ok(vec![0.0; items.len()]);
    }
    let height = shown.len() as f32 * style.line_height * 1.2 + style.size * 4.0;
    let pdf = styled_pdf(&shown, style, 20000.0, height, false)?;
    let doc = PdfDocument::from_bytes(&pdf)?;
    if doc.page_count()? != 1 {
        bail!("text layout did not fit on one page");
    }
    let rows = page_lines(&doc, 0)?;
    // One row per item, top to bottom. Rows that merged or split mean the
    // measurement can't be trusted; then estimate from the character count.
    let widths: Vec<f32> = if rows.len() == shown.len() {
        rows.iter().map(|r| r.bbox.x1.max(0.0)).collect()
    } else {
        shown.iter().map(|s| s.chars().count() as f32 * style.size * 0.55).collect()
    };
    let mut it = widths.into_iter();
    Ok(items.iter().map(|s| if s.trim().is_empty() { 0.0 } else { it.next().unwrap_or(0.0) }).collect())
}

/// Break `text` into lines no wider than `width` (points). Paragraphs
/// ("\n") are kept; words break at spaces only. A word wider than the box
/// gets a line of its own.
pub fn wrap(text: &str, style: &Style, width: f32) -> Result<Vec<String>> {
    let paragraphs: Vec<Vec<&str>> = text.split('\n').map(|p| p.split_whitespace().collect()).collect();
    let mut words: Vec<String> = paragraphs.iter().flatten().map(|w| w.to_string()).collect();
    words.sort();
    words.dedup();
    let mut probe = words.clone();
    probe.push("x x".into());
    probe.push("xx".into());
    let measured = measure(&probe, style)?;
    let n = words.len();
    let space = (measured[n] - measured[n + 1]).max(style.size * 0.2);
    let width_of = |w: &str| words.binary_search_by(|x| x.as_str().cmp(w)).map(|i| measured[i]).unwrap_or(style.size);

    // First guess from word widths, then check the real widths of the lines
    // and move a last word down while any line is too wide.
    let mut lines: Vec<String> = Vec::new();
    for words in &paragraphs {
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }
        let mut line = String::new();
        let mut used = 0.0;
        for w in words {
            let ww = width_of(w);
            if !line.is_empty() && used + space + ww > width {
                lines.push(std::mem::take(&mut line));
                used = 0.0;
            }
            if !line.is_empty() {
                line.push(' ');
                used += space;
            }
            line.push_str(w);
            used += ww;
        }
        lines.push(line);
    }
    for _ in 0..6 {
        let real = measure(&lines, style)?;
        let Some(i) = real.iter().zip(&lines).position(|(w, l)| *w > width + 0.5 && l.contains(' ')) else { break };
        let (keep, moved) = lines[i].rsplit_once(' ').map(|(a, b)| (a.to_string(), b.to_string())).unwrap();
        lines[i] = keep;
        // The moved word starts the next line of the same paragraph, or a new line.
        let next_is_same_paragraph = i + 1 < lines.len() && !lines[i + 1].is_empty();
        if next_is_same_paragraph {
            lines[i + 1] = format!("{moved} {}", lines[i + 1]);
        } else {
            lines.insert(i + 1, moved);
        }
    }
    Ok(lines)
}

// ---------------------------------------------------------------------------
// Placing text
// ---------------------------------------------------------------------------

/// What to do when edited text needs more lines than its box has (FR-6.12).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// Stop with `TEXT_TOO_LONG:<lines needed>:<lines that fit>`.
    #[default]
    Check,
    /// Make the text smaller until it fits.
    Shrink,
    /// Let it run below the box.
    Overflow,
}

/// Where the text goes.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TextPlacement {
    /// Left edge of the text box, in page points.
    pub left: f32,
    /// Width of the box: text wraps inside it.
    pub width: f32,
    /// Baseline of the first line (editing existing text), or the top of
    /// the box (a new text box): exactly one is set.
    pub baseline: Option<f32>,
    pub top: Option<f32>,
    /// Height available, for the fit check (none for new boxes).
    pub max_height: Option<f32>,
    /// Boxes of the old text to remove first.
    #[serde(default)]
    pub remove: Vec<[f32; 4]>,
    #[serde(default)]
    pub fit: Fit,
}

/// Remove text (only text) inside the given boxes.
fn remove_text(doc: &mut PdfDocument, page_no: i32, boxes: &[[f32; 4]]) -> Result<()> {
    if boxes.is_empty() {
        return Ok(());
    }
    let mut page = doc.load_pdf_page(page_no)?;
    for b in boxes {
        // Trim the top and bottom a little so neighbouring lines (whose
        // descenders and accents reach into this box) are not touched.
        let h = b[3] - b[1];
        let r = Rect::new(b[0] - 0.5, b[1] + h * 0.15, b[2] + 0.5, b[3] - h * 0.15);
        page.add_redact_annotation(r)?;
    }
    page.apply_redactions_with_options(PdfRedactOptions {
        black_boxes: false,
        image_method: PdfRedactImageMethod::None,
        line_art: PdfRedactLineArtMethod::None,
        text: PdfRedactTextMethod::Remove,
    })?;
    Ok(())
}

/// Lay out `text` in `style` and put it on the page. Returns the number of
/// lines used.
pub fn place_text(doc: &mut PdfDocument, page_no: i32, text: &str, style: &Style, at: &TextPlacement) -> Result<usize> {
    if text.trim().is_empty() && at.remove.is_empty() {
        bail!("EMPTY_TEXT");
    }
    if !(at.width > 1.0) {
        bail!("the text box is too narrow");
    }
    let mut style = style.clone();
    let mut lines = wrap(text, &style, at.width)?;
    if let (Some(max_h), Fit::Check | Fit::Shrink) = (at.max_height, at.fit) {
        let fits = |n: usize, lh: f32| (n as f32 - 1.0) * lh + lh * 0.8 <= max_h + lh * 0.25;
        if !fits(lines.len(), style.line_height) {
            if at.fit == Fit::Check {
                let can = (((max_h + style.line_height * 0.25 - style.line_height * 0.8) / style.line_height) + 1.0).floor().max(1.0);
                bail!("TEXT_TOO_LONG:{}:{}", lines.len(), can as usize);
            }
            // Shrink in small steps until it fits (not below 4 pt).
            while !fits(lines.len(), style.line_height) && style.size > 4.0 {
                style.size *= 0.93;
                style.line_height *= 0.93;
                lines = wrap(text, &style, at.width)?;
            }
        }
    }

    remove_text(doc, page_no, &at.remove)?;
    if text.trim().is_empty() {
        return Ok(0); // Editing a block down to nothing just removes it.
    }

    let height = lines.len() as f32 * style.line_height + style.size * 2.0;
    let rep_bytes = styled_pdf(&lines, &style, at.width, height, true)?;
    let rep = PdfDocument::from_bytes(&rep_bytes)?;
    if rep.page_count()? != 1 {
        bail!("the new text did not fit on one layout page");
    }
    let first = page_lines(&rep, 0)?.into_iter().next().context("the new text produced no line")?;
    let rep_page = rep.load_pdf_page(0)?;
    let rep_box = rep_page.bounds()?;
    let (rep_w, rep_h) = (rep_box.x1 - rep_box.x0, rep_box.y1 - rep_box.y0);

    // Copy the laid-out page in as a Form XObject.
    let rep_res = rep.find_page(0)?.get_dict_inheritable("Resources")?.context("new text has no resources")?;
    let content = read_contents(rep_page.contents()?)?;
    let mut graft = doc.new_graft_map()?;
    let res = graft.graft_object(&rep_res)?;
    let mut bbox = doc.new_array()?;
    for v in [0.0, 0.0, rep_w, rep_h] {
        bbox.array_push(PdfObject::new_real(v)?)?;
    }
    let mut form = doc.new_dict()?;
    form.dict_put("Type", PdfObject::new_name("XObject")?)?;
    form.dict_put("Subtype", PdfObject::new_name("Form")?)?;
    form.dict_put("BBox", bbox)?;
    form.dict_put("Resources", res)?;
    let xobj = doc.add_stream(&Buffer::from_bytes(&content)?, Some(&form), false)?;

    let mut page = doc.load_pdf_page(page_no)?;
    let mut resources = page.resources()?;
    let mut xobjects = match resources.get_dict("XObject")? {
        Some(d) => d,
        None => {
            resources.dict_put("XObject", doc.new_dict()?)?;
            resources.get_dict("XObject")?.context("creating XObject dictionary")?
        }
    };
    let name = (1..)
        .map(|i| format!("AkkhorText{i}"))
        .find(|n| matches!(xobjects.get_dict(n.as_str()), Ok(None)))
        .expect("an unused name");
    xobjects.dict_put(name.as_str(), xobj)?;

    // Where the laid-out page's top-left goes on the page (y down).
    let x0 = at.left;
    let y0 = match (at.baseline, at.top) {
        (Some(base), _) => base - first.origin.y,
        // A new box: the tallest letters of the first line start at `top`.
        (None, Some(top)) => top - first.bbox.y0.min(first.origin.y - style.size * 0.8),
        (None, None) => bail!("the text needs a position"),
    };
    // Form space has y up: (x, y) lands at (x0 + x, y0 + rep_h - y), then
    // into the page's own coordinates (handles rotated pages too).
    let mut m = Matrix::new(1.0, 0.0, 0.0, -1.0, x0, y0 + rep_h);
    m.concat(page.ctm()?.invert().context("the page has no size")?);
    page.wrap_contents(doc)?;
    page.insert_contents(
        doc,
        format!("q {:.4} {:.4} {:.4} {:.4} {:.3} {:.3} cm /{name} Do Q\n", m.a, m.b, m.c, m.d, m.e, m.f).as_bytes(),
        true,
    )?;
    Ok(lines.len())
}

/// Read a page's /Contents (one stream or an array of streams) as bytes.
fn read_contents(contents: Option<PdfObject>) -> Result<Vec<u8>> {
    let Some(contents) = contents else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    if contents.is_array()? {
        for i in 0..contents.len()? {
            if let Some(s) = contents.get_array(i as i32)? {
                out.extend(s.read_stream()?);
                out.push(b'\n');
            }
        }
    } else {
        out = contents.read_stream()?;
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Highlight, underline, strikethrough (FR-6.7) and redaction (FR-6.8)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Markup {
    Highlight,
    Underline,
    Strike,
}

/// Mark the text inside `rect` (one box per line, around the glyphs whose
/// centre is inside). Returns how many lines were marked.
pub fn markup(doc: &mut PdfDocument, page_no: i32, kind: Markup, rect: [f32; 4], color: [u8; 3]) -> Result<usize> {
    let area = Rect::new(rect[0].min(rect[2]), rect[1].min(rect[3]), rect[0].max(rect[2]), rect[1].max(rect[3]));
    let mut page = doc.load_pdf_page(page_no)?;
    // Glyph positions as drawn (ignoring ActualText, which only groups text).
    let tp = page.to_text_page(TextPageFlags::IGNORE_ACTUALTEXT)?;
    let mut quads: Vec<Quad> = Vec::new();
    for block in tp.blocks() {
        for line in block.lines() {
            let mut hit: Option<Rect> = None;
            for ch in line.chars() {
                let q = ch.quad();
                let cx = (q.ul.x + q.ur.x + q.ll.x + q.lr.x) / 4.0;
                let cy = (q.ul.y + q.ur.y + q.ll.y + q.lr.y) / 4.0;
                if area.contains(cx, cy) {
                    let r = Rect::new(
                        q.ul.x.min(q.ll.x),
                        q.ul.y.min(q.ur.y),
                        q.ur.x.max(q.lr.x),
                        q.ll.y.max(q.lr.y),
                    );
                    hit = Some(hit.map_or(r, |h| h.union(&r)));
                }
            }
            if let Some(r) = hit {
                // Lines split by Bangla vowel reordering share a row: join them.
                if let Some(last) = quads.last_mut() {
                    let lr = Rect::new(last.ul.x, last.ul.y, last.lr.x, last.lr.y);
                    let same_row = (lr.y0 - r.y0).abs() < 0.3 * (r.y1 - r.y0) && (lr.y1 - r.y1).abs() < 0.3 * (r.y1 - r.y0);
                    if same_row {
                        *last = Quad::from(lr.union(&r));
                        continue;
                    }
                }
                quads.push(Quad::from(r));
            }
        }
    }
    if quads.is_empty() {
        bail!("NO_TEXT_THERE");
    }
    let count = quads.len();
    let mut annot = match kind {
        Markup::Highlight => page.add_highlight_annotation(quads)?,
        Markup::Underline => page.add_underline_annotation(quads)?,
        Markup::Strike => page.add_strikeout_annotation(quads)?,
    };
    let [r, g, b] = color.map(|c| c as f32 / 255.0);
    annot.set_color(AnnotationColor::Rgb { red: r, green: g, blue: b })?;
    annot.update()?;
    Ok(count)
}

/// Remove everything inside `rect` for good and cover it with a black box.
pub fn redact(doc: &mut PdfDocument, page_no: i32, rect: [f32; 4]) -> Result<()> {
    let area = Rect::new(rect[0].min(rect[2]), rect[1].min(rect[3]), rect[0].max(rect[2]), rect[1].max(rect[3]));
    if area.width() < 1.0 || area.height() < 1.0 {
        bail!("the area to black out is too small");
    }
    let mut page = doc.load_pdf_page(page_no)?;
    page.add_redact_annotation(area)?;
    let applied = page.apply_redactions_with_options(PdfRedactOptions {
        black_boxes: true,
        image_method: PdfRedactImageMethod::Pixels,
        line_art: PdfRedactLineArtMethod::RemoveIfTouched,
        text: PdfRedactTextMethod::Remove,
    })?;
    if !applied {
        return Err(anyhow!("the redaction could not be applied"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::looks_unreadable;

    #[test]
    fn spots_text_that_was_not_read_properly() {
        assert!(!looks_unreadable("আমি বাংলায় গান গাই।", "NotoSansBengali"));
        assert!(!looks_unreadable("Invoice No. 2024", "Arial"));
        assert!(looks_unreadable("Avwg evsjvq Mvb MvB", "SutonnyMJ"), "Bijoy font");
        assert!(looks_unreadable("‡mvbvi evsjv Šj¨", "Unknown"), "Bijoy-looking symbols");
        assert!(looks_unreadable("\u{FFFD}\u{FFFD}\u{FFFD}ab", "Broken"), "replacement characters");
    }
}
