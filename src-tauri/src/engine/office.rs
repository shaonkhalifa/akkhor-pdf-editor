//! Word to PDF (SRS FR-7.7) using an office program already on the user's
//! PC: LibreOffice (free) or Microsoft Word. Akkhor bundles neither; if
//! neither is installed the feature says so. Nothing is sent anywhere: the
//! office program runs hidden on this PC and writes a PDF to a temp folder.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct OfficeStatus {
    pub libreoffice: bool,
    pub word: bool,
}

fn program_files() -> Vec<PathBuf> {
    ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
        .iter()
        .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
        .collect()
}

fn libreoffice() -> Option<PathBuf> {
    program_files()
        .into_iter()
        .map(|p| p.join("LibreOffice").join("program").join("soffice.exe"))
        .find(|p| p.is_file())
}

fn word() -> Option<PathBuf> {
    for base in program_files() {
        for office in ["root\\Office16", "Office16", "root\\Office15", "Office15"] {
            let exe = base.join("Microsoft Office").join(office).join("WINWORD.EXE");
            if exe.is_file() {
                return Some(exe);
            }
        }
    }
    None
}

pub fn status() -> OfficeStatus {
    OfficeStatus { libreoffice: libreoffice().is_some(), word: word().is_some() }
}

#[cfg(windows)]
fn hidden(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW)
}

#[cfg(not(windows))]
fn hidden(cmd: &mut Command) -> &mut Command {
    cmd
}

/// Run a command, giving up after `limit`.
fn run(cmd: &mut Command, limit: Duration) -> Result<()> {
    let mut child = hidden(cmd).spawn().context("starting the office program")?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            if !status.success() {
                bail!("the office program could not convert this file");
            }
            return Ok(());
        }
        if started.elapsed() > limit {
            let _ = child.kill();
            bail!("the office program took too long");
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// Convert a Word (or other office) document to PDF bytes.
pub fn to_pdf(input: &Path) -> Result<Vec<u8>> {
    if !input.is_file() {
        bail!("{} was not found", input.display());
    }
    let work = tempfile::tempdir()?;
    let stem = input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "document".into());
    if let Some(soffice) = libreoffice() {
        // A private profile folder, so a running LibreOffice is not disturbed.
        let profile = work.path().join("profile");
        let profile_url = format!("file:///{}", profile.to_string_lossy().replace('\\', "/"));
        run(
            Command::new(soffice)
                .arg(format!("-env:UserInstallation={profile_url}"))
                .args(["--headless", "--norestore", "--convert-to", "pdf", "--outdir"])
                .arg(work.path())
                .arg(input),
            Duration::from_secs(180),
        )?;
        let out = work.path().join(format!("{stem}.pdf"));
        return std::fs::read(&out).map_err(|_| anyhow!("LibreOffice did not produce a PDF"));
    }
    if word().is_some() {
        let out = work.path().join(format!("{stem}.pdf"));
        // Paths go in through environment variables, never into the script.
        let script = "$ErrorActionPreference='Stop'; $w = New-Object -ComObject Word.Application; $w.Visible = $false; \
                      try { $d = $w.Documents.Open($env:AKKHOR_IN, $false, $true); $d.SaveAs2($env:AKKHOR_OUT, 17); $d.Close($false) } \
                      finally { $w.Quit() }";
        run(
            Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .env("AKKHOR_IN", input)
                .env("AKKHOR_OUT", &out),
            Duration::from_secs(180),
        )?;
        return std::fs::read(&out).map_err(|_| anyhow!("Word did not produce a PDF"));
    }
    bail!("NO_OFFICE")
}
