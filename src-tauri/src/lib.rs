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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Results::default())
        .invoke_handler(tauri::generate_handler![app_info, run_self_test, render_result])
        .run(tauri::generate_context!())
        .expect("error while running Akkhor PDF Editor");
}
