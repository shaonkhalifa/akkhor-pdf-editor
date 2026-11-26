//! Turn Bangla + English text into a PDF whose text is shaped by HarfBuzz.
//!
//! MuPDF's HTML engine shapes every text run with HarfBuzz. We build a tiny
//! HTML page in which each run gets the right font (SRS FR-6.4, FR-6.10), let
//! MuPDF lay it out and write it as PDF, then tag every run with its original
//! Unicode text (`/ActualText`) so copy, search and text extraction return
//! exactly what was typed (SRS 6.4, FR-2.6).
//!
//! Why the tagging is needed: a shaped Bangla conjunct such as ক্ষ্ম is one
//! glyph that stands for several characters, and pre-base vowels such as ে are
//! drawn before their consonant. MuPDF's PDF writer maps glyphs back to text
//! one-to-one, which loses both. `/ActualText` is the PDF standard's answer and
//! is honoured by Adobe Reader, Chrome and MuPDF.
//!
//! Phase 1 lays every line out on one row (no wrapping). Wrapping long text
//! inside a box is Phase 5 work (FR-6.12).

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{bail, Context, Result};
use mupdf::pdf::PdfDocument;
use mupdf::{Buffer, Document, DocumentWriter, Matrix};

use super::fonts::{BanglaFont, Coverage};
use super::script::{split_runs, Script};

/// Page and text settings for a generated PDF.
#[derive(Debug, Clone)]
pub struct LayoutOptions {
    /// Page width in points (1/72 inch).
    pub page_width: f32,
    /// Page height in points.
    pub page_height: f32,
    /// Margin on every side, in points.
    pub margin: f32,
    /// Font size in points.
    pub font_size: f32,
    pub bold: bool,
    pub bangla_font: BanglaFont,
}

impl LayoutOptions {
    /// An A4 page (595 × 842 pt) with 40 pt margins and 12 pt text.
    pub fn a4(bangla_font: BanglaFont) -> Self {
        Self {
            page_width: 595.0,
            page_height: 842.0,
            margin: 40.0,
            font_size: 12.0,
            bold: false,
            bangla_font,
        }
    }
}

/// A run of text drawn with one font: "bn" (Bangla font) or "en" (English font).
pub type Span = (&'static str, String);

fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Which font class ("bn" or "en") each character is drawn with.
///
/// Bangla characters use the Bangla font and English characters the English
/// font. A neutral character (space, digit, punctuation) stays in its run's
/// font only if that font really has a glyph for it; otherwise it falls back
/// to the English font. Noto Sans Bengali, for example, has no ASCII digits.
fn classes_for_line(line: &str, bangla: &Coverage) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    for run in split_runs(line) {
        for c in run.text.chars() {
            let class = match run.script {
                Script::Bangla if bangla.has(c) || c == '\u{200C}' || c == '\u{200D}' => "bn",
                Script::Bangla if c.is_whitespace() => "bn",
                _ => "en",
            };
            match spans.last_mut() {
                Some((k, text)) if *k == class => text.push(c),
                _ => spans.push((class, c.to_string())),
            }
        }
    }
    spans
}

/// Build the HTML document for `lines`. Returns the HTML and, for each line,
/// the font runs it was split into (used later for `/ActualText`).
pub fn build_html(
    lines: &[&str],
    opts: &LayoutOptions,
    bangla: &Coverage,
    fonts_dir: &Path,
) -> (String, Vec<Vec<Span>>) {
    let f = &opts.bangla_font;
    let mut css = String::new();
    let _ = writeln!(
        css,
        "@font-face {{ font-family: 'AkkhorBangla'; font-weight: normal; src: url('{}'); }}",
        font_url(fonts_dir, f.regular)
    );
    if let Some(bold) = f.bold {
        let _ = writeln!(
            css,
            "@font-face {{ font-family: 'AkkhorBangla'; font-weight: bold; src: url('{}'); }}",
            font_url(fonts_dir, bold)
        );
    }
    let _ = write!(
        css,
        "body {{ margin: {m}pt; font-size: {s}pt; font-weight: {w}; line-height: 1.6; }}\n\
         p {{ margin: 0 0 {gap}pt 0; white-space: pre; }}\n\
         .bn {{ font-family: 'AkkhorBangla'; }}\n\
         .en {{ font-family: sans-serif; }}\n",
        m = opts.margin,
        s = opts.font_size,
        w = if opts.bold { "bold" } else { "normal" },
        gap = opts.font_size * 0.4,
    );

    let mut body = String::new();
    let mut all_spans = Vec::with_capacity(lines.len());
    for line in lines {
        let spans = classes_for_line(line, bangla);
        body.push_str("<p>");
        for (class, text) in &spans {
            let _ = write!(body, "<span class=\"{class}\">{}</span>", escape_html(text));
        }
        body.push_str("</p>\n");
        all_spans.push(spans);
    }

    let html = format!(
        "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\"><style>\n{css}</style></head>\n<body>\n{body}</body></html>\n"
    );
    (html, all_spans)
}

/// Lay out `lines` and return the bytes of a new PDF.
pub fn lines_to_pdf(lines: &[&str], opts: &LayoutOptions, fonts_dir: &Path) -> Result<Vec<u8>> {
    let font = &opts.bangla_font;
    let coverage = Coverage::load(&fonts_dir.join(font.regular))?;
    let (html, spans) = build_html(lines, opts, &coverage, fonts_dir);

    // MuPDF reads the HTML from a file; the PDF it writes goes next to it.
    let work = tempfile::tempdir().context("creating temp folder")?;
    let html_path = work.path().join("page.html");
    std::fs::write(&html_path, html)?;
    let pdf_path = work.path().join("out.pdf");

    let mut doc = Document::open(&mupdf_path(&html_path)?).context("opening HTML with MuPDF")?;
    doc.layout(opts.page_width, opts.page_height, opts.font_size)?;
    {
        let mut writer = DocumentWriter::new(&mupdf_path(&pdf_path)?, "pdf", "")?;
        for i in 0..doc.page_count()? {
            let page = doc.load_page(i)?;
            let bounds = page.bounds()?;
            let device = writer.begin_page(bounds)?;
            page.run(&device, &Matrix::IDENTITY)?;
            writer.end_page(device)?;
        }
        // Dropping the writer closes the file.
    }
    let bytes = std::fs::read(&pdf_path).context("reading generated PDF")?;
    let runs: Vec<Span> = spans.into_iter().flatten().filter(|(_, t)| !t.is_empty()).collect();
    add_actual_text(&bytes, &runs)
}

/// A valid ToUnicode CMap with no mappings.
const EMPTY_TO_UNICODE: &str = "/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def
/CMapName /Akkhor-ActualText-Only def
/CMapType 2 def
1 begincodespacerange
<0000> <FFFF>
endcodespacerange
endcmap
CMapName currentdict /CMap defineresource pop
end
end
";

/// Encode text as a PDF text string: UTF-16BE with a byte-order mark, in hex.
fn pdf_utf16_hex(text: &str) -> String {
    let mut hex = String::from("<FEFF");
    for unit in text.encode_utf16() {
        let _ = write!(hex, "{unit:04X}");
    }
    hex.push('>');
    hex
}

/// Tag every text-drawing operator in the page content streams with
/// `/Span <</ActualText (original text)>> BDC … EMC`.
///
/// MuPDF writes one operator per line. A new font run starts when the font
/// changes (`Tf`) or the text moves to a new row (`Tm` with a different y),
/// because neighbouring runs always use different fonts and every line is one
/// row. A run may be drawn by several `TJ` operators (marks are nudged with
/// `Td` between them). Each `TJ` gets its own tag: the run's first `TJ`
/// carries the run's whole text and the others carry an empty text. Readers
/// (including MuPDF's own text extraction) then output the text exactly once,
/// in typing order, without mixing characters between operators.
pub fn add_actual_text(pdf: &[u8], runs: &[Span]) -> Result<Vec<u8>> {
    let mut doc = PdfDocument::from_bytes(pdf)?;
    let mut run_index: usize = 0;

    for page_no in 0..doc.page_count()? {
        let page = doc.load_pdf_page(page_no)?;
        let Some(mut contents) = page.contents()? else { continue };
        if contents.is_array()? {
            bail!("ActualText tagging expects one content stream per page");
        }
        let source = String::from_utf8_lossy(&contents.read_stream()?).into_owned();

        let mut out = String::with_capacity(source.len() * 2);
        let mut font = String::new();
        let mut y: Option<String> = None;
        // Text of the current run that has not been attached to a TJ yet.
        let mut pending: Option<&str> = None;
        // Font resources (e.g. "F1") used for Bangla runs on this page.
        let mut bangla_fonts: Vec<String> = Vec::new();
        let mut fresh = false; // a run started and nothing has been drawn yet

        for line in source.lines() {
            let trimmed = line.trim();
            let op = trimmed.rsplit(|c: char| c.is_whitespace() || c == ']').next().unwrap_or("");
            let mut starts_run = false;
            match op {
                "Tf" => {
                    let name = trimmed.split_whitespace().next().unwrap_or("").to_string();
                    if name != font {
                        starts_run = true;
                        font = name;
                    }
                }
                "Tm" => {
                    let new_y = trimmed.split_whitespace().nth(5).map(str::to_string);
                    if new_y != y && !fresh {
                        starts_run = true;
                    }
                    y = new_y;
                }
                _ => {}
            }

            if starts_run {
                let (class, text) = runs.get(run_index).map(|(c, t)| (*c, t.as_str())).unwrap_or(("", ""));
                if class == "bn" {
                    let key = font.trim_start_matches('/').to_string();
                    if !bangla_fonts.contains(&key) {
                        bangla_fonts.push(key);
                    }
                }
                pending = Some(text);
                run_index += 1;
                fresh = true;
            }

            if op == "TJ" || op == "Tj" {
                let text = pending.take().unwrap_or("");
                let _ = writeln!(out, "/Span <</ActualText {}>> BDC", pdf_utf16_hex(text));
                out.push_str(line);
                out.push_str("\nEMC\n");
                fresh = false;
            } else {
                out.push_str(line);
                out.push('\n');
            }
        }
        contents.write_stream_buffer(&Buffer::from_bytes(out.as_bytes())?)?;

        // Work around a MuPDF text-extraction bug: when the last glyph of a
        // tagged run maps to the same letter as the end of its ActualText,
        // MuPDF counts the rest in bytes, not characters, and repeats Bangla
        // letters (3 bytes each in UTF-8). The Bangla font's glyph-to-Unicode
        // map is wrong for shaped text anyway (conjuncts, reordered vowels),
        // so we give it an empty map and let ActualText carry all the text.
        if !bangla_fonts.is_empty() {
            let fonts = page.resources()?.get_dict("Font")?;
            for key in &bangla_fonts {
                if let Some(mut font_dict) = fonts.as_ref().and_then(|f| f.get_dict(key.as_str()).ok().flatten()) {
                    let cmap = doc.add_stream(&Buffer::from_bytes(EMPTY_TO_UNICODE.as_bytes())?, None, false)?;
                    font_dict.dict_put("ToUnicode", cmap)?;
                }
            }
        }
    }

    if run_index != runs.len() {
        bail!(
            "ActualText tagging matched {run_index} font runs but the text has {}; layout changed unexpectedly",
            runs.len()
        );
    }
    let mut bytes = Vec::new();
    doc.write_to(&mut bytes)?;
    Ok(bytes)
}

/// Absolute font path for CSS `url('…')`. MuPDF opens a @font-face source
/// that is not inside the document's own folder as a plain file path, so an
/// absolute path works wherever the HTML file lives.
fn font_url(fonts_dir: &Path, file: &str) -> String {
    let full = fonts_dir.join(file);
    let full = std::path::absolute(&full).unwrap_or(full);
    full.to_string_lossy().replace('\\', "/").replace('\'', "%27")
}

/// A path for MuPDF: absolute, UTF-8, with forward slashes. MuPDF finds a
/// file's folder by looking for '/', so a Windows path with '\' would make it
/// fall back to the current folder, which may not even be readable.
pub(crate) fn mupdf_path(p: &Path) -> Result<String> {
    let abs = std::path::absolute(p).context("making path absolute")?;
    let s = abs.to_str().context("path is not valid UTF-8")?;
    Ok(s.replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::fonts::{default_bangla, dev_fonts_dir};

    #[test]
    fn digits_fall_back_when_bangla_font_lacks_them() {
        let f = default_bangla();
        let cov = Coverage::load(&dev_fonts_dir().join(f.regular)).unwrap();
        let spans = classes_for_line("মোট ১২ (12)", &cov);
        // Bangla digits stay in the Bangla font; ASCII digits and brackets do not.
        assert!(spans.iter().any(|(k, t)| *k == "bn" && t.contains("১২")));
        assert!(spans.iter().any(|(k, t)| *k == "en" && t.contains("12")));
    }

    #[test]
    fn html_escapes_markup() {
        let f = default_bangla();
        let cov = Coverage::load(&dev_fonts_dir().join(f.regular)).unwrap();
        let (html, _) = build_html(&["a<b>&c"], &LayoutOptions::a4(f), &cov, &dev_fonts_dir());
        assert!(html.contains("a&lt;b&gt;&amp;c"));
    }

    #[test]
    fn mupdf_paths_are_absolute_with_forward_slashes() {
        // Regression: the installed app failed with "'.' is not a directory"
        // because MuPDF could not find the folder of a backslash path.
        let p = mupdf_path(Path::new("some\\dir\\page.html")).unwrap();
        assert!(!p.contains('\\'), "{p}");
        assert!(Path::new(&p).is_absolute(), "{p}");
        assert!(p.ends_with("some/dir/page.html"), "{p}");
    }

    #[test]
    fn utf16_hex_has_bom() {
        assert_eq!(pdf_utf16_hex("কA"), "<FEFF0995 0041>".replace(' ', ""));
    }
}
