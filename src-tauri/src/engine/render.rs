//! Render PDF pages to PNG images.

use anyhow::Result;
use mupdf::{Colorspace, ImageFormat, Matrix};

use super::extract::open_pdf;

/// Render one page of a PDF as PNG bytes. `zoom` 1.0 = 72 dpi, 2.0 = 144 dpi.
pub fn page_png(pdf: &[u8], page_no: i32, zoom: f32) -> Result<Vec<u8>> {
    let doc = open_pdf(pdf)?;
    let page = doc.load_page(page_no)?;
    let pixmap = page.to_pixmap(&Matrix::new_scale(zoom, zoom), &Colorspace::device_rgb(), false, true)?;
    let mut png = Vec::new();
    pixmap.write_to(&mut png, ImageFormat::PNG)?;
    Ok(png)
}

/// Render one page as raw RGB pixels: (width, height, bytes).
pub fn page_rgb(pdf: &[u8], page_no: i32, zoom: f32) -> Result<(u32, u32, Vec<u8>)> {
    let doc = open_pdf(pdf)?;
    let page = doc.load_page(page_no)?;
    let pixmap = page.to_pixmap(&Matrix::new_scale(zoom, zoom), &Colorspace::device_rgb(), false, true)?;
    let (w, h, n) = (pixmap.width(), pixmap.height(), pixmap.n() as usize);
    let stride = pixmap.stride() as usize;
    let samples = pixmap.samples();
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h as usize {
        let row = &samples[y * stride..y * stride + w as usize * n];
        for px in row.chunks_exact(n) {
            rgb.extend_from_slice(&px[..3]);
        }
    }
    Ok((w, h, rgb))
}
