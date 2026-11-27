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
use engine::pages::{self, MergeInput};
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
            recent_files,
            clear_recent_files,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Akkhor PDF Editor");
}
