//! OCR: reading text from scanned pages (SRS Module 10).
//!
//! Tesseract and Leptonica are compiled into MuPDF (from the sources MuPDF
//! ships), so recognition runs inside the app, on this PC. The language
//! data (Bangla + English, about 26 MB) is not in the app: it comes as a
//! separate offline installer (FR-10.3) that puts the files in
//! `%LOCALAPPDATA%\Akkhor PDF Editor OCR\tessdata`. The app never downloads.
//!
//! "Make searchable" (FR-10.2) runs a page through MuPDF's PDF+OCR writer:
//! the page is drawn as a picture and the recognised words are laid under
//! it as invisible text, so the page looks the same but can be searched,
//! selected and copied.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use mupdf::pdf::PdfDocument;
use mupdf::{DocumentWriter, Matrix, Rect};
use serde::Serialize;

use super::extract::page_lines;
use super::pages::insert_document;

/// Folder name used by the OCR language pack installer.
pub const PACK_DIR: &str = "Akkhor PDF Editor OCR";

#[derive(Debug, Clone, Serialize)]
pub struct OcrStatus {
    /// Where the language data was found, if anywhere.
    pub datadir: Option<String>,
    /// Languages installed, e.g. ["ben", "eng"].
    pub languages: Vec<String>,
}

/// Places the language data may be: the pack's folder for this user, for
/// all users, next to the app, or `extra` (tests and development).
fn candidates(extra: &[PathBuf]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = extra.to_vec();
    if let Some(d) = std::env::var_os("AKKHOR_TESSDATA") {
        dirs.push(PathBuf::from(d));
    }
    for var in ["LOCALAPPDATA", "ProgramData"] {
        if let Some(base) = std::env::var_os(var) {
            dirs.push(PathBuf::from(base).join(PACK_DIR).join("tessdata"));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.join("tessdata"));
        }
    }
    dirs
}

fn languages_in(dir: &Path) -> Vec<String> {
    let mut langs: Vec<String> = std::fs::read_dir(dir)
        .map(|it| {
            it.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().into_owned();
                    name.strip_suffix(".traineddata").map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    langs.sort();
    langs
}

/// Is the language data installed, and where?
pub fn status(extra: &[PathBuf]) -> OcrStatus {
    for dir in candidates(extra) {
        let langs = languages_in(&dir);
        if !langs.is_empty() {
            return OcrStatus { datadir: Some(dir.to_string_lossy().into_owned()), languages: langs };
        }
    }
    OcrStatus { datadir: None, languages: Vec::new() }
}

/// The languages to use: Bangla and English where installed.
fn language_list(dir: &Path) -> Result<String> {
    let have = languages_in(dir);
    let wanted: Vec<&str> = ["ben", "eng"].into_iter().filter(|l| have.iter().any(|h| h == l)).collect();
    if wanted.is_empty() {
        bail!("OCR_DATA_MISSING");
    }
    Ok(wanted.join("+"))
}

/// Run part of a page (or all of it) through the PDF+OCR writer. Returns the
/// one-page PDF it writes: the area as a picture with invisible text.
fn ocr_pdf(doc: &PdfDocument, page_no: i32, area: Option<Rect>, datadir: &Path, dpi: u32) -> Result<Vec<u8>> {
    let langs = language_list(datadir)?;
    let page = doc.load_page(page_no)?;
    let bounds = page.bounds()?;
    let area = area.unwrap_or(bounds);
    let size = Rect::new(0.0, 0.0, area.x1 - area.x0, area.y1 - area.y0);
    if size.width() < 2.0 || size.height() < 2.0 {
        bail!("the area is too small to read");
    }
    let work = tempfile::tempdir()?;
    let out = work.path().join("ocr.pdf");
    // MuPDF splits options at commas; paths use forward slashes.
    let dir = datadir.to_string_lossy().replace('\\', "/");
    if dir.contains(',') {
        bail!("the OCR data folder name must not contain a comma");
    }
    let options = format!("compression=flate,resolution={dpi},ocr-language={langs},ocr-datadir={dir}");
    {
        let mut writer = DocumentWriter::new(&out.to_string_lossy().replace('\\', "/"), "ocr", &options)
            .map_err(|e| anyhow!("OCR is not available in this build ({e})"))?;
        let device = writer.begin_page(size)?;
        page.run(&device, &Matrix::new_translate(bounds.x0 - area.x0, bounds.y0 - area.y0))?;
        writer.end_page(device)?;
    }
    std::fs::read(&out).context("reading the OCR result")
}

/// Recognise the text in an area of a page (FR-6.11: "OCR this block").
pub fn read_area(doc: &PdfDocument, page_no: i32, rect: [f32; 4], datadir: &Path) -> Result<String> {
    let area = Rect::new(rect[0].min(rect[2]), rect[1].min(rect[3]), rect[0].max(rect[2]), rect[1].max(rect[3]));
    let pdf = ocr_pdf(doc, page_no, Some(area), datadir, 300)?;
    let result = PdfDocument::from_bytes(&pdf)?;
    let lines: Vec<String> = page_lines(&result, 0)?.into_iter().map(|l| l.text.trim().to_string()).filter(|t| !t.is_empty()).collect();
    Ok(lines.join("\n"))
}

/// Replace a page with a searchable copy: the page as a picture at `dpi`,
/// with the recognised text invisible underneath (FR-10.1, FR-10.2).
/// Returns the number of words found.
pub fn make_searchable(doc: &mut PdfDocument, page_no: i32, datadir: &Path, dpi: u32) -> Result<usize> {
    let pdf = ocr_pdf(doc, page_no, None, datadir, dpi.clamp(100, 600))?;
    let result = PdfDocument::from_bytes(&pdf)?;
    let words: usize = page_lines(&result, 0)?.iter().map(|l| l.text.split_whitespace().count()).sum();
    insert_document(doc, page_no, &result)?;
    doc.delete_page(page_no + 1)?;
    Ok(words)
}
