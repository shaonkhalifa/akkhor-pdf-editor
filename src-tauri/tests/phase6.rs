//! Phase 6 acceptance tests: create, convert and forms (SRS Modules 7, 8),
//! plus moving text blocks.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use akkhor_pdf_editor_lib::engine::{
    convert::{self, Compression, PictureFormat, PictureLayout},
    extract::document_text,
    fonts::{bangla_faces, default_bangla, dev_fonts_dir, english_face, SANS},
    forms,
    images::{self, Placement},
    layout::{lines_to_pdf, Align, LayoutOptions, Style},
    textedit::{Decoration, Fit, Spot, TextPlacement},
    viewer::{DocInfo, PageEdit, Viewer},
};
use image::{ImageFormat, Rgba, RgbaImage};
use mupdf::pdf::PdfDocument;

fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn work_dir(test: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-output").join("phase6").join(test);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn style(size: f32) -> Style {
    Style {
        bangla: bangla_faces("Noto Sans Bengali", &dev_fonts_dir()),
        english: english_face(SANS),
        size,
        line_height: size * 1.3,
        bold: false,
        italic: false,
        color: [0, 0, 0],
        align: Align::Center,
    }
}

fn png(img: &RgbaImage) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

fn pixel(viewer: &Viewer, info: &DocInfo, page: i32, x: f32, y: f32) -> [u8; 3] {
    let rgba = viewer.render_rgba(info.id, page, 1.0).unwrap();
    let w = u32::from_le_bytes(rgba[0..4].try_into().unwrap()) as usize;
    let i = 8 + (y as usize * w + x as usize) * 4;
    [rgba[i], rgba[i + 1], rgba[i + 2]]
}

fn texts(viewer: &Viewer, info: &DocInfo, page: i32) -> Vec<String> {
    viewer.text_lines(info.id, page).unwrap().into_iter().map(|l| squash(&l.text)).collect()
}

#[test]
fn new_blank_pdf_is_untitled_until_saved() {
    let dir = work_dir("blank");
    let viewer = Viewer::new();
    let info = viewer.open_bytes(convert::blank_pdf([612.0, 792.0], 3).unwrap(), "Untitled.pdf").unwrap();
    assert_eq!(info.page_count, 3);
    assert_eq!(info.pages[0], [612.0, 792.0]);
    assert!(info.untitled && info.dirty);
    let err = viewer.save(info.id, None).unwrap_err().to_string();
    assert!(err.contains("NEEDS_SAVE_AS"), "{err}");
    let out = dir.join("letter.pdf");
    let info = viewer.save(info.id, Some(out.clone())).unwrap();
    assert!(!info.untitled && !info.dirty && info.name == "letter.pdf");
    viewer.save(info.id, None).unwrap();
}

#[test]
fn pictures_become_pages_and_pages_become_pictures() {
    let dir = work_dir("pictures");
    let tall = dir.join("tall.png");
    let wide = dir.join("wide.png");
    std::fs::write(&tall, png(&RgbaImage::from_pixel(300, 600, Rgba([200, 0, 0, 255])))).unwrap();
    std::fs::write(&wide, png(&RgbaImage::from_pixel(900, 300, Rgba([0, 0, 200, 255])))).unwrap();

    let viewer = Viewer::new();
    let fit = viewer.open_bytes(convert::pictures_to_pdf(&[tall.clone(), wide.clone()], PictureLayout::FitPicture).unwrap(), "p.pdf").unwrap();
    assert_eq!(fit.page_count, 2);
    assert_eq!(fit.pages[0], [144.0, 288.0], "300 x 600 px at 150 dpi");
    assert_eq!(fit.pages[1], [432.0, 144.0]);
    let [r, g, b] = pixel(&viewer, &fit, 0, 72.0, 144.0);
    assert!(r > 180 && g < 40 && b < 40, "red picture fills the page");

    let a4 = viewer.open_bytes(convert::pictures_to_pdf(&[tall, wide], PictureLayout::A4).unwrap(), "a4.pdf").unwrap();
    assert_eq!(a4.pages[0], [595.0, 842.0]);
    assert_eq!(a4.pages[1], [842.0, 595.0], "a wide picture gets a landscape page");
    let [r, g, b] = pixel(&viewer, &a4, 1, 421.0, 297.0);
    assert!(b > 180 && r < 40 && g < 40, "blue picture in the middle");
    assert_eq!(pixel(&viewer, &a4, 1, 10.0, 10.0), [255, 255, 255], "margin stays white");

    // Page 1 as pictures.
    let list = viewer.page(a4.id, 0).unwrap();
    let png_bytes = convert::page_picture(&list, 72.0, PictureFormat::Png).unwrap();
    let img = image::load_from_memory(&png_bytes).unwrap();
    assert_eq!((img.width(), img.height()), (595, 842));
    let jpg = convert::page_picture(&list, 150.0, PictureFormat::Jpg).unwrap();
    assert!(jpg.starts_with(&[0xFF, 0xD8]));
    assert_eq!(image::load_from_memory(&jpg).unwrap().width(), 1240);
    assert!(convert::pictures_to_pdf(&[], PictureLayout::A4).is_err());
}

#[test]
fn compression_shrinks_photos_and_keeps_text() {
    let dir = work_dir("compress");
    let line = "সংকুচিত PDF — লেখা ঠিক থাকবে";
    let base = lines_to_pdf(&[line], &LayoutOptions::a4(default_bangla()), &dev_fonts_dir()).unwrap();
    let mut doc = PdfDocument::from_bytes(&base).unwrap();
    // A sharp, noisy photo shown 200 pt wide: about 860 dpi.
    let photo = RgbaImage::from_fn(2400, 1600, |x, y| {
        let v = ((x * 7919 + y * 104729) ^ (x * y)) % 256;
        Rgba([v as u8, (v * 3 % 256) as u8, (255 - v) as u8, 255])
    });
    let bytes = images::prepare(&png(&photo)).unwrap().bytes;
    images::place(&mut doc, 0, &bytes, &Placement { center: [300.0, 400.0], size: [200.0, 133.0], rotation: 0.0, opacity: 1.0 }).unwrap();
    let mut original = Vec::new();
    doc.write_to(&mut original).unwrap();

    let low = convert::compress(&original, None, Compression::Low).unwrap();
    let medium = convert::compress(&original, None, Compression::Medium).unwrap();
    let high = convert::compress(&original, None, Compression::High).unwrap();
    assert!(low.len() <= original.len());
    assert!(medium.len() * 2 < original.len(), "medium: {} -> {}", original.len(), medium.len());
    assert!(high.len() < medium.len(), "high {} < medium {}", high.len(), medium.len());
    for out in [&low, &medium, &high] {
        assert!(squash(&document_text(out).unwrap()).contains(&squash(line)), "text survives compression");
    }
    std::fs::write(dir.join("medium.pdf"), &medium).unwrap();
}

#[test]
fn password_can_be_added_and_removed() {
    let dir = work_dir("password");
    let plain = lines_to_pdf(&["গোপন ফাইল"], &LayoutOptions::a4(default_bangla()), &dev_fonts_dir()).unwrap();
    let locked = convert::protect(&plain, None, "পাস১২৩", None).unwrap();
    let path = dir.join("locked.pdf");
    std::fs::write(&path, &locked).unwrap();
    let viewer = Viewer::new();
    assert!(viewer.open(&path, None).is_err());
    assert!(viewer.open(&path, Some("wrong".into())).is_err());
    let info = viewer.open(&path, Some("পাস১২৩".into())).unwrap();

    // Remove the password from the open (unlocked) document.
    let (bytes, password) = viewer.snapshot(info.id).unwrap();
    let open = convert::unprotect(&bytes, password.as_deref()).unwrap();
    let path2 = dir.join("open.pdf");
    std::fs::write(&path2, &open).unwrap();
    let info2 = Viewer::new().open(&path2, None).unwrap();
    assert_eq!(info2.page_count, 1);
    assert!(squash(&document_text(&open).unwrap()).contains("গোপনফাইল"));
    assert!(convert::protect(&plain, None, "", None).is_err());
}

#[test]
fn page_numbers_header_and_watermark() {
    let viewer = Viewer::new();
    let info = viewer.open_bytes(convert::blank_pdf([595.0, 842.0], 3).unwrap(), "d.pdf").unwrap();
    let id = info.id;
    let numbers = Decoration {
        text: "পৃষ্ঠা {n} / {total}".into(),
        spot: Spot::Bottom,
        margin: 30.0,
        rotation: 0.0,
        opacity: 1.0,
        bangla_digits: true,
        pages: vec![],
        first_number: 1,
    };
    viewer.edit(id, PageEdit::Decorate { item: numbers, style: style(10.0) }).unwrap();
    let header = Decoration { text: "Akkhor অফিস — Report".into(), spot: Spot::Top, ..dec_default() };
    viewer.edit(id, PageEdit::Decorate { item: header, style: style(11.0) }).unwrap();
    let mark = Decoration { text: "গোপনীয়".into(), spot: Spot::Middle, rotation: -45.0, opacity: 0.25, pages: vec![1], ..dec_default() };
    let info = viewer.edit(id, PageEdit::Decorate { item: mark, style: style(60.0) }).unwrap();

    for p in 0..3 {
        let lines = viewer.text_lines(id, p).unwrap();
        let want = format!("পৃষ্ঠা{}/৩", ['১', '২', '৩'][p as usize]);
        let num = lines.iter().find(|l| squash(&l.text) == want).unwrap_or_else(|| panic!("page {p}: {lines:#?}"));
        assert!(num.rect[3] <= 842.0 - 29.0 && num.rect[1] > 780.0, "number near the bottom: {:?}", num.rect);
        assert!((num.rect[0] + num.rect[2]) / 2.0 > 280.0 && (num.rect[0] + num.rect[2]) / 2.0 < 315.0, "centred: {:?}", num.rect);
        let head = lines.iter().find(|l| squash(&l.text) == squash("Akkhor অফিস — Report")).expect("header");
        assert!(head.rect[1] >= 29.0 && head.rect[1] < 50.0, "header near the top: {:?}", head.rect);
    }
    assert!(texts(&viewer, &info, 1).iter().any(|t| t.contains("গোপনীয়")), "watermark on page 2");
    assert!(!texts(&viewer, &info, 0).iter().any(|t| t.contains("গোপনীয়")), "only on page 2");
    // See-through: the middle of the watermark is light grey, not black.
    let [r, g, b] = pixel(&viewer, &info, 1, 297.0, 421.0);
    assert!(r > 150 && g > 150 && b > 150, "watermark should be faint: {:?}", [r, g, b]);
}

fn dec_default() -> Decoration {
    Decoration { text: String::new(), spot: Spot::Top, margin: 30.0, rotation: 0.0, opacity: 1.0, bangla_digits: false, pages: vec![], first_number: 1 }
}

/// A one-page form: a text field, a checkbox and a drop-down.
fn form_pdf(path: &Path) {
    let mut doc = PdfDocument::from_bytes(&convert::blank_pdf([595.0, 842.0], 1).unwrap()).unwrap();
    let page_ref = doc.find_page(0).unwrap();
    let helv = doc.add_object(&doc.new_object_from_str("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>").unwrap()).unwrap();
    let on = doc.add_stream(&mupdf::Buffer::from_bytes(b"0 g 2 2 12 12 re f").unwrap(), Some(&doc.new_object_from_str("<< /Type /XObject /Subtype /Form /BBox [0 0 16 16] >>").unwrap()), false).unwrap();
    let off = doc.add_stream(&mupdf::Buffer::from_bytes(b"").unwrap(), Some(&doc.new_object_from_str("<< /Type /XObject /Subtype /Form /BBox [0 0 16 16] >>").unwrap()), false).unwrap();
    let mut name = doc.add_object(&doc.new_object_from_str("<< /Type /Annot /Subtype /Widget /FT /Tx /T (name) /Rect [100 700 400 724] /F 4 /DA (/Helv 12 Tf 0 g) >>").unwrap()).unwrap();
    let mut agree = doc.add_object(&doc.new_object_from_str("<< /Type /Annot /Subtype /Widget /FT /Btn /T (agree) /Rect [100 650 116 666] /F 4 /V /Off /AS /Off /DA (/Helv 0 Tf 0 g) >>").unwrap()).unwrap();
    let mut ap = doc.new_dict().unwrap();
    let mut n = doc.new_dict().unwrap();
    n.dict_put("Yes", on).unwrap();
    n.dict_put("Off", off).unwrap();
    ap.dict_put("N", n).unwrap();
    agree.dict_put("AP", ap).unwrap();
    let mut city = doc
        .add_object(&doc.new_object_from_str("<< /Type /Annot /Subtype /Widget /FT /Ch /Ff 131072 /T (city) /Rect [100 600 300 620] /F 4 /DA (/Helv 11 Tf 0 g) >>").unwrap())
        .unwrap();
    let mut opt = doc.new_array().unwrap();
    for c in ["Dhaka", "ঢাকা", "Chattogram"] {
        opt.array_push(mupdf::pdf::PdfObject::new_string(c).unwrap()).unwrap();
    }
    city.dict_put("Opt", opt).unwrap();
    let mut annots = doc.new_array().unwrap();
    for w in [&mut name, &mut agree, &mut city] {
        w.dict_put("P", page_ref.clone()).unwrap();
        annots.array_push(w.clone()).unwrap();
    }
    let mut page_obj = page_ref.clone();
    page_obj.dict_put("Annots", annots.clone()).unwrap();
    let mut acro = doc.new_dict().unwrap();
    acro.dict_put("Fields", annots).unwrap();
    acro.dict_put("DA", mupdf::pdf::PdfObject::new_string("/Helv 0 Tf 0 g").unwrap()).unwrap();
    let mut dr = doc.new_dict().unwrap();
    let mut fonts = doc.new_dict().unwrap();
    fonts.dict_put("Helv", helv).unwrap();
    dr.dict_put("Font", fonts).unwrap();
    acro.dict_put("DR", dr).unwrap();
    doc.catalog().unwrap().dict_put("AcroForm", acro).unwrap();
    doc.save(path.to_str().unwrap()).unwrap();
}

#[test]
fn fill_bangla_form_save_and_flatten() {
    let dir = work_dir("forms");
    let path = dir.join("form.pdf");
    form_pdf(&path);
    let viewer = Viewer::new();
    let info = viewer.open(&path, None).unwrap();
    assert!(info.has_form);
    let fields = viewer.fields(info.id, 0).unwrap();
    let kinds: Vec<&str> = fields.iter().map(|f| f.kind).collect();
    assert_eq!(kinds, ["text", "checkbox", "combo"], "{fields:#?}");
    assert_eq!(fields[1].on_value.as_deref(), Some("Yes"));
    assert_eq!(fields[2].options.len(), 3);

    let name = "আমার নাম অক্ষর Akkhor";
    let id = info.id;
    viewer.edit(id, PageEdit::SetField { page: 0, xref: fields[0].xref, value: name.into(), style: style(10.0) }).unwrap();
    viewer.edit(id, PageEdit::SetField { page: 0, xref: fields[1].xref, value: "true".into(), style: style(10.0) }).unwrap();
    let info = viewer.edit(id, PageEdit::SetField { page: 0, xref: fields[2].xref, value: "ঢাকা".into(), style: style(10.0) }).unwrap();

    let after = viewer.fields(id, 0).unwrap();
    assert_eq!(after[0].value, name);
    assert!(after[1].checked);
    assert_eq!(after[2].value, "ঢাকা");
    // The Bangla is drawn correctly in the field (shaped, readable text).
    let shown = texts(&viewer, &info, 0);
    assert!(shown.contains(&squash(name)), "{shown:?}");
    assert!(shown.contains(&squash("ঢাকা")), "{shown:?}");
    let [r, _, _] = pixel(&viewer, &info, 0, 108.0, 842.0 - 658.0);
    assert!(r < 60, "the checkbox is ticked");

    // Saved and reopened: values and appearances stay; readers keep ours.
    let filled = dir.join("filled.pdf");
    viewer.save(id, Some(filled.clone())).unwrap();
    let again = Viewer::new();
    let info2 = again.open(&filled, None).unwrap();
    let f2 = again.fields(info2.id, 0).unwrap();
    assert_eq!(f2[0].value, name);
    assert!(f2[1].checked);
    assert!(texts(&again, &info2, 0).contains(&squash(name)));
    let doc = PdfDocument::open(filled.to_str().unwrap()).unwrap();
    let need = doc.catalog().unwrap().get_dict("AcroForm").unwrap().unwrap().get_dict("NeedAppearances").unwrap();
    assert!(need.is_some_and(|n| !n.as_bool().unwrap()), "NeedAppearances must be false");

    // Flatten: no fields left, the text is part of the page.
    let info3 = again.edit(info2.id, PageEdit::Flatten).unwrap();
    assert!(again.fields(info2.id, 0).unwrap().is_empty());
    assert!(texts(&again, &info3, 0).contains(&squash(name)));
    // Untick works too (on the original document).
    let info = viewer.edit(id, PageEdit::SetField { page: 0, xref: fields[1].xref, value: "false".into(), style: style(10.0) }).unwrap();
    assert!(!viewer.fields(info.id, 0).unwrap()[1].checked);
    assert!(forms::has_form(&PdfDocument::open(path.to_str().unwrap()).unwrap()).unwrap());
}

#[test]
fn text_blocks_can_be_moved() {
    let dir = work_dir("move");
    let path = dir.join("m.pdf");
    let line = "সরানো যায় এমন লেখা — movable";
    std::fs::write(&path, lines_to_pdf(&[line], &LayoutOptions::a4(default_bangla()), &dev_fonts_dir()).unwrap()).unwrap();
    let viewer = Viewer::new();
    let info = viewer.open(&path, None).unwrap();
    let block = viewer.text_blocks(info.id, 0).unwrap().remove(0);
    let mut st = style(12.0);
    st.align = Align::Left;
    let place = TextPlacement {
        left: block.origin[0] + 150.0,
        width: block.max_width,
        baseline: Some(block.origin[1] + 300.0),
        top: None,
        max_height: None,
        remove: block.lines.clone(),
        fit: Fit::Overflow,
    };
    let info = viewer.edit(info.id, PageEdit::PlaceText { page: 0, text: block.text.clone(), style: st, placement: place }).unwrap();
    let moved = viewer.text_blocks(info.id, 0).unwrap();
    assert_eq!(moved.len(), 1, "the old copy is gone: {moved:#?}");
    assert_eq!(squash(&moved[0].text), squash(line));
    assert!((moved[0].origin[1] - (block.origin[1] + 300.0)).abs() < 1.5);
    assert!((moved[0].origin[0] - (block.origin[0] + 150.0)).abs() < 2.5);
}
