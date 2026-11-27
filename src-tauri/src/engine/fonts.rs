//! Bundled fonts (SRS section 6.1) and glyph coverage checks.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// A bundled Bangla font family: regular file and optional bold file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BanglaFont {
    pub name: &'static str,
    pub regular: &'static str,
    pub bold: Option<&'static str>,
}

/// Bangla fonts bundled with the app in Phase 1 (all SIL OFL).
pub const BANGLA_FONTS: &[BanglaFont] = &[
    BanglaFont {
        name: "Noto Sans Bengali",
        regular: "NotoSansBengali-Regular.ttf",
        bold: Some("NotoSansBengali-Bold.ttf"),
    },
    BanglaFont {
        name: "Noto Serif Bengali",
        regular: "NotoSerifBengali-Regular.ttf",
        bold: None,
    },
    BanglaFont {
        name: "Hind Siliguri",
        regular: "HindSiliguri-Regular.ttf",
        bold: Some("HindSiliguri-Bold.ttf"),
    },
];

/// The default Bangla font (Noto Sans Bengali).
pub fn default_bangla() -> BanglaFont {
    BANGLA_FONTS[0]
}

/// Find a bundled font by its family name.
pub fn by_name(name: &str) -> Option<BanglaFont> {
    BANGLA_FONTS.iter().copied().find(|f| f.name == name)
}

/// Fonts folder used by tests and `cargo run` (src-tauri/fonts).
pub fn dev_fonts_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fonts")
}

/// A loaded font used to ask "does this font have a glyph for this character?".
pub struct Coverage {
    font: mupdf::Font,
}

impl Coverage {
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path).with_context(|| format!("reading font {}", path.display()))?;
        let font = mupdf::Font::from_bytes("coverage", &bytes)
            .with_context(|| format!("loading font {}", path.display()))?;
        Ok(Self { font })
    }

    /// True if the font has a real glyph (not .notdef) for `c`.
    pub fn has(&self, c: char) -> bool {
        self.font.encode_character(c as i32).map(|g| g > 0).unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Fonts for text editing (SRS 6.1 to 6.3)
// ---------------------------------------------------------------------------

use super::layout::{EnglishFace, Faces};
use serde::Serialize;

/// A font family the user can pick for edited or new text.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FontOption {
    /// Name shown to the user and sent back when chosen.
    pub name: String,
    /// "bangla" or "english".
    pub script: &'static str,
    /// "bundled", "builtin" (MuPDF's standard fonts) or "system".
    pub source: &'static str,
    pub serif: bool,
}

/// English families built into MuPDF: same widths as the standard PDF fonts.
pub const SANS: &str = "Sans (Helvetica, Arial widths)";
pub const SERIF: &str = "Serif (Times widths)";
pub const MONO: &str = "Mono (Courier)";
const BUILTIN_ENGLISH: &[(&str, &str, bool)] = &[(SANS, "sans-serif", false), (SERIF, "serif", true), (MONO, "monospace", false)];

/// Windows fonts used when installed (never bundled; SRS 6.2). Files are
/// regular, bold, italic, bold italic.
const SYSTEM_FONTS: &[(&str, &str, bool, [&str; 4])] = &[
    ("Arial", "english", false, ["arial.ttf", "arialbd.ttf", "ariali.ttf", "arialbi.ttf"]),
    ("Times New Roman", "english", true, ["times.ttf", "timesbd.ttf", "timesi.ttf", "timesbi.ttf"]),
    ("Calibri", "english", false, ["calibri.ttf", "calibrib.ttf", "calibrii.ttf", "calibriz.ttf"]),
    ("Georgia", "english", true, ["georgia.ttf", "georgiab.ttf", "georgiai.ttf", "georgiaz.ttf"]),
    ("Verdana", "english", false, ["verdana.ttf", "verdanab.ttf", "verdanai.ttf", "verdanaz.ttf"]),
    ("Segoe UI", "english", false, ["segoeui.ttf", "segoeuib.ttf", "segoeuii.ttf", "segoeuiz.ttf"]),
    ("SolaimanLipi", "bangla", false, ["solaimanlipi.ttf", "", "", ""]),
    ("Kalpurush", "bangla", false, ["kalpurush.ttf", "", "", ""]),
    ("Nikosh", "bangla", false, ["nikosh.ttf", "", "", ""]),
    ("Vrinda", "bangla", false, ["vrinda.ttf", "vrindab.ttf", "", ""]),
];

/// Folders Windows installs fonts into: for all users, and for this user.
fn system_font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(win) = std::env::var_os("WINDIR") {
        dirs.push(PathBuf::from(win).join("Fonts"));
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts"));
    }
    dirs
}

/// Find a font file by name in the system font folders, ignoring case.
/// Bangla fonts are often installed under longer names, such as
/// "SolaimanLipi_22-02-2012.ttf", so a name that starts the same also counts.
fn find_system_file(file: &str) -> Option<PathBuf> {
    if file.is_empty() {
        return None;
    }
    let want = file.to_lowercase();
    let stem = want.trim_end_matches(".ttf").to_string();
    for dir in system_font_dirs() {
        let exact = dir.join(file);
        if exact.is_file() {
            return Some(exact);
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name == want || (name.starts_with(&stem) && (name.ends_with(".ttf") || name.ends_with(".otf"))) {
                return Some(e.path());
            }
        }
    }
    None
}

fn system_entry(name: &str, script: &str) -> Option<&'static (&'static str, &'static str, bool, [&'static str; 4])> {
    SYSTEM_FONTS.iter().find(|(n, s, ..)| n.eq_ignore_ascii_case(name) && *s == script)
}

fn system_faces(name: &str, script: &str) -> Option<Faces> {
    let (_, _, _, files) = system_entry(name, script)?;
    Some(Faces {
        regular: find_system_file(files[0])?,
        bold: find_system_file(files[1]),
        italic: find_system_file(files[2]),
        bold_italic: find_system_file(files[3]),
    })
}

fn bundled_faces(f: &BanglaFont, fonts_dir: &Path) -> Faces {
    Faces { regular: fonts_dir.join(f.regular), bold: f.bold.map(|b| fonts_dir.join(b)), italic: None, bold_italic: None }
}

/// Every font the user can choose: bundled Bangla fonts, MuPDF's built-in
/// English fonts, and supported fonts installed in Windows.
pub fn font_options() -> Vec<FontOption> {
    let mut out: Vec<FontOption> = BANGLA_FONTS
        .iter()
        .map(|f| FontOption { name: f.name.into(), script: "bangla", source: "bundled", serif: f.name.contains("Serif") })
        .collect();
    out.extend(BUILTIN_ENGLISH.iter().map(|(n, _, serif)| FontOption { name: (*n).into(), script: "english", source: "builtin", serif: *serif }));
    for (name, script, serif, files) in SYSTEM_FONTS {
        if find_system_file(files[0]).is_some() {
            out.push(FontOption { name: (*name).into(), script, source: "system", serif: *serif });
        }
    }
    out
}

/// The files for a Bangla font chosen by name (bundled first, then system).
/// Unknown names fall back to Noto Sans Bengali, which is always there.
pub fn bangla_faces(name: &str, fonts_dir: &Path) -> Faces {
    if let Some(f) = by_name(name) {
        return bundled_faces(&f, fonts_dir);
    }
    system_faces(name, "bangla").unwrap_or_else(|| bundled_faces(&default_bangla(), fonts_dir))
}

/// The English font chosen by name (built-in or system). Unknown names fall
/// back to the built-in sans-serif.
pub fn english_face(name: &str) -> EnglishFace {
    if let Some((_, css, _)) = BUILTIN_ENGLISH.iter().find(|(n, ..)| *n == name) {
        return EnglishFace::Generic { css: (*css).into() };
    }
    match system_faces(name, "english") {
        Some(faces) => EnglishFace::Files { faces },
        None => EnglishFace::Generic { css: "sans-serif".into() },
    }
}

fn squash(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect()
}

/// The fonts to use for text that was set in `original` (SRS 6.3): the same
/// font if it is installed, else the closest substitute, keeping serif or
/// sans. Returns (Bangla font name, English font name).
pub fn match_original(original: &str, serif: bool, mono: bool) -> (String, String) {
    let lower = squash(original);
    let installed = |name: &str, script: &str| system_entry(name, script).is_some() && system_faces(name, script).is_some();

    let english = if mono || ["courier", "mono", "consolas"].iter().any(|w| lower.contains(w)) {
        MONO.to_string()
    } else {
        let named = SYSTEM_FONTS
            .iter()
            .filter(|(_, s, ..)| *s == "english")
            .map(|(n, ..)| *n)
            .find(|n| lower.contains(&squash(n)) || (*n == "Times New Roman" && lower.contains("times")))
            .or(if lower.contains("helvetica") { Some("Arial") } else { None });
        match named {
            Some(n) if installed(n, "english") => n.to_string(),
            Some("Times New Roman") | Some("Georgia") => SERIF.into(),
            Some(_) => SANS.into(),
            None if serif => SERIF.into(),
            None => SANS.into(),
        }
    };

    let bangla_named = BANGLA_FONTS
        .iter()
        .map(|f| f.name)
        .chain(SYSTEM_FONTS.iter().filter(|(_, s, ..)| *s == "bangla").map(|(n, ..)| *n))
        .find(|n| lower.contains(&squash(n)));
    let bangla = match bangla_named {
        Some(n) if by_name(n).is_some() || installed(n, "bangla") => n.to_string(),
        _ if serif => "Noto Serif Bengali".to_string(),
        _ if installed("SolaimanLipi", "bangla") => "SolaimanLipi".to_string(),
        _ => default_bangla().name.to_string(),
    };
    (bangla, english)
}

#[cfg(test)]
mod matching_tests {
    use super::*;

    #[test]
    fn originals_map_to_installed_fonts_or_substitutes() {
        let (bn, en) = match_original("NotoSerifBengali-Regular", true, false);
        assert_eq!(bn, "Noto Serif Bengali");
        assert_eq!(en, SERIF);
        let (_, en) = match_original("CourierNewPSMT", false, true);
        assert_eq!(en, MONO);
        let (bn, en) = match_original("SomeUnknownSans", false, false);
        assert!(bn == "Noto Sans Bengali" || bn == "SolaimanLipi");
        assert_eq!(en, SANS);
        // Arial is used when installed, else its same-width substitute.
        let (_, en) = match_original("Arial-BoldMT", false, false);
        assert!(en == "Arial" || en == SANS, "{en}");
        let (_, en) = match_original("Helvetica", false, false);
        assert!(en == "Arial" || en == SANS, "{en}");
        let (_, en) = match_original("TimesNewRomanPSMT", true, false);
        assert!(en == "Times New Roman" || en == SERIF, "{en}");
    }

    #[test]
    fn unknown_choices_fall_back_safely() {
        let dir = dev_fonts_dir();
        assert_eq!(bangla_faces("No Such Font", &dir).regular, dir.join("NotoSansBengali-Regular.ttf"));
        assert_eq!(english_face("No Such Font"), EnglishFace::Generic { css: "sans-serif".into() });
        assert!(font_options().iter().any(|o| o.name == "Hind Siliguri"));
    }
}
