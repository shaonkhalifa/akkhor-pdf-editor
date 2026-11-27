//! Filling PDF forms (SRS Module 8): text fields, checkboxes, radio buttons
//! and drop-downs, in Bangla and English.
//!
//! MuPDF draws field contents with Helvetica, which has no Bangla letters.
//! So Akkhor sets each field's value itself and draws its appearance with
//! the same HarfBuzz layout used for editing, then tells readers to use that
//! appearance. Adobe Reader and Chrome then show the Bangla correctly
//! (FR-8.2). Flattening turns the fields into ordinary page content (FR-8.3).

use anyhow::{bail, Context, Result};
use mupdf::pdf::{PdfDocument, PdfObject, WidgetType};
use mupdf::Buffer;
use serde::Serialize;

use super::layout::{Align, Style};
use super::textedit::{laid_xobject, lay_out, wrap};

#[derive(Debug, Clone, Serialize)]
pub struct FormField {
    /// Identifies the widget on its page.
    pub xref: i32,
    /// "text", "checkbox", "radio", "combo", "list", "signature" or "button".
    pub kind: &'static str,
    pub name: String,
    pub value: String,
    /// x0, y0, x1, y1 in page points.
    pub rect: [f32; 4],
    /// Choices for drop-downs and lists: (value, label).
    pub options: Vec<(String, String)>,
    /// For checkboxes and radio buttons: the value meaning "ticked".
    pub on_value: Option<String>,
    pub checked: bool,
    pub readonly: bool,
    pub multiline: bool,
}

fn kind_name(t: WidgetType) -> &'static str {
    match t {
        WidgetType::Text => "text",
        WidgetType::Checkbox => "checkbox",
        WidgetType::RadioButton => "radio",
        WidgetType::Combobox => "combo",
        WidgetType::Listbox => "list",
        WidgetType::Signature => "signature",
        _ => "button",
    }
}

/// The field dictionary a widget belongs to (the one with the /T name).
fn field_of(widget: &PdfObject) -> Result<PdfObject> {
    let mut obj = widget.clone();
    for _ in 0..16 {
        if obj.get_dict("T")?.is_some() {
            return Ok(obj);
        }
        match obj.get_dict("Parent")? {
            Some(p) => obj = p,
            None => break,
        }
    }
    Ok(widget.clone())
}

/// The appearance state names of a checkbox or radio widget, other than "Off".
fn on_state(widget: &PdfObject) -> Result<Option<String>> {
    let Some(ap) = widget.get_dict("AP")? else { return Ok(None) };
    let Some(n) = ap.get_dict("N")? else { return Ok(None) };
    if !n.is_dict()? {
        return Ok(None);
    }
    for i in 0..n.dict_len()? {
        if let Some(k) = n.get_dict_key(i as i32)? {
            let name = String::from_utf8_lossy(&k.as_name()?).into_owned();
            if name != "Off" {
                return Ok(Some(name));
            }
        }
    }
    Ok(None)
}

fn text_of(obj: Option<PdfObject>) -> String {
    match obj {
        Some(o) if o.is_name().unwrap_or(false) => o.as_name().map(|n| String::from_utf8_lossy(&n).into_owned()).unwrap_or_default(),
        Some(o) => o.as_string().unwrap_or_default(),
        None => String::new(),
    }
}

fn choices(field: &PdfObject) -> Result<Vec<(String, String)>> {
    let Some(opt) = field.get_dict_inheritable("Opt")? else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for i in 0..opt.len()? {
        let Some(item) = opt.get_array(i as i32)? else { continue };
        if item.is_array()? {
            let value = text_of(item.get_array(0)?);
            let label = text_of(item.get_array(1)?);
            out.push((value, label));
        } else {
            let v = text_of(Some(item));
            out.push((v.clone(), v));
        }
    }
    Ok(out)
}

/// Does the document have any form fields?
pub fn has_form(doc: &PdfDocument) -> Result<bool> {
    Ok(doc.has_acro_form()?)
}

/// The fields on a page.
pub fn fields(doc: &PdfDocument, page_no: i32) -> Result<Vec<FormField>> {
    let page = doc.load_pdf_page(page_no)?;
    let mut out = Vec::new();
    for w in page.widgets() {
        let obj = w.annotation().object();
        let field = field_of(&obj)?;
        let kind = kind_name(w.r#type()?);
        let b = w.annotation().bounds()?;
        let flags = w.field_flags()?.bits();
        let value = text_of(field.get_dict_inheritable("V")?);
        let on_value = if matches!(kind, "checkbox" | "radio") { on_state(&obj)? } else { None };
        let state = text_of(obj.get_dict("AS")?);
        out.push(FormField {
            xref: w.xref()?,
            kind,
            name: w.name()?.unwrap_or_default(),
            checked: on_value.as_ref().is_some_and(|on| state == *on || (state.is_empty() && value == *on)),
            value,
            rect: [b.x0, b.y0, b.x1, b.y1],
            options: if matches!(kind, "combo" | "list") { choices(&field)? } else { Vec::new() },
            on_value,
            readonly: w.is_readonly()?,
            multiline: flags & (1 << 12) != 0,
        });
    }
    Ok(out)
}

/// Font size and colour from a field's default appearance ("/Helv 12 Tf 0 g").
fn default_appearance(field: &PdfObject) -> (f32, [u8; 3]) {
    let da = field.get_dict_inheritable("DA").ok().flatten().and_then(|d| d.as_string().ok()).unwrap_or_default();
    let tokens: Vec<&str> = da.split_whitespace().collect();
    let mut size = 0.0;
    let mut color = [0u8; 3];
    for (i, t) in tokens.iter().enumerate() {
        let num = |k: usize| tokens.get(i.wrapping_sub(k)).and_then(|s| s.parse::<f32>().ok());
        match *t {
            "Tf" => size = num(1).unwrap_or(0.0),
            "g" => {
                let v = (num(1).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0) as u8;
                color = [v, v, v];
            }
            "rg" => {
                if let (Some(r), Some(g), Some(b)) = (num(3), num(2), num(1)) {
                    color = [r, g, b].map(|c| (c.clamp(0.0, 1.0) * 255.0) as u8);
                }
            }
            _ => {}
        }
    }
    (size, color)
}

/// Draw a text field's (or drop-down's) value as its appearance.
fn draw_text_appearance(doc: &mut PdfDocument, widget: &mut PdfObject, field: &PdfObject, text: &str, multiline: bool, base: &Style) -> Result<()> {
    let rect = widget.get_dict("Rect")?.context("the field has no position")?;
    let n: Vec<f32> = (0..4).map(|i| rect.get_array(i).ok().flatten().and_then(|v| v.as_float().ok()).unwrap_or(0.0)).collect();
    let (w, h) = ((n[2] - n[0]).abs(), (n[3] - n[1]).abs());
    if w < 2.0 || h < 2.0 {
        bail!("the field is too small");
    }
    let (da_size, color) = default_appearance(field);
    let mut style = base.clone();
    style.color = color;
    style.align = match field.get_dict_inheritable("Q")?.and_then(|q| q.as_int().ok()) {
        Some(1) => Align::Center,
        Some(2) => Align::Right,
        _ => Align::Left,
    };
    style.size = if da_size > 0.0 { da_size } else if multiline { 10.0 } else { ((h - 2.0) * 0.7).clamp(4.0, 12.0) };
    style.line_height = style.size * 1.2;
    let inner = (w - 4.0).max(1.0);

    let mut appearance = format!("/Tx BMC\nEMC\n");
    let mut resources = doc.new_dict()?;
    if !text.trim().is_empty() {
        let mut lines = if multiline { wrap(text, &style, inner)? } else { vec![text.replace('\n', " ")] };
        // One-line fields: shrink long text until it fits the width.
        if !multiline {
            loop {
                let laid = lay_out(&lines, &style, 20000.0)?;
                let measured = PdfDocument::from_bytes(&laid.pdf)?;
                let used = super::extract::page_lines(&measured, 0)?.iter().map(|r| r.bbox.x1).fold(0.0, f32::max);
                if used <= inner || style.size <= 4.0 {
                    break;
                }
                style.size = (style.size * inner / used).max(4.0);
                style.line_height = style.size * 1.2;
            }
            lines = vec![text.replace('\n', " ")];
        }
        let laid = lay_out(&lines, &style, inner)?;
        let xobj = laid_xobject(doc, &laid)?;
        let mut xobjects = doc.new_dict()?;
        xobjects.dict_put("Fm0", xobj)?;
        resources.dict_put("XObject", xobjects)?;
        // Baseline: centred for one line, from the top for several.
        let baseline = if multiline { h - 2.0 - (laid.first_baseline - laid.first_top) } else { (h - style.size * 0.7) / 2.0 };
        let ty = baseline - (laid.height - laid.first_baseline);
        appearance = format!("/Tx BMC\nq 1 0 0 1 2 {ty:.3} cm /Fm0 Do Q\nEMC\n");
    }
    let mut bbox = doc.new_array()?;
    for v in [0.0, 0.0, w, h] {
        bbox.array_push(PdfObject::new_real(v)?)?;
    }
    let mut dict = doc.new_dict()?;
    dict.dict_put("Type", PdfObject::new_name("XObject")?)?;
    dict.dict_put("Subtype", PdfObject::new_name("Form")?)?;
    dict.dict_put("BBox", bbox)?;
    dict.dict_put("Resources", resources)?;
    let stream = doc.add_stream(&Buffer::from_bytes(appearance.as_bytes())?, Some(&dict), false)?;
    let mut ap = doc.new_dict()?;
    ap.dict_put("N", stream)?;
    widget.dict_put("AP", ap)?;
    Ok(())
}

/// Tell readers to use the appearances Akkhor drew instead of making their own.
fn keep_our_appearances(doc: &mut PdfDocument) -> Result<()> {
    if let Some(mut acro) = doc.catalog()?.get_dict("AcroForm")? {
        acro.dict_put("NeedAppearances", PdfObject::new_bool(false))?;
    }
    Ok(())
}

/// Fill in one field. For checkboxes, "true"/"false" (or the on/off value);
/// for radio buttons, the chosen widget's on value; for drop-downs, the value.
pub fn set_field(doc: &mut PdfDocument, page_no: i32, xref: i32, value: &str, style: &Style) -> Result<()> {
    let page = doc.load_pdf_page(page_no)?;
    let widget = page.widgets().find(|w| w.xref().ok() == Some(xref)).context("FIELD_NOT_FOUND")?;
    if widget.is_readonly()? {
        bail!("FIELD_READ_ONLY");
    }
    let kind = kind_name(widget.r#type()?);
    let flags = widget.field_flags()?.bits();
    let mut obj = widget.annotation().object();
    let mut field = field_of(&obj)?;
    match kind {
        "text" | "combo" => {
            field.dict_put("V", PdfObject::new_string(value)?)?;
            draw_text_appearance(doc, &mut obj, &field, value, flags & (1 << 12) != 0, style)?;
        }
        "list" => field.dict_put("V", PdfObject::new_string(value)?)?,
        "checkbox" | "radio" => {
            let on = on_state(&obj)?.unwrap_or_else(|| "Yes".into());
            let ticked = matches!(value, "true" | "1") || value == on;
            let state = if ticked { on.clone() } else { "Off".into() };
            field.dict_put("V", PdfObject::new_name(&state)?)?;
            // Every widget of the field shows its own state: radio buttons
            // that share a name turn off when another is chosen.
            let kids = field.get_dict("Kids")?;
            let widgets: Vec<PdfObject> = match kids {
                Some(k) => (0..k.len()?).filter_map(|i| k.get_array(i as i32).ok().flatten()).collect(),
                None => vec![obj.clone()],
            };
            for mut w in widgets {
                let mine = on_state(&w)?;
                let show = if ticked && mine.as_deref() == Some(on.as_str()) { on.clone() } else { "Off".into() };
                w.dict_put("AS", PdfObject::new_name(&show)?)?;
            }
        }
        _ => bail!("this kind of field can't be filled in here"),
    }
    keep_our_appearances(doc)?;
    Ok(())
}

/// Make the filled fields part of the page; they can no longer be changed.
pub fn flatten(doc: &mut PdfDocument) -> Result<()> {
    doc.bake(false, true)?;
    Ok(())
}
