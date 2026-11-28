//! Phase 8A acceptance tests: image tools (resize, convert, target file
//! size, plain background remover).

use std::io::Cursor;

use akkhor_pdf_editor_lib::engine::imagetools::{self, BackgroundJob, ImageJob, OutFormat, ResizeMode};
use image::{GenericImageView, ImageFormat, Rgba, RgbaImage};

fn encode(img: &RgbaImage, f: ImageFormat) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    if f == ImageFormat::Jpeg {
        image::DynamicImage::ImageRgba8(img.clone()).to_rgb8().write_to(&mut out, f).unwrap();
    } else {
        img.write_to(&mut out, f).unwrap();
    }
    out.into_inner()
}

/// A busy "photo" (noise compresses badly, like real photos).
fn photo(w: u32, h: u32) -> Vec<u8> {
    let img = RgbaImage::from_fn(w, h, |x, y| {
        let v = (x.wrapping_mul(2654435761) ^ y.wrapping_mul(40503) ^ (x * y)) as u8;
        Rgba([v, v.wrapping_mul(3), 255 - v, 255])
    });
    encode(&img, ImageFormat::Jpeg)
}

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
fn resize_by_pixels_percent_and_crop() {
    let src = photo(1200, 800);
    let mut j = job();
    j.resize = ResizeMode::Pixels;
    (j.width, j.height) = (300, 300);
    let out = imagetools::process(&src, &j).unwrap();
    assert_eq!((out.width, out.height), (300, 200), "fits inside 300x300, same shape");
    assert_eq!(out.ext, "jpg", "a JPEG stays JPEG");

    j.crop = true;
    let out = imagetools::process(&src, &j).unwrap();
    assert_eq!((out.width, out.height), (300, 300), "exact size, cropped");
    assert_eq!(image::load_from_memory(&out.bytes).unwrap().dimensions(), (300, 300));

    let mut p = job();
    p.resize = ResizeMode::Percent;
    p.percent = 50.0;
    assert_eq!(imagetools::process(&src, &p).map(|o| (o.width, o.height)).unwrap(), (600, 400));
}

#[test]
fn convert_between_formats() {
    // Half transparent PNG → JPEG on a chosen background colour.
    let png = encode(&RgbaImage::from_fn(40, 20, |x, _| if x < 20 { Rgba([0, 0, 200, 255]) } else { Rgba([0, 0, 0, 0]) }), ImageFormat::Png);
    let mut j = job();
    j.format = OutFormat::Jpg;
    j.background = [255, 0, 0];
    let out = imagetools::process(&png, &j).unwrap();
    assert!(out.bytes.starts_with(&[0xFF, 0xD8]));
    let img = image::load_from_memory(&out.bytes).unwrap().to_rgb8();
    let right = img.get_pixel(35, 10).0;
    assert!(right[0] > 200 && right[1] < 60, "transparent part becomes the background colour: {right:?}");

    j.format = OutFormat::Png;
    let out = imagetools::process(&photo(64, 64), &j).unwrap();
    assert!(out.bytes.starts_with(b"\x89PNG") && out.ext == "png");

    j.format = OutFormat::Webp;
    let out = imagetools::process(&png, &j).unwrap();
    assert_eq!(out.ext, "webp");
    let back = image::load_from_memory(&out.bytes).unwrap().to_rgba8();
    assert_eq!(back.get_pixel(35, 10).0[3], 0, "WebP keeps transparency");

    let info = imagetools::info(&png).unwrap();
    assert!(info.has_alpha && info.format == "png" && info.width == 40);
    assert!(imagetools::process(b"GIF89a....", &j).unwrap_err().to_string().contains("UNSUPPORTED_IMAGE"));
}

#[test]
fn shrink_to_a_target_file_size() {
    let src = photo(2000, 1500);
    assert!(src.len() > 400 * 1024, "the test photo starts big ({} bytes)", src.len());
    let mut j = job();
    j.target_kb = Some(100);
    let out = imagetools::process(&src, &j).unwrap();
    assert!(out.bytes.len() <= 100 * 1024, "{} bytes", out.bytes.len());
    assert!(out.width >= 300, "should not shrink more than needed: {}x{}", out.width, out.height);

    // A form that wants 300x300 under 50 KB.
    j.resize = ResizeMode::Pixels;
    (j.width, j.height, j.crop) = (300, 300, true);
    j.target_kb = Some(50);
    let out = imagetools::process(&src, &j).unwrap();
    assert!(out.bytes.len() <= 50 * 1024);

    // PNG has no quality setting: it gets smaller in pixels instead.
    let mut png = job();
    png.format = OutFormat::Png;
    png.target_kb = Some(200);
    let out = imagetools::process(&src, &png).unwrap();
    assert!(out.bytes.len() <= 200 * 1024 && out.width < 2000);

    let mut tiny = job();
    tiny.target_kb = Some(1);
    tiny.format = OutFormat::Png;
    assert!(imagetools::process(&src, &tiny).unwrap_err().to_string().contains("TARGET_TOO_SMALL"));
}

/// A dark blue round shape with a white "window" inside, on an off-white
/// background with a little noise.
fn subject_on_white() -> Vec<u8> {
    let img = RgbaImage::from_fn(200, 160, |x, y| {
        let (dx, dy) = (x as f32 - 100.0, y as f32 - 80.0);
        let r = (dx * dx + dy * dy).sqrt();
        if r < 20.0 {
            Rgba([252, 252, 252, 255]) // white inside the subject
        } else if r < 50.0 {
            Rgba([20, 30, 120, 255])
        } else {
            let n = ((x * 7 + y * 13) % 5) as u8;
            Rgba([246 + n, 246 + n, 244 + n, 255])
        }
    });
    encode(&img, ImageFormat::Png)
}

#[test]
fn plain_background_becomes_transparent() {
    let src = subject_on_white();
    let mut j = BackgroundJob { color: None, tolerance: 18.0, softness: 8.0, from_edges: true, trim: false };
    let out = imagetools::remove_background(&src, &j).unwrap();
    assert_eq!(out.ext, "png");
    let img = image::load_from_memory(&out.bytes).unwrap().to_rgba8();
    assert_eq!(img.get_pixel(5, 5).0[3], 0, "background corner is transparent");
    assert_eq!(img.get_pixel(100, 120).0[3], 255, "the subject stays");
    assert_eq!(img.get_pixel(100, 80).0[3], 255, "white inside the subject stays (from the edges only)");

    // Remove that colour everywhere: the white inside goes too.
    j.from_edges = false;
    let img = image::load_from_memory(&imagetools::remove_background(&src, &j).unwrap().bytes).unwrap().to_rgba8();
    assert_eq!(img.get_pixel(100, 80).0[3], 0);

    // Trim to the subject (about 100 x 100).
    j.from_edges = true;
    j.trim = true;
    let out = imagetools::remove_background(&src, &j).unwrap();
    assert!((98..=104).contains(&out.width) && (98..=104).contains(&out.height), "{}x{}", out.width, out.height);

    // A picked colour works too; the guess finds the off-white.
    let bg = imagetools::guess_background(&image::load_from_memory(&src).unwrap().to_rgba8());
    assert!(bg.iter().all(|c| *c > 240), "{bg:?}");
    j.color = Some([248, 248, 246]);
    assert!(imagetools::remove_background(&src, &j).is_ok());

    // A plain picture with nothing on it: nothing is left after trimming.
    let blank = encode(&RgbaImage::from_pixel(50, 50, Rgba([255, 255, 255, 255])), ImageFormat::Png);
    let err = imagetools::remove_background(&blank, &BackgroundJob { color: None, tolerance: 10.0, softness: 0.0, from_edges: true, trim: true }).unwrap_err();
    assert!(err.to_string().contains("NOTHING_LEFT"), "{err}");
}
