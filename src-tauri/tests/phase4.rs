//! Phase 4 acceptance tests: images, stamps and signatures (SRS Modules 4, 5).
//!
//! Pictures are placed, then the page is drawn and the pixels are checked,
//! so position, size, rotation and opacity are tested as the user sees them.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use akkhor_pdf_editor_lib::engine::{
    fonts::{default_bangla, dev_fonts_dir},
    images::{self, Placement},
    layout::{lines_to_pdf, LayoutOptions},
    viewer::{DocInfo, PageEdit, Viewer},
};
use image::{ImageFormat, Rgba, RgbaImage};

const LINE: &str = "পাতা ক্ষ্ম বিজ্ঞান — Invoice";

fn work_dir(test: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-output").join("phase4").join(test);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// An A4 page with one Bangla line at the top.
fn a4(dir: &Path) -> PathBuf {
    let path = dir.join("page.pdf");
    std::fs::write(&path, lines_to_pdf(&[LINE], &LayoutOptions::a4(default_bangla()), &dev_fonts_dir()).unwrap()).unwrap();
    path
}

fn png(img: &RgbaImage) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

/// A solid picture `w` x `h` pixels, with a blue top half and red bottom half,
/// so turning it is visible.
fn two_tone(w: u32, h: u32) -> Vec<u8> {
    png(&RgbaImage::from_fn(w, h, |_, y| if y < h / 2 { Rgba([0, 0, 255, 255]) } else { Rgba([255, 0, 0, 255]) }))
}

/// Colour of the drawn page at (x, y) points, at 100% (1 px per point).
fn pixel(viewer: &Viewer, info: &DocInfo, page: i32, x: f32, y: f32) -> [u8; 3] {
    let rgba = viewer.render_rgba(info.id, page, 1.0).unwrap();
    let w = u32::from_le_bytes(rgba[0..4].try_into().unwrap()) as usize;
    let i = 8 + (y as usize * w + x as usize) * 4;
    [rgba[i], rgba[i + 1], rgba[i + 2]]
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (*x as i32 - y as i32).abs() <= 24)
}

const WHITE: [u8; 3] = [255, 255, 255];
const BLUE: [u8; 3] = [0, 0, 255];
const RED: [u8; 3] = [255, 0, 0];

fn place(viewer: &Viewer, id: u64, page: i32, bytes: Vec<u8>, placement: Placement) -> DocInfo {
    viewer.edit(id, PageEdit::PlaceImage { page, bytes, placement }).unwrap()
}

#[test]
fn picture_lands_where_asked_with_rotation_and_opacity() {
    let dir = work_dir("place");
    let viewer = Viewer::new();
    let info = viewer.open(&a4(&dir), None).unwrap();
    let id = info.id;

    // 200 x 100 pt box centred at (300, 400): blue above y=400, red below.
    let info = place(&viewer, id, 0, two_tone(400, 200), Placement { center: [300.0, 400.0], size: [200.0, 100.0], rotation: 0.0, opacity: 1.0 });
    assert!(near(pixel(&viewer, &info, 0, 300.0, 370.0), BLUE));
    assert!(near(pixel(&viewer, &info, 0, 300.0, 430.0), RED));
    assert!(near(pixel(&viewer, &info, 0, 190.0, 400.0), WHITE), "nothing left of the box");
    let found = viewer.images(id, 0).unwrap();
    assert_eq!(found.len(), 1);
    let [x0, y0, x1, y1] = found[0].rect;
    assert!((x0 - 200.0).abs() < 1.0 && (y0 - 350.0).abs() < 1.0 && (x1 - 400.0).abs() < 1.0 && (y1 - 450.0).abs() < 1.0, "{:?}", found[0].rect);
    assert!(found[0].xref.is_some(), "a placed picture can be deleted later");

    // Turned 90° clockwise: the blue top is now on the right.
    let info = place(&viewer, id, 0, two_tone(400, 200), Placement { center: [300.0, 650.0], size: [200.0, 100.0], rotation: 90.0, opacity: 1.0 });
    assert!(near(pixel(&viewer, &info, 0, 330.0, 650.0), BLUE));
    assert!(near(pixel(&viewer, &info, 0, 270.0, 650.0), RED));
    assert!(near(pixel(&viewer, &info, 0, 300.0, 570.0), BLUE), "turned box is 200 pt tall");

    // Half see-through red over white is pink.
    let red = png(&RgbaImage::from_pixel(10, 10, Rgba([255, 0, 0, 255])));
    let info = place(&viewer, id, 0, red, Placement { center: [100.0, 150.0], size: [60.0, 60.0], rotation: 0.0, opacity: 0.5 });
    assert!(near(pixel(&viewer, &info, 0, 100.0, 150.0), [255, 128, 128]), "{:?}", pixel(&viewer, &info, 0, 100.0, 150.0));

    // The text under and around is untouched and still reads correctly.
    let text = viewer.text_lines(id, 0).unwrap();
    assert_eq!(text[0].text.split_whitespace().collect::<String>(), LINE.split_whitespace().collect::<String>());

    // Survives saving and reopening.
    let out = dir.join("saved.pdf");
    viewer.save(id, Some(out.clone())).unwrap();
    let again = Viewer::new();
    let info = again.open(&out, None).unwrap();
    assert!(near(pixel(&again, &info, 0, 300.0, 370.0), BLUE));
    assert_eq!(again.images(info.id, 0).unwrap().len(), 3);
}

#[test]
fn placing_on_a_turned_page_uses_the_page_as_shown() {
    let dir = work_dir("turned");
    let viewer = Viewer::new();
    let info = viewer.open(&a4(&dir), None).unwrap();
    let info = viewer.edit(info.id, PageEdit::Rotate { pages: vec![0], degrees: 90 }).unwrap();
    let [w, h] = info.pages[0];
    assert!(w > h, "landscape after turning");
    let info = place(&viewer, info.id, 0, two_tone(200, 200), Placement { center: [700.0, 100.0], size: [100.0, 100.0], rotation: 0.0, opacity: 1.0 });
    assert!(near(pixel(&viewer, &info, 0, 700.0, 70.0), BLUE), "upright on screen: blue on top");
    assert!(near(pixel(&viewer, &info, 0, 700.0, 130.0), RED));
}

#[test]
fn delete_and_replace_an_image_with_undo() {
    let dir = work_dir("delete");
    let viewer = Viewer::new();
    let info = viewer.open(&a4(&dir), None).unwrap();
    let id = info.id;
    let blue = png(&RgbaImage::from_pixel(8, 8, Rgba([0, 0, 255, 255])));
    let info = place(&viewer, id, 0, blue, Placement { center: [200.0, 300.0], size: [80.0, 80.0], rotation: 0.0, opacity: 1.0 });
    let xref = viewer.images(id, 0).unwrap()[0].xref.unwrap();
    assert!(near(pixel(&viewer, &info, 0, 200.0, 300.0), BLUE));

    // Replace keeps the place and size.
    let path = dir.join("red.png");
    std::fs::write(&path, png(&RgbaImage::from_pixel(5, 5, Rgba([255, 0, 0, 255])))).unwrap();
    let bytes = images::prepare(&std::fs::read(&path).unwrap()).unwrap().bytes;
    let info = viewer.edit(id, PageEdit::ReplaceImage { page: 0, xref, bytes }).unwrap();
    assert!(near(pixel(&viewer, &info, 0, 200.0, 300.0), RED));
    assert!(near(pixel(&viewer, &info, 0, 250.0, 300.0), WHITE));

    let xref = viewer.images(id, 0).unwrap()[0].xref.unwrap();
    let info = viewer.edit(id, PageEdit::DeleteImage { page: 0, xref }).unwrap();
    assert!(near(pixel(&viewer, &info, 0, 200.0, 300.0), WHITE), "the image is gone");
    assert!(viewer.images(id, 0).unwrap().is_empty());
    assert!(!viewer.text_lines(id, 0).unwrap().is_empty(), "text stays");

    let info = viewer.undo(id).unwrap();
    assert!(near(pixel(&viewer, &info, 0, 200.0, 300.0), RED), "undo brings it back");

    // Deleting an image that is not on the page fails and changes nothing.
    assert!(viewer.edit(id, PageEdit::DeleteImage { page: 0, xref: 99999 }).is_err());
    assert!(near(pixel(&viewer, &info, 0, 200.0, 300.0), RED));

    // Deleted image data is dropped from the saved file.
    let info = viewer.redo(id).unwrap();
    let with = dir.join("with.pdf");
    let without = dir.join("without.pdf");
    let big = png(&RgbaImage::from_fn(600, 600, |x, y| Rgba([(x * 7 % 251) as u8, (y * 13 % 251) as u8, ((x ^ y) % 251) as u8, 255])));
    place(&viewer, id, 0, big, Placement { center: [300.0, 600.0], size: [100.0, 100.0], rotation: 0.0, opacity: 1.0 });
    viewer.save(info.id, Some(with.clone())).unwrap();
    let xref = viewer.images(id, 0).unwrap()[0].xref.unwrap();
    viewer.edit(id, PageEdit::DeleteImage { page: 0, xref }).unwrap();
    viewer.save(id, Some(without.clone())).unwrap();
    let (a, b) = (std::fs::metadata(&with).unwrap().len(), std::fs::metadata(&without).unwrap().len());
    assert!(b + 100_000 < a, "deleting a picture should shrink the file: {a} -> {b} bytes");
}

#[test]
fn large_photos_are_shrunk_and_webp_works() {
    let big = RgbaImage::from_fn(5000, 3000, |x, y| Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255]));
    let mut jpeg = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(big).to_rgb8().write_to(&mut jpeg, ImageFormat::Jpeg).unwrap();
    let prepared = images::prepare(jpeg.get_ref()).unwrap();
    assert_eq!((prepared.width, prepared.height), (2400, 1440));
    assert!(prepared.bytes.starts_with(&[0xFF, 0xD8]), "a photo stays JPEG");
    assert!(prepared.bytes.len() < jpeg.get_ref().len());

    // WebP (which PDF cannot hold) becomes PNG, keeping transparency.
    let mut webp = Cursor::new(Vec::new());
    RgbaImage::from_fn(40, 20, |x, _| Rgba([0, 120, 0, if x < 20 { 255 } else { 0 }]))
        .write_to(&mut webp, ImageFormat::WebP)
        .unwrap();
    let prepared = images::prepare(webp.get_ref()).unwrap();
    assert_eq!((prepared.width, prepared.height), (40, 20));
    assert!(prepared.bytes.starts_with(b"\x89PNG"));

    // Small files are kept exactly as they are.
    let small = two_tone(30, 30);
    assert_eq!(images::prepare(&small).unwrap().bytes, small);

    let err = images::prepare(b"GIF89a not supported").unwrap_err();
    assert!(err.to_string().contains("UNSUPPORTED_IMAGE"), "{err}");
}

#[test]
fn signature_photo_becomes_clean_ink() {
    // Greyish paper with uneven light, a dark diagonal stroke, and specks.
    let (w, h) = (600u32, 300u32);
    let photo = RgbaImage::from_fn(w, h, |x, y| {
        let paper = 200 + ((x + y) / 30) as u8 % 30;
        let on_stroke = (y as i32 - (x as i32 / 3 + 80)).abs() < 5 && (100..500).contains(&x);
        let v = if on_stroke { 45 } else if (x * 31 + y * 17) % 97 == 0 { paper - 12 } else { paper };
        Rgba([v, v, v.saturating_add(5), 255])
    });
    let mut jpeg = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(photo).to_rgb8().write_to(&mut jpeg, ImageFormat::Jpeg).unwrap();

    let ink = [10, 40, 160];
    let out = images::signature_from_photo(jpeg.get_ref(), ink).unwrap();
    let sig = image::load_from_memory(&out).unwrap().to_rgba8();
    // Cropped close to the stroke (400 x ~133 px plus a margin).
    assert!(sig.width() < 470 && sig.width() > 400, "width {}", sig.width());
    assert!(sig.height() < 220, "height {}", sig.height());
    let alpha: Vec<u8> = sig.pixels().map(|p| p.0[3]).collect();
    let solid = alpha.iter().filter(|&&a| a > 200).count();
    let clear = alpha.iter().filter(|&&a| a == 0).count();
    assert!(clear > alpha.len() * 3 / 4, "paper must become transparent");
    assert!(solid > 1500, "the stroke must stay solid ({solid} px)");
    assert!(sig.pixels().filter(|p| p.0[3] > 200).all(|p| p.0[..3] == ink), "ink colour is applied");

    // A blank sheet is reported, not turned into an empty signature.
    let blank = png(&RgbaImage::from_pixel(100, 100, Rgba([230, 230, 230, 255])));
    assert!(images::signature_from_photo(&blank, ink).unwrap_err().to_string().contains("NO_SIGNATURE_FOUND"));
}
