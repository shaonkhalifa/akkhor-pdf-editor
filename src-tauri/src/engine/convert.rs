//! Create and convert (SRS Module 7): blank PDFs, pictures to PDF, pages to
//! pictures, compression and passwords.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use image::{DynamicImage, GrayImage, ImageFormat, RgbImage};
use mupdf::pdf::{Encryption, PdfDocument, PdfWriteOptions};
use mupdf::text_page::{TextBlockType, TextPageFlags};
use mupdf::{Colorspace, DisplayList, Image, Matrix, Size};
use serde::{Deserialize, Serialize};

use super::images::{self, Placement};
use super::pages::{save_options, to_bytes, unused_path, write_file_safely};

/// A new PDF with `count` empty pages of `size` points (FR-7.1).
pub fn blank_pdf(size: [f32; 2], count: u32) -> Result<Vec<u8>> {
    let (w, h) = (size[0].clamp(72.0, 14400.0), size[1].clamp(72.0, 14400.0));
    let mut doc = PdfDocument::new();
    for _ in 0..count.clamp(1, 500) {
        doc.new_page(Size::new(w, h))?;
    }
    to_bytes(&doc, save_options())
}

/// How pictures are put on pages.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PictureLayout {
    /// Each page is exactly the size of its picture (at 150 dpi).
    FitPicture,
    /// A4 pages (turned to landscape for wide pictures), the picture
    /// centred with a margin.
    A4,
}

/// One PDF with one picture per page, in the given order (FR-7.2).
pub fn pictures_to_pdf(files: &[PathBuf], layout: PictureLayout) -> Result<Vec<u8>> {
    if files.is_empty() {
        bail!("choose at least one picture");
    }
    let mut doc = PdfDocument::new();
    for (i, file) in files.iter().enumerate() {
        let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let prepared = images::prepare(&std::fs::read(file).with_context(|| format!("reading {name}"))?)
            .map_err(|e| anyhow!("{name}: {e:#}"))?;
        let (pw, ph) = (prepared.width as f32 * 72.0 / 150.0, prepared.height as f32 * 72.0 / 150.0);
        let (page, size) = match layout {
            PictureLayout::FitPicture => ([pw, ph], [pw, ph]),
            PictureLayout::A4 => {
                let page = if pw > ph { [842.0, 595.0] } else { [595.0, 842.0] };
                let margin = 28.0;
                // Fit inside the margins; small pictures are enlarged at most twice.
                let scale = ((page[0] - 2.0 * margin) / pw).min((page[1] - 2.0 * margin) / ph).min(2.0);
                (page, [pw * scale, ph * scale])
            }
        };
        doc.new_page(Size::new(page[0], page[1]))?;
        let at = Placement { center: [page[0] / 2.0, page[1] / 2.0], size, rotation: 0.0, opacity: 1.0 };
        images::place(&mut doc, i as i32, &prepared.bytes, &at)?;
    }
    to_bytes(&doc, save_options())
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PictureFormat {
    Png,
    Jpg,
}

/// Draw a recorded page as a PNG or JPEG picture at `dpi` (FR-7.3).
pub fn page_picture(list: &DisplayList, dpi: f32, format: PictureFormat) -> Result<Vec<u8>> {
    let scale = dpi.clamp(36.0, 600.0) / 72.0;
    let pixmap = list.to_pixmap(&Matrix::new_scale(scale, scale), &Colorspace::device_rgb(), false)?;
    let (w, h, n, stride) = (pixmap.width(), pixmap.height(), pixmap.n() as usize, pixmap.stride() as usize);
    let samples = pixmap.samples();
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h as usize {
        for px in samples[y * stride..y * stride + w as usize * n].chunks_exact(n) {
            rgb.extend_from_slice(&px[..3]);
        }
    }
    let img = RgbImage::from_raw(w, h, rgb).context("building the picture")?;
    let mut out = Cursor::new(Vec::new());
    match format {
        PictureFormat::Png => img.write_to(&mut out, ImageFormat::Png)?,
        PictureFormat::Jpg => image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90).encode_image(&img)?,
    }
    Ok(out.into_inner())
}

/// Where page pictures are written: "<stem> page N.png" in `dir`, never
/// overwriting existing files.
pub fn picture_path(dir: &Path, stem: &str, page: i32, format: PictureFormat) -> PathBuf {
    let ext = match format {
        PictureFormat::Png => "png",
        PictureFormat::Jpg => "jpg",
    };
    let first = dir.join(format!("{stem} page {}.{ext}", page + 1));
    if !first.exists() {
        return first;
    }
    (2..).map(|i| dir.join(format!("{stem} page {} ({i}).{ext}", page + 1))).find(|p| !p.exists()).expect("a free name")
}

/// How hard to compress (FR-7.4).
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Compression {
    /// Clean up and compress the file's structure; pictures untouched.
    Low,
    /// Also shrink pictures sharper than 150 dpi.
    Medium,
    /// Also shrink pictures sharper than 100 dpi, at lower JPEG quality.
    High,
}

/// Decode a PDF image into an `image` picture (grey or RGB only; others,
/// and pictures with transparency, are left alone).
fn decode(img: &Image) -> Option<DynamicImage> {
    if img.mask().is_some() {
        return None;
    }
    let pix = img.to_pixmap().ok()?;
    if pix.alpha() {
        return None;
    }
    let (w, h, n, stride) = (pix.width(), pix.height(), pix.n() as usize, pix.stride() as usize);
    let s = pix.samples();
    let mut buf = Vec::with_capacity((w * h) as usize * n);
    for y in 0..h as usize {
        buf.extend_from_slice(&s[y * stride..y * stride + w as usize * n]);
    }
    match n {
        1 => GrayImage::from_raw(w, h, buf).map(DynamicImage::ImageLuma8),
        3 => RgbImage::from_raw(w, h, buf).map(DynamicImage::ImageRgb8),
        _ => None,
    }
}

/// A smaller copy of the PDF in `source` (FR-7.4). Text stays text, so it
/// can still be searched and copied.
pub fn compress(source: &[u8], password: Option<&str>, level: Compression) -> Result<Vec<u8>> {
    let mut doc = PdfDocument::from_bytes(source)?;
    if let Some(pw) = password {
        if doc.needs_password()? {
            doc.authenticate(pw)?;
        }
    }
    let (max_dpi, quality) = match level {
        Compression::Low => (f32::INFINITY, 0),
        Compression::Medium => (150.0, 75),
        Compression::High => (100.0, 60),
    };
    if max_dpi.is_finite() {
        // Each image: the sharpest it is ever shown, and the image itself.
        let mut seen: HashMap<i32, (f32, Image)> = HashMap::new();
        for page_no in 0..doc.page_count()? {
            let page = doc.load_pdf_page(page_no)?;
            let known = page.images()?;
            let tp = page.to_text_page(TextPageFlags::PRESERVE_IMAGES)?;
            for block in tp.blocks() {
                if block.r#type() != TextBlockType::Image {
                    continue;
                }
                let Some(img) = block.image() else { continue };
                let same: Vec<i32> = known.iter().filter(|k| k.width == img.width() && k.height == img.height()).map(|k| k.xref).collect();
                let [xref] = same[..] else { continue };
                let b = block.bounds();
                let shown_in = ((b.x1 - b.x0) / 72.0).max((b.y1 - b.y0) / 72.0 * img.width() as f32 / img.height().max(1) as f32);
                if shown_in <= 0.0 {
                    continue;
                }
                let dpi = img.width() as f32 / shown_in;
                let entry = seen.entry(xref).or_insert((f32::INFINITY, img));
                entry.0 = entry.0.min(dpi);
            }
        }
        for (xref, (dpi, img)) in seen {
            if dpi <= max_dpi * 1.15 {
                continue;
            }
            let Some(pic) = decode(&img) else { continue };
            let factor = max_dpi / dpi;
            let (w, h) = (((pic.width() as f32) * factor).round().max(1.0) as u32, ((pic.height() as f32) * factor).round().max(1.0) as u32);
            let small = pic.resize_exact(w, h, image::imageops::FilterType::Triangle);
            let mut jpeg = Cursor::new(Vec::new());
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, quality).encode_image(&small.to_rgb8())?;
            let new = doc.add_image(&Image::from_bytes(jpeg.get_ref())?)?;
            images::replace_everywhere(&mut doc, xref, &new)?;
        }
    }
    let mut opts = PdfWriteOptions::default();
    opts.set_garbage_level(3).set_compress(true).set_compress_fonts(true).set_compress_images(true);
    let out = to_bytes(&doc, opts)?;
    // Never hand back a bigger file.
    if out.len() >= source.len() {
        let mut keep = PdfWriteOptions::default();
        keep.set_garbage_level(1);
        let tidy = to_bytes(&doc, keep)?;
        return Ok(if tidy.len() < out.len() { tidy } else { out });
    }
    Ok(out)
}

/// A copy of the PDF protected with a password (AES-256, FR-7.5). The owner
/// password (for changing permissions) is the same unless given.
pub fn protect(source: &[u8], password: Option<&str>, user: &str, owner: Option<&str>) -> Result<Vec<u8>> {
    if user.is_empty() {
        bail!("the password is empty");
    }
    let mut doc = PdfDocument::from_bytes(source)?;
    if let Some(pw) = password {
        if doc.needs_password()? {
            doc.authenticate(pw)?;
        }
    }
    let mut opts = save_options();
    opts.set_encryption(Encryption::Aes256).set_user_password(user).set_owner_password(owner.unwrap_or(user));
    to_bytes(&doc, opts)
}

/// A copy of the PDF without its password. The document must be open
/// (unlocked) already.
pub fn unprotect(source: &[u8], password: Option<&str>) -> Result<Vec<u8>> {
    let mut doc = PdfDocument::from_bytes(source)?;
    if doc.needs_password()? {
        match password {
            Some(pw) if doc.authenticate(pw)? => {}
            _ => bail!("WRONG_PASSWORD"),
        }
    }
    let mut opts = save_options();
    opts.set_encryption(Encryption::None);
    to_bytes(&doc, opts)
}

/// Write `bytes` to a new file in `dir` named after `stem`, never
/// overwriting an existing file.
pub fn write_new(dir: &Path, stem: &str, bytes: &[u8]) -> Result<PathBuf> {
    let path = unused_path(dir, stem);
    write_file_safely(&path, bytes)?;
    Ok(path)
}
