//! Akkhor PDF Editor — Tauri backend.
//!
//! The UI never touches files or PDFs directly: it calls the commands below,
//! and all PDF work happens in `engine`, on the user's own PC.

pub mod engine;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{ipc::Response, path::BaseDirectory, AppHandle, Manager, State};

use engine::fonts::{default_bangla, dev_fonts_dir};
use engine::selftest::{check_all_fonts, run_edit_spike, FontCheck, SpikeResult};
use engine::images::{self, PageImage, Placement};
use engine::pages::{self, MergeInput};
use engine::fonts::FontOption;
use engine::layout::{Align, Style};
use engine::signatures::{self, SavedSignature};
use engine::textedit::{Markup, TextBlock, TextPlacement};
use engine::viewer::{DocInfo, PageEdit, PageLink, SearchHit, TextLine, Viewer};

/// PDFs produced by the last self-test, kept in memory (never written to disk).
#[derive(Default)]
struct Results(Mutex<HashMap<String, Vec<u8>>>);

#[derive(Serialize)]
struct SelfTestReport {
    fonts: Vec<FontCheck>,
    spike: SpikeResult,
    all_passed: bool,
}

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    licence: &'static str,
    network: &'static str,
}

/// Bundled fonts: the app's resource folder, or src-tauri/fonts during development.
fn fonts_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .resolve("fonts", BaseDirectory::Resource)
        .ok()
        .filter(|p| p.join(default_bangla().regular).exists())
        .unwrap_or_else(dev_fonts_dir)
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Akkhor PDF Editor",
        version: env!("CARGO_PKG_VERSION"),
        licence: "AGPL-3.0-or-later",
        network: "none",
    }
}

/// Run the Bangla shaping check for every bundled font plus the text-edit spike.
#[tauri::command]
async fn run_self_test(app: AppHandle, results: State<'_, Results>) -> Result<SelfTestReport, String> {
    let dir = fonts_dir(&app);
    let (fonts, spike) = tauri::async_runtime::spawn_blocking(move || {
        let fonts = check_all_fonts(&dir)?;
        let spike = run_edit_spike(default_bangla(), &dir)?;
        anyhow::Ok((fonts, spike))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))?;

    let mut store = results.0.lock().map_err(|e| e.to_string())?;
    store.clear();
    for (i, f) in fonts.iter().enumerate() {
        store.insert(format!("font:{i}"), f.pdf.clone());
    }
    store.insert("spike:before".into(), spike.before_pdf.clone());
    store.insert("spike:after".into(), spike.after_pdf.clone());

    let all_passed = spike.passed && fonts.iter().all(|f| f.passed);
    Ok(SelfTestReport { fonts, spike, all_passed })
}

/// Render page 1 of a self-test PDF as PNG, sent to the UI as raw bytes.
#[tauri::command]
async fn render_result(key: String, zoom: f32, results: State<'_, Results>) -> Result<Response, String> {
    let pdf = results
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .get(&key)
        .cloned()
        .ok_or_else(|| format!("no result named {key}"))?;
    let png = tauri::async_runtime::spawn_blocking(move || engine::render::page_png(&pdf, 0, zoom))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))?;
    Ok(Response::new(png))
}

// ---------------------------------------------------------------------------
// Viewer (SRS Modules 1 and 2)
// ---------------------------------------------------------------------------

/// Files larger than this ask for confirmation before opening (FR-1.9).
const LARGE_FILE_BYTES: u64 = 500 * 1024 * 1024;
/// How many recent files to remember (FR-1.4).
const RECENT_MAX: usize = 10;

/// Open documents, all owned by the engine thread.
struct Docs {
    viewer: Viewer,
}

/// Run blocking PDF work off the UI thread and turn errors into text.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> anyhow::Result<T> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))
}

fn recent_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("recent.json"))
}

fn read_recent(app: &AppHandle) -> Vec<String> {
    recent_file(app)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_recent(app: &AppHandle, list: &[String]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(list).map_err(|e| e.to_string())?;
    std::fs::write(recent_file(app)?, json).map_err(|e| e.to_string())
}

/// Open a PDF chosen by the user (file dialog, drag and drop, or recent list).
///
/// Errors use stable codes the UI understands: `NEEDS_PASSWORD`,
/// `WRONG_PASSWORD` and `LARGE_FILE:<bytes>` (pass `force: true` to go on).
#[tauri::command]
async fn open_pdf(
    app: AppHandle,
    docs: State<'_, Docs>,
    path: String,
    password: Option<String>,
    force: Option<bool>,
) -> Result<DocInfo, String> {
    let path = PathBuf::from(path);
    let size = std::fs::metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?.len();
    if size > LARGE_FILE_BYTES && !force.unwrap_or(false) {
        return Err(format!("LARGE_FILE:{size}"));
    }
    let viewer = docs.viewer.clone();
    let p = path.clone();
    let info = tauri::async_runtime::spawn_blocking(move || viewer.open(&p, password))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    remember(&app, &path);
    Ok(info)
}

/// Put a file at the top of the recent files list.
fn remember(app: &AppHandle, path: &std::path::Path) {
    let mut recent = read_recent(app);
    let shown = path.to_string_lossy().into_owned();
    recent.retain(|p| p != &shown);
    recent.insert(0, shown);
    recent.truncate(RECENT_MAX);
    let _ = write_recent(app, &recent);
}

#[tauri::command]
fn close_pdf(docs: State<'_, Docs>, id: u64) {
    docs.viewer.close(id);
}

/// Draw a page as raw RGBA (8-byte width/height header, then pixels).
#[tauri::command]
async fn render_page(docs: State<'_, Docs>, id: u64, page: i32, scale: f32) -> Result<Response, String> {
    let viewer = docs.viewer.clone();
    let scale = scale.clamp(0.05, 8.0);
    blocking(move || viewer.render_rgba(id, page, scale)).await.map(Response::new)
}

/// Draw a page as PNG, for printing.
#[tauri::command]
async fn render_page_png(docs: State<'_, Docs>, id: u64, page: i32, scale: f32) -> Result<Response, String> {
    let viewer = docs.viewer.clone();
    let scale = scale.clamp(0.05, 8.0);
    blocking(move || viewer.render_png(id, page, scale)).await.map(Response::new)
}

#[tauri::command]
async fn page_text(docs: State<'_, Docs>, id: u64, page: i32) -> Result<Vec<TextLine>, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.text_lines(id, page)).await
}

#[tauri::command]
async fn page_links(docs: State<'_, Docs>, id: u64, page: i32) -> Result<Vec<PageLink>, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.links(id, page)).await
}

#[tauri::command]
async fn search_pdf(
    docs: State<'_, Docs>,
    id: u64,
    page_count: i32,
    needle: String,
) -> Result<Vec<SearchHit>, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.search(id, page_count, &needle)).await
}

// ---------------------------------------------------------------------------
// Page tools (SRS Module 3)
// ---------------------------------------------------------------------------

/// Rotate, delete, move or insert pages. Returns the updated document.
#[tauri::command]
async fn edit_pages(docs: State<'_, Docs>, id: u64, edit: PageEdit) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.edit(id, edit)).await
}

#[tauri::command]
async fn undo(docs: State<'_, Docs>, id: u64) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.undo(id)).await
}

#[tauri::command]
async fn redo(docs: State<'_, Docs>, id: u64) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.redo(id)).await
}

/// Save the document to its own file, or to `dest` (Save As). The file is
/// written to a temporary file first and then renamed over the target, so a
/// failed save never leaves a half-written file (SRS 4.5).
#[tauri::command]
async fn save_pdf(app: AppHandle, docs: State<'_, Docs>, id: u64, dest: Option<String>) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    let info = blocking(move || viewer.save(id, dest.map(PathBuf::from))).await?;
    remember(&app, std::path::Path::new(&info.path));
    Ok(info)
}

/// Copy the chosen pages into a new PDF file.
#[tauri::command]
async fn extract_pages(docs: State<'_, Docs>, id: u64, pages: Vec<i32>, dest: String) -> Result<(), String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.extract_to(id, &pages, std::path::Path::new(&dest))).await
}

/// How to split a document.
#[derive(serde::Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
enum SplitPlan {
    /// A new file every `n` pages.
    Every { n: i32 },
    /// Page ranges typed by the user, such as "1-3, 4-10".
    Ranges { text: String },
}

/// Split into several files in `dir`. Returns the paths written.
#[tauri::command]
async fn split_pdf(
    docs: State<'_, Docs>,
    id: u64,
    page_count: i32,
    plan: SplitPlan,
    dir: String,
    stem: String,
) -> Result<Vec<String>, String> {
    let viewer = docs.viewer.clone();
    blocking(move || {
        let groups = match plan {
            SplitPlan::Every { n } => pages::every_n(page_count, n),
            SplitPlan::Ranges { text } => pages::parse_ranges(&text, page_count)?,
        };
        let written = viewer.split_to(id, &groups, std::path::Path::new(&dir), &stem)?;
        Ok(written.into_iter().map(|p| p.to_string_lossy().into_owned()).collect())
    })
    .await
}

#[derive(serde::Deserialize)]
struct MergeFile {
    path: String,
    password: Option<String>,
}

/// Join several PDFs into a new file at `dest`.
#[tauri::command]
async fn merge_pdfs(files: Vec<MergeFile>, dest: String) -> Result<(), String> {
    blocking(move || {
        let inputs: Vec<MergeInput> = files
            .into_iter()
            .map(|f| MergeInput { path: PathBuf::from(f.path), password: f.password })
            .collect();
        let dest = PathBuf::from(dest);
        if inputs.iter().any(|i| std::path::absolute(&i.path).ok() == std::path::absolute(&dest).ok()) {
            anyhow::bail!("SAVE_OVER_INPUT");
        }
        let bytes = pages::merge(&inputs)?;
        pages::write_file_safely(&dest, &bytes)
    })
    .await
}

/// Check that a file is a PDF that opens, and whether it needs a password.
/// Used before merging or inserting. Returns the page count.
#[tauri::command]
async fn probe_pdf(path: String, password: Option<String>) -> Result<i32, String> {
    blocking(move || Ok(pages::load(std::path::Path::new(&path), password.as_deref())?.page_count()?)).await
}

// ---------------------------------------------------------------------------
// Images, stamps and signatures (SRS Modules 4 and 5)
// ---------------------------------------------------------------------------

/// A picture ready to place, for the UI to preview.
#[derive(Serialize)]
struct Picture {
    data_url: String,
    width: u32,
    height: u32,
}

fn picture(bytes: &[u8], width: u32, height: u32) -> Picture {
    let mime = if bytes.starts_with(&[0xFF, 0xD8]) { "image/jpeg" } else { "image/png" };
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);
    Picture { data_url: format!("data:{mime};base64,{b64}"), width, height }
}

/// The bytes inside a `data:image/...;base64,` URL.
fn from_data_url(url: &str) -> anyhow::Result<Vec<u8>> {
    let b64 = url
        .strip_prefix("data:")
        .and_then(|rest| rest.split_once(";base64,"))
        .map(|(_, b64)| b64)
        .ok_or_else(|| anyhow::anyhow!("not a picture"))?;
    Ok(base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)?)
}

/// Read a PNG, JPEG or WebP file chosen by the user, shrinking it if it is
/// very large (FR-4.4).
#[tauri::command]
async fn load_picture(path: String) -> Result<Picture, String> {
    blocking(move || {
        let prepared = images::prepare(&std::fs::read(&path)?)?;
        Ok(picture(&prepared.bytes, prepared.width, prepared.height))
    })
    .await
}

/// A photo of a signature on paper, turned into ink on a transparent background.
#[tauri::command]
async fn photo_signature(path: String, ink: [u8; 3]) -> Result<Picture, String> {
    blocking(move || {
        let png = images::signature_from_photo(&std::fs::read(&path)?, ink)?;
        let prepared = images::prepare(&png)?;
        Ok(picture(&prepared.bytes, prepared.width, prepared.height))
    })
    .await
}

/// Put a picture (a data URL from `load_picture`, a stamp or a signature) on a page.
#[tauri::command]
async fn place_image(
    docs: State<'_, Docs>,
    id: u64,
    page: i32,
    data_url: String,
    placement: Placement,
) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || {
        let bytes = images::prepare(&from_data_url(&data_url)?)?.bytes;
        viewer.edit(id, PageEdit::PlaceImage { page, bytes, placement })
    })
    .await
}

#[tauri::command]
async fn page_images(docs: State<'_, Docs>, id: u64, page: i32) -> Result<Vec<PageImage>, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.images(id, page)).await
}

#[tauri::command]
async fn delete_image(docs: State<'_, Docs>, id: u64, page: i32, xref: i32) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.edit(id, PageEdit::DeleteImage { page, xref })).await
}

#[tauri::command]
async fn replace_image(docs: State<'_, Docs>, id: u64, page: i32, xref: i32, path: String) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || {
        let bytes = images::prepare(&std::fs::read(&path)?)?.bytes;
        viewer.edit(id, PageEdit::ReplaceImage { page, xref, bytes })
    })
    .await
}

fn signature_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("signatures"))
}

#[tauri::command]
async fn list_signatures(app: AppHandle) -> Result<Vec<SavedSignature>, String> {
    let dir = signature_dir(&app)?;
    blocking(move || signatures::list(&dir)).await
}

#[tauri::command]
async fn save_signature(app: AppHandle, label: String, data_url: String) -> Result<SavedSignature, String> {
    let dir = signature_dir(&app)?;
    blocking(move || signatures::save(&dir, &label, &from_data_url(&data_url)?)).await
}

#[tauri::command]
async fn delete_signature(app: AppHandle, id: String) -> Result<(), String> {
    let dir = signature_dir(&app)?;
    blocking(move || signatures::delete(&dir, &id)).await
}

// ---------------------------------------------------------------------------
// Text editing (SRS Module 6)
// ---------------------------------------------------------------------------

/// Fonts the user can choose for text (bundled, built-in and installed).
#[tauri::command]
async fn font_list() -> Result<Vec<FontOption>, String> {
    blocking(|| Ok(engine::fonts::font_options())).await
}

#[tauri::command]
async fn text_blocks(docs: State<'_, Docs>, id: u64, page: i32) -> Result<Vec<TextBlock>, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.text_blocks(id, page)).await
}

/// The look of text as chosen in the UI (fonts by name).
#[derive(serde::Deserialize)]
struct StyleChoice {
    bangla_font: String,
    english_font: String,
    size: f32,
    line_height: Option<f32>,
    bold: bool,
    italic: bool,
    color: [u8; 3],
    align: Align,
}

/// Replace a block of text or add a new text box. Fails with
/// `TEXT_TOO_LONG:<lines needed>:<lines that fit>` when the text needs more
/// room and `placement.fit` is "check".
#[tauri::command]
async fn place_text(
    app: AppHandle,
    docs: State<'_, Docs>,
    id: u64,
    page: i32,
    text: String,
    style: StyleChoice,
    placement: TextPlacement,
) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    let dir = fonts_dir(&app);
    blocking(move || {
        let style = resolve_style(&style, &dir);
        viewer.edit(id, PageEdit::PlaceText { page, text, style, placement })
    })
    .await
}

/// Turn the fonts chosen by name into font files.
fn resolve_style(style: &StyleChoice, dir: &std::path::Path) -> Style {
    let size = style.size.clamp(2.0, 400.0);
    Style {
        bangla: engine::fonts::bangla_faces(&style.bangla_font, dir),
        english: engine::fonts::english_face(&style.english_font),
        size,
        line_height: style.line_height.filter(|lh| *lh > 0.0).unwrap_or(size * 1.25),
        bold: style.bold,
        italic: style.italic,
        color: style.color,
        align: style.align,
    }
}

#[tauri::command]
async fn markup_text(
    docs: State<'_, Docs>,
    id: u64,
    page: i32,
    markup: Markup,
    rect: [f32; 4],
    color: [u8; 3],
) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.edit(id, PageEdit::Markup { page, markup, rect, color })).await
}

/// Black out an area: everything under it is removed for good (FR-6.8).
#[tauri::command]
async fn redact_area(docs: State<'_, Docs>, id: u64, page: i32, rect: [f32; 4]) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.edit(id, PageEdit::Redact { page, rect })).await
}

// ---------------------------------------------------------------------------
// Create, convert and forms (SRS Modules 7 and 8)
// ---------------------------------------------------------------------------

/// A new blank PDF, opened in a new tab (not saved until the user saves it).
#[tauri::command]
async fn new_blank_pdf(docs: State<'_, Docs>, size: [f32; 2], pages: u32, name: String) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.open_bytes(engine::convert::blank_pdf(size, pages)?, &name)).await
}

/// One PDF from pictures, one per page, opened in a new tab.
#[tauri::command]
async fn pictures_to_pdf(
    docs: State<'_, Docs>,
    files: Vec<String>,
    layout: engine::convert::PictureLayout,
    name: String,
) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || {
        let files: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();
        viewer.open_bytes(engine::convert::pictures_to_pdf(&files, layout)?, &name)
    })
    .await
}

/// Save pages as PNG or JPG pictures in a folder. Returns the files written.
#[tauri::command]
async fn export_pictures(
    docs: State<'_, Docs>,
    id: u64,
    pages: Vec<i32>,
    dpi: f32,
    format: engine::convert::PictureFormat,
    dir: String,
    stem: String,
) -> Result<Vec<String>, String> {
    let viewer = docs.viewer.clone();
    blocking(move || {
        let dir = PathBuf::from(dir);
        let mut written = Vec::new();
        for page in pages {
            let list = viewer.page(id, page)?;
            let bytes = engine::convert::page_picture(&list, dpi, format)?;
            let path = engine::convert::picture_path(&dir, &stem, page, format);
            pages::write_file_safely(&path, &bytes)?;
            written.push(path.to_string_lossy().into_owned());
        }
        Ok(written)
    })
    .await
}

#[derive(Serialize)]
struct Sizes {
    before: u64,
    after: u64,
}

/// Save a smaller copy of the document at `dest`.
#[tauri::command]
async fn compress_pdf(
    docs: State<'_, Docs>,
    id: u64,
    level: engine::convert::Compression,
    dest: String,
) -> Result<Sizes, String> {
    let viewer = docs.viewer.clone();
    blocking(move || {
        let (bytes, password) = viewer.snapshot(id)?;
        let doc = mupdf::pdf::PdfDocument::from_bytes(&bytes)?;
        let mut doc = doc;
        if let Some(pw) = &password {
            if doc.needs_password()? {
                doc.authenticate(pw)?;
            }
        }
        let before = pages::to_bytes(&doc, pages::save_options())?.len() as u64;
        let out = engine::convert::compress(&bytes, password.as_deref(), level)?;
        pages::write_file_safely(std::path::Path::new(&dest), &out)?;
        Ok(Sizes { before, after: out.len() as u64 })
    })
    .await
}

/// Save a copy protected with a password (AES-256).
#[tauri::command]
async fn protect_pdf(docs: State<'_, Docs>, id: u64, user: String, owner: Option<String>, dest: String) -> Result<(), String> {
    let viewer = docs.viewer.clone();
    blocking(move || {
        let (bytes, password) = viewer.snapshot(id)?;
        let out = engine::convert::protect(&bytes, password.as_deref(), &user, owner.as_deref().filter(|o| !o.is_empty()))?;
        pages::write_file_safely(std::path::Path::new(&dest), &out)
    })
    .await
}

/// Save a copy without the password.
#[tauri::command]
async fn unprotect_pdf(docs: State<'_, Docs>, id: u64, dest: String) -> Result<(), String> {
    let viewer = docs.viewer.clone();
    blocking(move || {
        let (bytes, password) = viewer.snapshot(id)?;
        let out = engine::convert::unprotect(&bytes, password.as_deref())?;
        pages::write_file_safely(std::path::Path::new(&dest), &out)
    })
    .await
}

/// Page numbers, header, footer or watermark on many pages.
#[tauri::command]
async fn decorate_pages(
    app: AppHandle,
    docs: State<'_, Docs>,
    id: u64,
    item: engine::textedit::Decoration,
    style: StyleChoice,
) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    let dir = fonts_dir(&app);
    blocking(move || {
        let style = resolve_style(&style, &dir);
        viewer.edit(id, PageEdit::Decorate { item, style })
    })
    .await
}

#[tauri::command]
async fn form_fields(docs: State<'_, Docs>, id: u64, page: i32) -> Result<Vec<engine::forms::FormField>, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.fields(id, page)).await
}

/// Fill in one form field (Bangla or English).
#[tauri::command]
async fn set_field(app: AppHandle, docs: State<'_, Docs>, id: u64, page: i32, xref: i32, value: String) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    let dir = fonts_dir(&app);
    blocking(move || {
        let choice = StyleChoice {
            bangla_font: engine::fonts::default_bangla().name.to_string(),
            english_font: engine::fonts::SANS.to_string(),
            size: 10.0,
            line_height: None,
            bold: false,
            italic: false,
            color: [0, 0, 0],
            align: Align::Left,
        };
        let style = resolve_style(&choice, &dir);
        viewer.edit(id, PageEdit::SetField { page, xref, value, style })
    })
    .await
}

/// Make the filled form part of the pages.
#[tauri::command]
async fn flatten_form(docs: State<'_, Docs>, id: u64) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.edit(id, PageEdit::Flatten)).await
}

// ---------------------------------------------------------------------------
// Bijoy to Unicode (SRS Module 9)
// ---------------------------------------------------------------------------

/// Convert Bijoy text (typed or pasted) to Unicode Bangla.
#[tauri::command]
fn bijoy_to_unicode(text: String) -> String {
    engine::bijoy::to_unicode(&text)
}

/// Does this text look like Bijoy? (Used to offer conversion on paste.)
#[tauri::command]
fn looks_like_bijoy(text: String) -> bool {
    engine::bijoy::looks_like_bijoy(&text)
}

/// Convert all Bijoy text on some pages (all when empty) to Unicode.
#[tauri::command]
async fn convert_bijoy_pages(app: AppHandle, docs: State<'_, Docs>, id: u64, pages: Vec<i32>) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    let dir = fonts_dir(&app);
    blocking(move || {
        // SRS 6.3: SolaimanLipi if installed, else the bundled Noto Sans Bengali.
        let (bangla, _) = engine::fonts::match_original("SutonnyMJ", false, false);
        let choice = StyleChoice {
            bangla_font: bangla,
            english_font: engine::fonts::SANS.to_string(),
            size: 12.0,
            line_height: None,
            bold: false,
            italic: false,
            color: [0, 0, 0],
            align: Align::Left,
        };
        let style = resolve_style(&choice, &dir);
        viewer.edit(id, PageEdit::ConvertBijoy { pages, style })
    })
    .await
}

// ---------------------------------------------------------------------------
// OCR (SRS Module 10)
// ---------------------------------------------------------------------------

/// Is the OCR language pack installed?
#[tauri::command]
fn ocr_status() -> engine::ocr::OcrStatus {
    engine::ocr::status(&[])
}

fn ocr_dir() -> anyhow::Result<PathBuf> {
    engine::ocr::status(&[]).datadir.map(PathBuf::from).ok_or_else(|| anyhow::anyhow!("OCR_DATA_MISSING"))
}

/// Make one page searchable (undoable). Pages are done one at a time so the
/// window can show progress and stop.
#[tauri::command]
async fn ocr_page(docs: State<'_, Docs>, id: u64, page: i32, dpi: u32) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.edit(id, PageEdit::Ocr { page, datadir: ocr_dir()?, dpi })).await
}

/// Read the text in an area of a page.
#[tauri::command]
async fn ocr_area(docs: State<'_, Docs>, id: u64, page: i32, rect: [f32; 4]) -> Result<String, String> {
    let viewer = docs.viewer.clone();
    blocking(move || viewer.ocr_area(id, page, rect, &ocr_dir()?)).await
}

#[tauri::command]
fn office_status() -> engine::office::OfficeStatus {
    engine::office::status()
}

/// Convert a Word document with LibreOffice or Word, and open the PDF.
#[tauri::command]
async fn word_to_pdf(docs: State<'_, Docs>, path: String) -> Result<DocInfo, String> {
    let viewer = docs.viewer.clone();
    blocking(move || {
        let input = PathBuf::from(&path);
        let name = format!("{}.pdf", input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "document".into()));
        viewer.open_bytes(engine::office::to_pdf(&input)?, &name)
    })
    .await
}

#[tauri::command]
fn recent_files(app: AppHandle) -> Vec<String> {
    read_recent(&app)
}

#[tauri::command]
fn clear_recent_files(app: AppHandle) -> Result<(), String> {
    write_recent(&app, &[])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Results::default())
        .manage(Docs { viewer: Viewer::new() })
        .invoke_handler(tauri::generate_handler![
            app_info,
            run_self_test,
            render_result,
            open_pdf,
            close_pdf,
            render_page,
            render_page_png,
            page_text,
            page_links,
            search_pdf,
            edit_pages,
            undo,
            redo,
            save_pdf,
            extract_pages,
            split_pdf,
            merge_pdfs,
            probe_pdf,
            load_picture,
            photo_signature,
            place_image,
            page_images,
            delete_image,
            replace_image,
            list_signatures,
            save_signature,
            delete_signature,
            font_list,
            text_blocks,
            place_text,
            markup_text,
            redact_area,
            new_blank_pdf,
            pictures_to_pdf,
            export_pictures,
            compress_pdf,
            protect_pdf,
            unprotect_pdf,
            decorate_pages,
            form_fields,
            set_field,
            flatten_form,
            office_status,
            word_to_pdf,
            bijoy_to_unicode,
            looks_like_bijoy,
            convert_bijoy_pages,
            ocr_status,
            ocr_page,
            ocr_area,
            recent_files,
            clear_recent_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Akkhor PDF Editor");
}
