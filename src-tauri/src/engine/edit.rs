//! Phase 1 text-edit spike: replace one line of existing text in a PDF.
//!
//! Steps (SRS section 3.6):
//! 1. Find the text line that contains the old words.
//! 2. Remove only that text with a redaction (images and lines stay).
//! 3. Lay out the new line with HarfBuzz (see `layout`) in a small PDF.
//! 4. Copy that small page into the original page as a Form XObject, placed
//!    on the old line's baseline.
//!
//! Limits of the spike: the page must not be rotated and its MediaBox must
//! start at (0, 0); the new text is black; long text is not wrapped yet
//! (FR-6.12 comes in Phase 5).

use anyhow::{anyhow, bail, Context, Result};
use mupdf::pdf::{
    PdfDocument, PdfObject, PdfRedactImageMethod, PdfRedactLineArtMethod, PdfRedactOptions,
    PdfRedactTextMethod,
};
use mupdf::Buffer;

use super::extract::page_lines;
use super::fonts::BanglaFont;
use super::layout::{lines_to_pdf, LayoutOptions};

/// Replace the first occurrence of `old` on page `page_no` with `new`.
/// The rest of that line is kept. Returns the bytes of the edited PDF.
pub fn replace_text(
    pdf: &[u8],
    page_no: i32,
    old: &str,
    new: &str,
    font: BanglaFont,
    fonts_dir: &std::path::Path,
) -> Result<Vec<u8>> {
    let mut doc = PdfDocument::from_bytes(pdf).context("opening PDF")?;

    // 1. Find the line.
    let lines = page_lines(&doc, page_no)?;
    let found: Vec<String> = lines.iter().map(|l| l.text.clone()).collect();
    let target = lines
        .into_iter()
        .find(|l| l.text.contains(old))
        .ok_or_else(|| anyhow!("text {old:?} not found on page {}; lines there: {found:?}", page_no + 1))?;
    let new_line = target.text.replacen(old, new, 1);

    let mut page = doc.load_pdf_page(page_no)?;
    if page.rotation()? != 0 {
        bail!("rotated pages are not supported by the Phase 1 spike");
    }
    let page_box = page.bounds()?;
    let page_h = page_box.y1 - page_box.y0;

    // 2. Remove the old text only.
    page.add_redact_annotation(target.bbox)?;
    page.apply_redactions_with_options(PdfRedactOptions {
        black_boxes: false,
        image_method: PdfRedactImageMethod::None,
        line_art: PdfRedactLineArtMethod::None,
        text: PdfRedactTextMethod::Remove,
    })?;

    // 3. Lay out the new line in a small page with room to spare.
    let size = if target.size > 0.0 { target.size } else { 12.0 };
    let opts = LayoutOptions {
        page_width: (page_box.x1 - target.bbox.x0).max(size * 4.0),
        page_height: size * 3.0,
        margin: 0.0,
        font_size: size,
        bold: false,
        bangla_font: font,
    };
    let rep_bytes = lines_to_pdf(&[&new_line], &opts, fonts_dir)?;
    let rep = PdfDocument::from_bytes(&rep_bytes)?;
    let rep_line = page_lines(&rep, 0)?
        .into_iter()
        .next()
        .context("replacement text produced no line")?;

    // 4. Turn the replacement page into a Form XObject in the original file.
    let rep_page = rep.load_pdf_page(0)?;
    // Use the replacement page's real size: MuPDF adds its own page margins.
    let rep_box = rep_page.bounds()?;
    let (rep_w, rep_h) = (rep_box.x1 - rep_box.x0, rep_box.y1 - rep_box.y0);
    let rep_res = rep
        .find_page(0)?
        .get_dict_inheritable("Resources")?
        .context("replacement page has no resources")?;
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

    let mut resources = page.resources()?;
    let mut xobjects = match resources.get_dict("XObject")? {
        Some(d) => d,
        None => {
            resources.dict_put("XObject", doc.new_dict()?)?;
            resources.get_dict("XObject")?.context("creating XObject dictionary")?
        }
    };
    let name = (1..)
        .map(|i| format!("AkkhorEdit{i}"))
        .find(|n| matches!(xobjects.get_dict(n.as_str()), Ok(None)))
        .expect("an unused name");
    xobjects.dict_put(name.as_str(), xobj)?;

    // Align the new baseline origin with the old one. Text-page coordinates
    // have y pointing down; PDF user space has y pointing up.
    let tx = target.origin.x - rep_line.origin.x;
    let ty = (page_h - target.origin.y) - (rep_h - rep_line.origin.y);
    page.wrap_contents(&mut doc)?;
    page.insert_contents(
        &mut doc,
        format!("q 1 0 0 1 {tx:.3} {ty:.3} cm /{name} Do Q\n").as_bytes(),
        true,
    )?;

    let mut out = Vec::new();
    doc.write_to(&mut out)?;
    Ok(out)
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
