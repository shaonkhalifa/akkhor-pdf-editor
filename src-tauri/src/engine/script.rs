//! Script detection for mixed Bangla + English text (SRS FR-6.10).
//!
//! Rules:
//! - U+0980..=U+09FF (Bengali block), danda U+0964/U+0965 and the joiners
//!   ZWNJ U+200C / ZWJ U+200D are **Bangla**.
//! - Whitespace, ASCII digits and punctuation are **Neutral**: they take the
//!   script of the text around them.
//! - Everything else is **English** (Latin and other symbols).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Bangla,
    English,
    Neutral,
}

/// Classify a single character.
pub fn classify(c: char) -> Script {
    let u = c as u32;
    if (0x0980..=0x09FF).contains(&u) || u == 0x0964 || u == 0x0965 || u == 0x200C || u == 0x200D {
        return Script::Bangla;
    }
    if c.is_whitespace() || c.is_ascii_digit() || c.is_ascii_punctuation() || is_general_punct(u) {
        return Script::Neutral;
    }
    Script::English
}

/// Common non-ASCII punctuation found in Bangla and English documents.
fn is_general_punct(u: u32) -> bool {
    matches!(
        u,
        0x00A0          // no-break space
        | 0x2010..=0x2015 // hyphens and dashes (– —)
        | 0x2018..=0x201F // curly quotes
        | 0x2026          // ellipsis
        | 0x2022          // bullet
        | 0x00AB | 0x00BB // « »
        | 0x00B7          // middle dot
    )
}

/// A run of text that should be set in one script's font.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub script: Script,
    pub text: String,
}

/// Split text into runs of Bangla or English.
///
/// Neutral characters join the previous strong script; neutral characters at
/// the very start join the first strong script that follows. A string with no
/// strong characters at all is a single English run.
pub fn split_runs(text: &str) -> Vec<Run> {
    let chars: Vec<char> = text.chars().collect();
    let first_strong = chars
        .iter()
        .map(|&c| classify(c))
        .find(|s| *s != Script::Neutral)
        .unwrap_or(Script::English);

    let mut runs: Vec<Run> = Vec::new();
    let mut current = first_strong;
    for c in chars {
        let s = match classify(c) {
            Script::Neutral => current,
            strong => strong,
        };
        current = s;
        match runs.last_mut() {
            Some(r) if r.script == s => r.text.push(c),
            _ => runs.push(Run { script: s, text: c.to_string() }),
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_bangla_block_danda_and_joiners() {
        for c in ['ক', 'ি', '্', '৳', '১', '।', '॥', '\u{200C}', '\u{200D}'] {
            assert_eq!(classify(c), Script::Bangla, "{c:?} should be Bangla");
        }
    }

    #[test]
    fn classifies_neutral_and_english() {
        for c in [' ', '1', ',', '.', ':', '—', '“'] {
            assert_eq!(classify(c), Script::Neutral, "{c:?} should be Neutral");
        }
        for c in ['A', 'z', 'é'] {
            assert_eq!(classify(c), Script::English, "{c:?} should be English");
        }
    }

    #[test]
    fn neutral_takes_surrounding_script() {
        let runs = split_runs("Total: ৳৫,০০০ only");
        assert_eq!(
            runs,
            vec![
                Run { script: Script::English, text: "Total: ".into() },
                Run { script: Script::Bangla, text: "৳৫,০০০ ".into() },
                Run { script: Script::English, text: "only".into() },
            ]
        );
    }

    #[test]
    fn leading_neutral_joins_first_strong_script() {
        let runs = split_runs("  ১২৩ abc");
        assert_eq!(runs[0], Run { script: Script::Bangla, text: "  ১২৩ ".into() });
        assert_eq!(runs[1], Run { script: Script::English, text: "abc".into() });
    }

    #[test]
    fn joiners_stay_inside_bangla_word() {
        let runs = split_runs("র\u{200D}্যাব");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].script, Script::Bangla);
    }
}
