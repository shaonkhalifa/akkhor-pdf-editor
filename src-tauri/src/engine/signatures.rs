//! "My Signatures" (SRS FR-5.5): signatures saved for reuse.
//!
//! Each signature is one file in the app's data folder, encrypted with
//! Windows data protection (DPAPI). The key belongs to the user's Windows
//! login and is managed by Windows itself, so the files are useless if copied
//! to another account or PC, and the app never stores a key or password.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};

/// What is stored (encrypted) for each signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Stored {
    label: String,
    /// Transparent PNG, base64.
    png: String,
    created: u64,
}

/// A saved signature as the UI sees it.
#[derive(Debug, Clone, Serialize)]
pub struct SavedSignature {
    pub id: String,
    pub label: String,
    /// `data:image/png;base64,...`, ready for an <img>.
    pub data_url: String,
    pub created: u64,
}

const EXT: &str = "akkhorsig";
/// Extra entropy mixed into the encryption, so other apps using DPAPI
/// cannot decrypt these files by accident.
const ENTROPY: &[u8] = b"Akkhor PDF Editor signature v1";

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 40 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub fn save(dir: &Path, label: &str, png: &[u8]) -> Result<SavedSignature> {
    if png.len() > 8 * 1024 * 1024 {
        bail!("the signature picture is too large");
    }
    image::load_from_memory_with_format(png, image::ImageFormat::Png).context("the signature is not a PNG picture")?;
    std::fs::create_dir_all(dir)?;
    let created = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let id = format!("{created:x}-{:04x}", std::process::id() as u16 ^ (created as u16).rotate_left(7));
    let label: String = label.trim().chars().take(60).collect();
    let stored = Stored { label: label.clone(), png: base64::engine::general_purpose::STANDARD.encode(png), created };
    let sealed = protect(&serde_json::to_vec(&stored)?)?;
    super::pages::write_file_safely(&dir.join(format!("{id}.{EXT}")), &sealed)?;
    Ok(SavedSignature { id, label, data_url: data_url(&stored.png), created })
}

fn data_url(b64: &str) -> String {
    format!("data:image/png;base64,{b64}")
}

/// All saved signatures, newest first. Files that cannot be decrypted (for
/// example copied from another PC) are skipped.
pub fn list(dir: &Path) -> Result<Vec<SavedSignature>> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Ok(Vec::new()) };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(EXT) {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else { continue };
        let Ok(sealed) = std::fs::read(&path) else { continue };
        let Ok(plain) = unprotect(&sealed) else { continue };
        let Ok(stored) = serde_json::from_slice::<Stored>(&plain) else { continue };
        out.push(SavedSignature { id, label: stored.label, data_url: data_url(&stored.png), created: stored.created });
    }
    out.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(out)
}

/// The PNG of one saved signature.
pub fn load_png(dir: &Path, id: &str) -> Result<Vec<u8>> {
    let stored: Stored = serde_json::from_slice(&unprotect(&std::fs::read(path_of(dir, id)?)?)?)?;
    Ok(base64::engine::general_purpose::STANDARD.decode(stored.png)?)
}

pub fn delete(dir: &Path, id: &str) -> Result<()> {
    let path = path_of(dir, id)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn path_of(dir: &Path, id: &str) -> Result<PathBuf> {
    if !valid_id(id) {
        bail!("not a signature id: {id}");
    }
    Ok(dir.join(format!("{id}.{EXT}")))
}

#[cfg(windows)]
fn protect(data: &[u8]) -> Result<Vec<u8>> {
    dpapi(data, true)
}

#[cfg(windows)]
fn unprotect(data: &[u8]) -> Result<Vec<u8>> {
    dpapi(data, false)
}

#[cfg(windows)]
fn dpapi(data: &[u8], encrypt: bool) -> Result<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let entropy = CRYPT_INTEGER_BLOB { cbData: ENTROPY.len() as u32, pbData: ENTROPY.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: the input blobs point at live slices for the duration of the
    // call; on success Windows allocates `output`, which is copied and then
    // released with LocalFree exactly once.
    let ok = unsafe {
        if encrypt {
            CryptProtectData(&input, std::ptr::null(), &entropy, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        } else {
            CryptUnprotectData(&input, std::ptr::null_mut(), &entropy, std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        }
    };
    if ok == 0 || output.pbData.is_null() {
        return Err(anyhow!("Windows could not {} the signature", if encrypt { "encrypt" } else { "decrypt" }));
    }
    let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe { LocalFree(output.pbData as _) };
    Ok(bytes)
}

// Akkhor is Windows-only for now (SRS 2.4). Elsewhere, refuse to store
// signatures rather than keep them unencrypted.
#[cfg(not(windows))]
fn protect(_: &[u8]) -> Result<Vec<u8>> {
    bail!("saving signatures needs Windows")
}

#[cfg(not(windows))]
fn unprotect(_: &[u8]) -> Result<Vec<u8>> {
    bail!("saved signatures need Windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn signatures_are_encrypted_and_come_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(4, 2, image::Rgba([0, 0, 200, 255]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let png = png.into_inner();
        let saved = save(dir.path(), "আমার সই", &png).unwrap();

        // The file on disk does not contain the picture or the label.
        let raw = std::fs::read(dir.path().join(format!("{}.{EXT}", saved.id))).unwrap();
        assert!(!raw.windows(4).any(|w| w == b"\x89PNG"));
        assert!(!String::from_utf8_lossy(&raw).contains("iVBOR"));

        let all = list(dir.path()).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].label, "আমার সই");
        assert_eq!(load_png(dir.path(), &saved.id).unwrap(), png);

        // A tampered file is skipped, not shown.
        std::fs::write(dir.path().join(format!("bad.{EXT}")), b"not encrypted").unwrap();
        assert_eq!(list(dir.path()).unwrap().len(), 1);

        delete(dir.path(), &saved.id).unwrap();
        assert!(list(dir.path()).unwrap().is_empty());
        assert!(load_png(dir.path(), "../escape").is_err());
    }
}
