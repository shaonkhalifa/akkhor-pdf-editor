//! Image tools (SRS Module 14, added in v2.1): resize pictures, convert
//! between JPG, PNG and WebP, shrink to a target file size, and make a plain
//! background transparent. Everything runs on this PC with the `image`
//! crate; no AI model and no network.

use std::collections::VecDeque;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageFormat, ImageReader, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

/// Largest picture accepted (pixels on the longest side), to stay within memory.
const MAX_INPUT_SIDE: u32 = 16_000;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutFormat {
    /// The same format as the input (JPEG stays JPEG, everything else PNG).
    Keep,
    Jpg,
    Png,
    Webp,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResizeMode {
    None,
    /// To `width` x `height` pixels.
    Pixels,
    /// To `percent` of the original size.
    Percent,
}

/// What to do with a picture.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageJob {
    pub resize: ResizeMode,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default = "hundred")]
    pub percent: f32,
    /// Keep the picture's shape: fit inside width x height.
    #[serde(default = "yes")]
    pub keep_aspect: bool,
    /// With `keep_aspect`, fill width x height exactly and cut off the rest
    /// (centred), instead of fitting inside.
    #[serde(default)]
    pub crop: bool,
    pub format: OutFormat,
    /// JPEG quality, 1–100.
    #[serde(default = "quality")]
    pub quality: u8,
    /// Colour behind transparent parts when saving as JPEG.
    #[serde(default = "white")]
    pub background: [u8; 3],
    /// Make the file at most this many kilobytes (lowers JPEG quality, then size).
    #[serde(default)]
    pub target_kb: Option<u32>,
}

fn hundred() -> f32 {
    100.0
}
fn yes() -> bool {
    true
}
fn quality() -> u8 {
    90
}
fn white() -> [u8; 3] {
    [255, 255, 255]
}

/// A finished picture.
#[derive(Debug, Clone)]
pub struct Output {
    pub bytes: Vec<u8>,
    /// File extension: "jpg", "png" or "webp".
    pub ext: &'static str,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    /// "jpg", "png" or "webp".
    pub format: &'static str,
    pub has_alpha: bool,
}

fn format_name(f: ImageFormat) -> Result<&'static str> {
    Ok(match f {
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Png => "png",
        ImageFormat::WebP => "webp",
        _ => bail!("UNSUPPORTED_IMAGE"),
    })
}

/// Open a PNG, JPEG or WebP picture.
pub fn open(bytes: &[u8]) -> Result<(DynamicImage, &'static str)> {
    let reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let format = format_name(reader.format().ok_or_else(|| anyhow!("UNSUPPORTED_IMAGE"))?)?;
    let img = reader.decode().map_err(|e| anyhow!("this picture could not be read ({e})"))?;
    if img.width().max(img.height()) > MAX_INPUT_SIDE {
        bail!("the picture is too large ({} x {} pixels)", img.width(), img.height());
    }
    Ok((img, format))
}

pub fn info(bytes: &[u8]) -> Result<ImageInfo> {
    let (img, format) = open(bytes)?;
    Ok(ImageInfo {
        width: img.width(),
        height: img.height(),
        bytes: bytes.len() as u64,
        format,
        has_alpha: img.color().has_alpha() && img.to_rgba8().pixels().any(|p| p.0[3] < 255),
    })
}

/// The new size for `job`, or the same size.
fn new_size(w: u32, h: u32, job: &ImageJob) -> (u32, u32) {
    match job.resize {
        ResizeMode::None => (w, h),
        ResizeMode::Percent => {
            let f = (job.percent / 100.0).clamp(0.01, 10.0);
            (((w as f32) * f).round().max(1.0) as u32, ((h as f32) * f).round().max(1.0) as u32)
        }
        ResizeMode::Pixels => {
            let (tw, th) = (job.width, job.height);
            match (tw, th, job.keep_aspect && !job.crop) {
                (0, 0, _) => (w, h),
                // One side given: the other follows the shape.
                (tw, 0, _) => (tw, ((h as f64) * tw as f64 / w as f64).round().max(1.0) as u32),
                (0, th, _) => (((w as f64) * th as f64 / h as f64).round().max(1.0) as u32, th),
                (tw, th, true) => {
                    let f = (tw as f64 / w as f64).min(th as f64 / h as f64);
                    (((w as f64) * f).round().max(1.0) as u32, ((h as f64) * f).round().max(1.0) as u32)
                }
                (tw, th, false) => (tw, th),
            }
        }
    }
}

fn resized(img: &DynamicImage, job: &ImageJob) -> DynamicImage {
    let (w, h) = img.dimensions();
    if job.resize == ResizeMode::Pixels && job.keep_aspect && job.crop && job.width > 0 && job.height > 0 {
        // Fill the exact size, cutting off the middle part of the overflow.
        return img.resize_to_fill(job.width.min(MAX_INPUT_SIDE), job.height.min(MAX_INPUT_SIDE), FilterType::Lanczos3);
    }
    let (nw, nh) = new_size(w, h, job);
    let (nw, nh) = (nw.clamp(1, MAX_INPUT_SIDE), nh.clamp(1, MAX_INPUT_SIDE));
    if (nw, nh) == (w, h) {
        img.clone()
    } else {
        img.resize_exact(nw, nh, FilterType::Lanczos3)
    }
}

/// Put a transparent picture on a solid colour (for JPEG, which has no transparency).
fn flatten(img: &DynamicImage, bg: [u8; 3]) -> image::RgbImage {
    let rgba = img.to_rgba8();
    image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = p[3] as u32;
        let mix = |c: u8, b: u8| ((c as u32 * a + b as u32 * (255 - a)) / 255) as u8;
        image::Rgb([mix(p[0], bg[0]), mix(p[1], bg[1]), mix(p[2], bg[2])])
    })
}

fn encode(img: &DynamicImage, ext: &'static str, quality: u8, bg: [u8; 3]) -> Result<Vec<u8>> {
    let mut out = Cursor::new(Vec::new());
    match ext {
        "jpg" => {
            let rgb = flatten(img, bg);
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100)).encode_image(&rgb)?;
        }
        "png" => img.write_to(&mut out, ImageFormat::Png)?,
        "webp" => {
            // The image crate writes lossless WebP.
            img.to_rgba8().write_to(&mut out, ImageFormat::WebP)?;
        }
        _ => bail!("unknown format {ext}"),
    }
    Ok(out.into_inner())
}

/// Resize and/or convert a picture (and shrink it to a file size if asked).
pub fn process(bytes: &[u8], job: &ImageJob) -> Result<Output> {
    let (img, input) = open(bytes)?;
    let ext: &'static str = match job.format {
        OutFormat::Keep => {
            if input == "jpg" {
                "jpg"
            } else if input == "webp" {
                "webp"
            } else {
                "png"
            }
        }
        OutFormat::Jpg => "jpg",
        OutFormat::Png => "png",
        OutFormat::Webp => "webp",
    };
    let mut img = resized(&img, job);
    let mut quality = job.quality;
    let mut bytes = encode(&img, ext, quality, job.background)?;

    if let Some(kb) = job.target_kb.filter(|kb| *kb > 0) {
        let limit = kb as usize * 1024;
        let mut rounds = 0;
        while bytes.len() > limit {
            rounds += 1;
            if rounds > 40 {
                bail!("TARGET_TOO_SMALL");
            }
            if ext == "jpg" && quality > 35 {
                // First lower the quality (binary search would also work;
                // small steps keep the best quality that fits).
                quality = quality.saturating_sub(if quality > 70 { 5 } else { 4 });
            } else {
                // Then make the picture smaller.
                // Never shrink below 64 pixels on the long side: a picture
                // that small is useless, so tell the user instead.
                let (w, h) = img.dimensions();
                let f = ((limit as f64 / bytes.len() as f64).sqrt() * 0.95).clamp(0.5, 0.95);
                let (nw, nh) = (((w as f64) * f).max(1.0) as u32, ((h as f64) * f).max(1.0) as u32);
                if nw.max(nh) < 64 {
                    bail!("TARGET_TOO_SMALL");
                }
                img = img.resize_exact(nw, nh, FilterType::Lanczos3);
            }
            bytes = encode(&img, ext, quality, job.background)?;
        }
    }
    let (width, height) = img.dimensions();
    Ok(Output { bytes, ext, width, height })
}

// ---------------------------------------------------------------------------
// Background remover (plain backgrounds)
// ---------------------------------------------------------------------------

/// How to remove a background.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BackgroundJob {
    /// The background colour; none means "guess from the picture's edges".
    pub color: Option<[u8; 3]>,
    /// How different a colour may be and still count as background (0–100).
    #[serde(default = "tolerance")]
    pub tolerance: f32,
    /// Width of the soft edge, in the same units (0 = hard edge).
    #[serde(default = "softness")]
    pub softness: f32,
    /// Only remove background that touches the edge of the picture (keeps
    /// same-coloured parts inside the subject, such as white eyes or text).
    #[serde(default = "yes")]
    pub from_edges: bool,
    /// Cut the picture down to the part that is left.
    #[serde(default)]
    pub trim: bool,
}

fn tolerance() -> f32 {
    18.0
}
fn softness() -> f32 {
    8.0
}

/// The most common colour along the picture's edges (rounded), which for a
/// photo on a plain background is the background.
pub fn guess_background(img: &RgbaImage) -> [u8; 3] {
    let (w, h) = img.dimensions();
    let mut counts = std::collections::HashMap::<[u8; 3], (u32, [u32; 3])>::new();
    let mut add = |x: u32, y: u32| {
        let p = img.get_pixel(x, y).0;
        let key = [p[0] / 16, p[1] / 16, p[2] / 16];
        let e = counts.entry(key).or_insert((0, [0; 3]));
        e.0 += 1;
        for i in 0..3 {
            e.1[i] += p[i] as u32;
        }
    };
    let step = ((w.max(h)) / 400).max(1) as usize;
    for x in (0..w).step_by(step) {
        add(x, 0);
        add(x, h - 1);
    }
    for y in (0..h).step_by(step) {
        add(0, y);
        add(w - 1, y);
    }
    let (n, sum) = counts.into_values().max_by_key(|(n, _)| *n).unwrap_or((1, [255, 255, 255]));
    [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8]
}

/// Colour difference on a 0–100 scale (weighted RGB, close to how eyes see it).
fn distance(a: [u8; 3], b: [u8; 3]) -> f32 {
    let (dr, dg, db) = (a[0] as f32 - b[0] as f32, a[1] as f32 - b[1] as f32, a[2] as f32 - b[2] as f32);
    let rmean = (a[0] as f32 + b[0] as f32) / 2.0;
    let d = ((2.0 + rmean / 256.0) * dr * dr + 4.0 * dg * dg + (2.0 + (255.0 - rmean) / 256.0) * db * db).sqrt();
    d / 7.65 // 765 is the largest possible value
}

/// Make the plain background of a picture transparent. Returns a PNG.
pub fn remove_background(bytes: &[u8], job: &BackgroundJob) -> Result<Output> {
    let (img, _) = open(bytes)?;
    let mut rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w < 2 || h < 2 {
        bail!("the picture is too small");
    }
    let bg = job.color.unwrap_or_else(|| guess_background(&rgba));
    let tol = job.tolerance.clamp(0.0, 100.0);
    let soft = job.softness.clamp(0.0, 50.0);
    // Alpha for a colour: 0 inside the tolerance, rising to full over `soft`.
    let alpha_of = |p: &Rgba<u8>| -> u8 {
        let d = distance([p.0[0], p.0[1], p.0[2]], bg);
        if d <= tol {
            0
        } else if soft <= 0.0 || d >= tol + soft {
            255
        } else {
            (((d - tol) / soft) * 255.0).round() as u8
        }
    };

    let mut removed = 0usize;
    if job.from_edges {
        // Flood from every edge pixel through background-like colours.
        let mut seen = vec![false; (w * h) as usize];
        let mut queue = VecDeque::new();
        let idx = |x: u32, y: u32| (y * w + x) as usize;
        for x in 0..w {
            queue.push_back((x, 0));
            queue.push_back((x, h - 1));
        }
        for y in 0..h {
            queue.push_back((0, y));
            queue.push_back((w - 1, y));
        }
        while let Some((x, y)) = queue.pop_front() {
            let i = idx(x, y);
            if seen[i] {
                continue;
            }
            seen[i] = true;
            let p = *rgba.get_pixel(x, y);
            let a = alpha_of(&p);
            if a == 255 {
                continue; // the subject starts here
            }
            let keep = (p.0[3] as u32 * a as u32 / 255) as u8;
            rgba.put_pixel(x, y, Rgba([p.0[0], p.0[1], p.0[2], keep]));
            removed += 1;
            if a > 0 {
                continue; // a soft edge pixel: do not spread past it
            }
            if x > 0 {
                queue.push_back((x - 1, y));
            }
            if x + 1 < w {
                queue.push_back((x + 1, y));
            }
            if y > 0 {
                queue.push_back((x, y - 1));
            }
            if y + 1 < h {
                queue.push_back((x, y + 1));
            }
        }
    } else {
        for p in rgba.pixels_mut() {
            let a = alpha_of(p);
            if a < 255 {
                p.0[3] = (p.0[3] as u32 * a as u32 / 255) as u8;
                removed += 1;
            }
        }
    }
    if removed == 0 {
        bail!("NO_BACKGROUND_FOUND");
    }

    let mut out_img = DynamicImage::ImageRgba8(rgba);
    if job.trim {
        let rgba = out_img.to_rgba8();
        let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
        for (x, y, p) in rgba.enumerate_pixels() {
            if p.0[3] > 16 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
        if x1 < x0 {
            bail!("NOTHING_LEFT");
        }
        out_img = out_img.crop_imm(x0, y0, x1 - x0 + 1, y1 - y0 + 1);
    }
    let (width, height) = out_img.dimensions();
    Ok(Output { bytes: encode(&out_img, "png", 100, [255, 255, 255])?, ext: "png", width, height })
}

/// A file name in `dir` for `stem` + `ext` that does not exist yet.
pub fn unused_file(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..).map(|i| dir.join(format!("{stem} ({i}).{ext}"))).find(|p| !p.exists()).expect("a free name")
}

/// A small preview (longest side at most `max`) as PNG or JPEG bytes.
pub fn preview(bytes: &[u8], max: u32) -> Result<Vec<u8>> {
    let (img, _) = open(bytes)?;
    let small = if img.width().max(img.height()) > max { img.resize(max, max, FilterType::Triangle) } else { img };
    let has_alpha = small.color().has_alpha();
    encode(&small, if has_alpha { "png" } else { "jpg" }, 85, [255, 255, 255]).context("making a preview")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> ImageJob {
        ImageJob {
            resize: ResizeMode::None,
            width: 0,
            height: 0,
            percent: 100.0,
            keep_aspect: true,
            crop: false,
            format: OutFormat::Keep,
            quality: 90,
            background: [255, 255, 255],
            target_kb: None,
        }
    }

    #[test]
    fn sizes() {
        let mut j = job();
        j.resize = ResizeMode::Pixels;
        (j.width, j.height) = (300, 300);
        assert_eq!(new_size(1200, 800, &j), (300, 200), "fits inside, keeps shape");
        j.keep_aspect = false;
        assert_eq!(new_size(1200, 800, &j), (300, 300), "stretches");
        (j.width, j.height) = (600, 0);
        assert_eq!(new_size(1200, 800, &j), (600, 400), "one side given");
        j.resize = ResizeMode::Percent;
        j.percent = 25.0;
        assert_eq!(new_size(1200, 800, &j), (300, 200));
    }

    #[test]
    fn colour_distance_is_zero_to_hundred() {
        assert_eq!(distance([10, 20, 30], [10, 20, 30]), 0.0);
        let far = distance([0, 0, 0], [255, 255, 255]);
        assert!(far > 99.0 && far <= 100.5, "{far}");
    }
}
