//! Images, stamps and signatures on PDF pages (SRS Modules 4 and 5).
//!
//! - Pictures (PNG, JPEG, WebP) are decoded with the `image` crate, shrunk
//!   if they are very large (FR-4.4) and added to the page as an image
//!   XObject, with optional rotation and opacity (FR-4.1 to FR-4.3).
//! - Stamps and signatures arrive from the UI as transparent PNGs and are
//!   placed the same way. A photo of a signature on paper is turned into
//!   clean ink on a transparent background here (FR-5.3, FR-5.4).
//! - Images already in a PDF can be found, deleted or replaced (FR-4.5).

use std::io::Cursor;

use anyhow::{anyhow, bail, Context, Result};
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, Rgba, RgbaImage};
use mupdf::pdf::{PdfDocument, PdfObject, PdfPage};
use mupdf::text_page::{TextBlockType, TextPageFlags};
use mupdf::{Buffer, Image, Matrix, Rect};
use serde::{Deserialize, Serialize};

/// Longest side, in pixels, an inserted picture is kept at. At 300 dpi this
/// is about 20 cm, more than any page needs; bigger photos only make the PDF
/// larger (FR-4.4).
pub const MAX_SIDE: u32 = 2400;
/// Above this size a picture without transparency is stored as JPEG.
const RECOMPRESS_BYTES: usize = 400 * 1024;

/// A picture ready to go into a PDF.
#[derive(Debug, Clone)]
pub struct Prepared {
    /// PNG or JPEG bytes.
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Decode a picture (PNG, JPEG or WebP), shrink it if needed, and encode it
/// as PNG (keeps transparency) or JPEG (photos). Small JPEG and PNG files are
/// kept exactly as they are.
pub fn prepare(bytes: &[u8]) -> Result<Prepared> {
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let format = reader.format().ok_or_else(|| anyhow!("UNSUPPORTED_IMAGE"))?;
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP) {
        bail!("UNSUPPORTED_IMAGE");
    }
    let img = reader.decode().map_err(|e| anyhow!("this picture could not be read ({e})"))?;
    let (w, h) = img.dimensions();
    let too_big = w.max(h) > MAX_SIDE;
    let keep_as_is = !too_big
        && matches!(format, ImageFormat::Jpeg | ImageFormat::Png)
        && (bytes.len() <= RECOMPRESS_BYTES || format == ImageFormat::Jpeg);
    if keep_as_is {
        return Ok(Prepared { bytes: bytes.to_vec(), width: w, height: h });
    }
    let img = if too_big { img.resize(MAX_SIDE, MAX_SIDE, image::imageops::FilterType::Lanczos3) } else { img };
    encode(&img)
}

fn has_transparency(img: &DynamicImage) -> bool {
    img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p.0[3] < 255)
}

fn encode(img: &DynamicImage) -> Result<Prepared> {
    let (width, height) = img.dimensions();
    let mut out = Cursor::new(Vec::new());
    if has_transparency(img) {
        img.to_rgba8().write_to(&mut out, ImageFormat::Png)?;
    } else {
        let rgb = img.to_rgb8();
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 88);
        enc.encode_image(&rgb)?;
    }
    Ok(Prepared { bytes: out.into_inner(), width, height })
}

/// Turn a photo or scan of a signature on paper into ink of colour `ink` on
/// a transparent background, cropped to the signature (FR-5.3, FR-5.4).
pub fn signature_from_photo(bytes: &[u8], ink: [u8; 3]) -> Result<Vec<u8>> {
    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .decode()
        .map_err(|e| anyhow!("this picture could not be read ({e})"))?;
    let img = if img.width().max(img.height()) > 1600 {
        img.resize(1600, 1600, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let gray = img.to_luma8();
    // Paper is the bright majority: take a high percentile as "paper" and a
    // low one as "ink", so uneven lighting and grey paper still work.
    let mut levels: Vec<u8> = gray.pixels().map(|p| p.0[0]).collect();
    if levels.is_empty() {
        bail!("the picture is empty");
    }
    levels.sort_unstable();
    let at = |q: f32| levels[((levels.len() - 1) as f32 * q) as usize] as f32;
    let paper = at(0.90);
    let dark = at(0.01).min(paper - 40.0);
    if paper - dark < 30.0 {
        bail!("NO_SIGNATURE_FOUND");
    }
    // Ink fades in smoothly between "a bit darker than paper" and "dark".
    let start = paper - (paper - dark) * 0.18;
    let full = dark + (paper - dark) * 0.35;
    let (w, h) = gray.dimensions();
    let mut out = RgbaImage::new(w, h);
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for (x, y, p) in gray.enumerate_pixels() {
        let v = p.0[0] as f32;
        let a = ((start - v) / (start - full)).clamp(0.0, 1.0);
        let a = (a * 255.0).round() as u8;
        out.put_pixel(x, y, Rgba([ink[0], ink[1], ink[2], a]));
        if a > 60 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    if x1 <= x0 || y1 <= y0 {
        bail!("NO_SIGNATURE_FOUND");
    }
    let pad = ((x1 - x0).max(y1 - y0) / 20).max(4);
    let (cx0, cy0) = (x0.saturating_sub(pad), y0.saturating_sub(pad));
    let (cx1, cy1) = ((x1 + pad).min(w - 1), (y1 + pad).min(h - 1));
    let cropped = image::imageops::crop_imm(&out, cx0, cy0, cx1 - cx0 + 1, cy1 - cy0 + 1).to_image();
    let mut png = Cursor::new(Vec::new());
    cropped.write_to(&mut png, ImageFormat::Png)?;
    Ok(png.into_inner())
}

/// Where to put a picture: a box on the page in MuPDF page coordinates
/// (points, origin top-left, as the page is shown), turned clockwise by
/// `rotation` degrees around its centre.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Placement {
    pub center: [f32; 2],
    pub size: [f32; 2],
    #[serde(default)]
    pub rotation: f32,
    /// 0 (invisible) to 1 (solid).
    #[serde(default = "solid")]
    pub opacity: f32,
}

fn solid() -> f32 {
    1.0
}

/// The matrix that maps the image's unit square to page coordinates
/// (y pointing down), for a box turned clockwise around its centre.
fn device_matrix(p: &Placement) -> Matrix {
    let (w, h) = (p.size[0], p.size[1]);
    let (s, c) = p.rotation.to_radians().sin_cos();
    // Image space: (0,0) is the bottom-left of the picture, (0,1) its top-left.
    // A point (u, v) sits at centre + R((u - 1/2) w, (1/2 - v) h), where R
    // turns clockwise on a y-down page: R(x, y) = (x c - y s, x s + y c).
    let e = p.center[0] - w / 2.0 * c - h / 2.0 * s;
    let f = p.center[1] - w / 2.0 * s + h / 2.0 * c;
    Matrix::new(w * c, w * s, h * s, -h * c, e, f)
}

/// Put a picture on a page. `bytes` is PNG or JPEG from `prepare`.
pub fn place(doc: &mut PdfDocument, page_no: i32, bytes: &[u8], at: &Placement) -> Result<()> {
    if !(at.size[0] > 0.0 && at.size[1] > 0.0) {
        bail!("the picture needs a size");
    }
    let mut page = doc.load_pdf_page(page_no)?;
    let image = Image::from_bytes(bytes).context("reading the picture")?;
    let xobj = doc.add_image(&image)?;
    let name = add_xobject(doc, &mut page, xobj, "AkkhorImg")?;
    let opacity = at.opacity.clamp(0.0, 1.0);
    let gs = if opacity < 0.999 { page.register_ext_gstate(doc, None, Some(opacity))? } else { None };

    // Page coordinates → PDF user space (handles page rotation and boxes
    // that do not start at 0,0).
    let ctm_inv = page.ctm()?.invert().context("the page has no size")?;
    let mut m = device_matrix(at);
    m.concat(ctm_inv);

    let mut content = String::from("q\n");
    if let Some(gs) = gs {
        content.push_str(&format!("{gs} gs\n"));
    }
    content.push_str(&format!(
        "{:.4} {:.4} {:.4} {:.4} {:.3} {:.3} cm\n/{name} Do\nQ\n",
        m.a, m.b, m.c, m.d, m.e, m.f
    ));
    // Keep the page's own drawing state from leaking into ours.
    page.wrap_contents(doc)?;
    page.insert_contents(doc, content.as_bytes(), true)?;
    Ok(())
}

/// Add an XObject to the page's resources under a new name.
fn add_xobject(doc: &mut PdfDocument, page: &mut PdfPage, xobj: PdfObject, prefix: &str) -> Result<String> {
    let mut resources = page.resources()?;
    let mut xobjects = match resources.get_dict("XObject")? {
        Some(d) => d,
        None => {
            resources.dict_put("XObject", doc.new_dict()?)?;
            resources.get_dict("XObject")?.context("creating XObject dictionary")?
        }
    };
    let name = (1..)
        .map(|i| format!("{prefix}{i}"))
        .find(|n| matches!(xobjects.get_dict(n.as_str()), Ok(None)))
        .expect("an unused name");
    xobjects.dict_put(name.as_str(), xobj)?;
    Ok(name)
}

/// An image drawn on a page.
#[derive(Debug, Clone, Serialize)]
pub struct PageImage {
    /// x0, y0, x1, y1 in page points.
    pub rect: [f32; 4],
    /// The image object, when it belongs to the page itself (then it can be
    /// deleted or replaced). Images inside other objects have none.
    pub xref: Option<i32>,
    pub width: u32,
    pub height: u32,
}

/// Every image drawn on a page, with the object behind it where known.
pub fn page_images(doc: &PdfDocument, page_no: i32) -> Result<Vec<PageImage>> {
    let page = doc.load_pdf_page(page_no)?;
    let known = page.images()?;
    let tp = page.to_text_page(TextPageFlags::PRESERVE_IMAGES)?;
    let mut out = Vec::new();
    for block in tp.blocks() {
        if block.r#type() != TextBlockType::Image {
            continue;
        }
        let Some(img) = block.image() else { continue };
        let (w, h) = (img.width(), img.height());
        // Match by pixel size; only trust a match that is unambiguous.
        let same: Vec<i32> = known.iter().filter(|k| k.width == w && k.height == h).map(|k| k.xref).collect();
        let b = block.bounds();
        out.push(PageImage {
            rect: [b.x0, b.y0, b.x1, b.y1],
            xref: if same.len() == 1 { Some(same[0]) } else { None },
            width: w,
            height: h,
        });
    }
    Ok(out)
}

/// Point every use of image `xref` on the page at `replacement`.
fn swap_xobject(doc: &mut PdfDocument, page_no: i32, xref: i32, replacement: PdfObject) -> Result<()> {
    let page = doc.load_pdf_page(page_no)?;
    let resources = page.resources()?;
    let mut xobjects = resources.get_dict("XObject")?.ok_or_else(|| anyhow!("IMAGE_NOT_FOUND"))?;
    let mut keys = Vec::new();
    for i in 0..xobjects.dict_len()? {
        let Some(v) = xobjects.get_dict_val(i as i32)? else { continue };
        if v.is_indirect()? && v.as_indirect()? == xref {
            if let Some(k) = xobjects.get_dict_key(i as i32)? {
                keys.push(String::from_utf8(k.as_name()?)?);
            }
        }
    }
    if keys.is_empty() {
        bail!("IMAGE_NOT_FOUND");
    }
    for k in keys {
        xobjects.dict_put(k.as_str(), replacement.clone())?;
    }
    Ok(())
}

/// Point every page's use of image `xref` at `replacement` (pages that do not
/// use it are skipped). Returns how many pages changed.
pub fn replace_everywhere(doc: &mut PdfDocument, xref: i32, replacement: &PdfObject) -> Result<usize> {
    let mut changed = 0;
    for page_no in 0..doc.page_count()? {
        if swap_xobject(doc, page_no, xref, replacement.clone()).is_ok() {
            changed += 1;
        }
    }
    Ok(changed)
}

/// Remove an image from a page. Its name is pointed at an empty drawing, so
/// every PDF reader shows nothing there; the image data itself is dropped
/// when the file is saved.
pub fn delete_image(doc: &mut PdfDocument, page_no: i32, xref: i32) -> Result<()> {
    let mut bbox = doc.new_array()?;
    for _ in 0..4 {
        bbox.array_push(PdfObject::new_real(0.0)?)?;
    }
    let mut form = doc.new_dict()?;
    form.dict_put("Type", PdfObject::new_name("XObject")?)?;
    form.dict_put("Subtype", PdfObject::new_name("Form")?)?;
    form.dict_put("BBox", bbox)?;
    let empty = doc.add_stream(&Buffer::from_bytes(b"")?, Some(&form), false)?;
    swap_xobject(doc, page_no, xref, empty)
}

/// Replace an image with another picture, in the same place and size.
pub fn replace_image(doc: &mut PdfDocument, page_no: i32, xref: i32, bytes: &[u8]) -> Result<()> {
    let image = Image::from_bytes(bytes).context("reading the picture")?;
    let new = doc.add_image(&image)?;
    swap_xobject(doc, page_no, xref, new)
}

/// The page box of a page as shown (after rotation), for fitting pictures.
pub fn page_box(doc: &PdfDocument, page_no: i32) -> Result<Rect> {
    Ok(doc.load_pdf_page(page_no)?.bounds()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(m: &Matrix, x: f32, y: f32) -> (f32, f32) {
        (x * m.a + y * m.c + m.e, x * m.b + y * m.d + m.f)
    }

    #[test]
    fn unrotated_box_maps_corners() {
        let p = Placement { center: [150.0, 100.0], size: [100.0, 50.0], rotation: 0.0, opacity: 1.0 };
        let m = device_matrix(&p);
        // Top-left of the picture is (0,1) in image space.
        let (x, y) = apply(&m, 0.0, 1.0);
        assert!((x - 100.0).abs() < 0.01 && (y - 75.0).abs() < 0.01, "{x},{y}");
        let (x, y) = apply(&m, 1.0, 0.0);
        assert!((x - 200.0).abs() < 0.01 && (y - 125.0).abs() < 0.01, "{x},{y}");
    }

    #[test]
    fn quarter_turn_keeps_centre_and_turns_clockwise() {
        let p = Placement { center: [150.0, 100.0], size: [100.0, 50.0], rotation: 90.0, opacity: 1.0 };
        let m = device_matrix(&p);
        let (x, y) = apply(&m, 0.5, 0.5);
        assert!((x - 150.0).abs() < 0.01 && (y - 100.0).abs() < 0.01, "centre moved to {x},{y}");
        // Turned clockwise, the picture's top-left ends up top-right.
        let (x, y) = apply(&m, 0.0, 1.0);
        assert!((x - 175.0).abs() < 0.01 && (y - 50.0).abs() < 0.01, "{x},{y}");
    }
}
