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
