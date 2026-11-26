//! The Bangla shaping acceptance check (SRS section 6.4) and the Phase 1
//! text-edit spike, as plain functions used by both the automated tests and
//! the app's self-test screen.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use super::edit::replace_text;
use super::extract::{document_text, open_pdf, page_lines, LineInfo};
use super::fonts::{BanglaFont, BANGLA_FONTS};
use super::layout::{lines_to_pdf, LayoutOptions};

/// SRS 6.4 test lines: conjuncts, reph, ya-/ra-phala, khanda ta, pre-base and
/// split vowels, ZWJ and ZWNJ, danda, Bangla digits and the taka sign.
pub const TEST_LINES: &[&str] = &[
    "Hello World — Invoice No: ১২৩ — ক্ষ্ম স্ত্রী প্রত্যেক কিংবা বিজ্ঞান রৌদ্র — Total: ৳৫,০০০",
    "কর্ম ধর্ম অর্জন — ব্যাকরণ ক্যামেরা — প্রমাণ গ্রাম — মুক্তি — হঠাৎ উৎস",
    "র\u{200D}্যাব — ক্\u{200C}ষ — আমি বাংলায় গান গাই। Bangla and English মিশ্র line॥",
];

/// Compare text while ignoring layout whitespace (line breaks, spacing).
pub fn normalize(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct FontCheck {
    pub font: String,
    pub passed: bool,
    pub expected: String,
    pub extracted: String,
    #[serde(skip)]
    pub pdf: Vec<u8>,
}

/// Create the test PDF with one Bangla font and check that the text reads back
/// as exactly the same Unicode.
pub fn check_font(font: BanglaFont, fonts_dir: &Path) -> Result<FontCheck> {
    let pdf = lines_to_pdf(TEST_LINES, &LayoutOptions::a4(font), fonts_dir)?;
    let extracted = document_text(&pdf)?;
    let expected = TEST_LINES.join("\n");
    Ok(FontCheck {
        font: font.name.to_string(),
        passed: normalize(&extracted) == normalize(&expected),
        expected,
        extracted,
        pdf,
    })
}

pub fn check_all_fonts(fonts_dir: &Path) -> Result<Vec<FontCheck>> {
    BANGLA_FONTS.iter().map(|f| check_font(*f, fonts_dir)).collect()
}

pub const SPIKE_LINES: &[&str] = &[
    "চিঠি নং ৪৫: আমি বাংলায় গান গাই।",
    "এই লাইনটি বদলাবে না — This line stays the same.",
];
pub const SPIKE_OLD: &str = "আমি বাংলায় গান গাই";
pub const SPIKE_NEW: &str = "আমরা বিজ্ঞান ও ক্ষমতা শিখি";

#[derive(Debug, Clone, Serialize)]
pub struct SpikeResult {
    pub passed: bool,
    pub before_text: String,
    pub after_text: String,
    #[serde(skip)]
    pub before_pdf: Vec<u8>,
    #[serde(skip)]
    pub after_pdf: Vec<u8>,
}

/// Make a two-line PDF, replace Bangla words on line 1, save, reopen and check.
pub fn run_edit_spike(font: BanglaFont, fonts_dir: &Path) -> Result<SpikeResult> {
    let before_pdf = lines_to_pdf(SPIKE_LINES, &LayoutOptions::a4(font), fonts_dir)?;
    let after_pdf = replace_text(&before_pdf, 0, SPIKE_OLD, SPIKE_NEW, font, fonts_dir)?;
    let before_text = document_text(&before_pdf)?;
    let after_text = document_text(&after_pdf)?;

    let after = normalize(&after_text);
    let expected_line1 = normalize(&SPIKE_LINES[0].replacen(SPIKE_OLD, SPIKE_NEW, 1));
    let text_ok = after.contains(&expected_line1)
        && after.contains(&normalize(SPIKE_LINES[1]))
        && !after.contains(&normalize(SPIKE_OLD));

    // The edited line must sit where the old one was: same baseline and the
    // same left edge (within 1.5 pt), measured from the glyphs actually drawn.
    let line_one = |pdf: &[u8]| -> Result<LineInfo> {
        let doc = open_pdf(pdf)?;
        page_lines(&doc, 0)?.into_iter().next().context("no text on page")
    };
    let (b, a) = (line_one(&before_pdf)?, line_one(&after_pdf)?);
    let position_ok =
        (b.origin.y - a.origin.y).abs() <= 1.5 && (b.bbox.x0 - a.bbox.x0).abs() <= 1.5;
    let passed = text_ok && position_ok;

    Ok(SpikeResult { passed, before_text, after_text, before_pdf, after_pdf })
}
