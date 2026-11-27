// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Ported to Rust from bijoy2unicode by Rajib Biswas
// (https://github.com/Mad-FOX/bijoy2unicode, MPL-2.0): the character tables
// below are generated from its converter.py, and `rearrange` follows its
// reArrangeUnicodeConvertedText step by step. Changes: indexes outside the
// text count as "no character" (Python wrapped them around), and the regex
// patterns of the original are applied as plain text replacements.

//! Bijoy to Unicode (SRS Module 9).
//!
//! Text typed with Bijoy keyboards in "MJ" fonts such as SutonnyMJ is stored
//! as Latin letters and symbols ("Avwg evsjvq Mvb MvB"); only the font makes
//! it look like Bangla. Converting it gives real Unicode Bangla ("আমি বাংলায়
//! গান গাই") that can be searched, copied and edited.

/// Is this a Bijoy (ANSI Bangla) font? Detected by the font name (FR-9.1):
/// SutonnyMJ and the other "MJ" fonts.
pub fn is_bijoy_font(name: &str) -> bool {
    let base = name.split_once('+').map(|(p, rest)| if p.len() == 6 { rest } else { name }).unwrap_or(name);
    let lower = base.to_lowercase().replace([' ', '_'], "");
    let family = lower.split(['-', ',']).next().unwrap_or(&lower);
    lower.contains("sutonny") || family.ends_with("mj") || family.ends_with("omj")
}

/// Does pasted text look like Bijoy? Used only to offer conversion: no
/// Unicode Bangla, and several of the symbols Bijoy uses for vowel signs
/// and conjuncts, or Bijoy-shaped words.
pub fn looks_like_bijoy(text: &str) -> bool {
    let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
    if chars.is_empty() || chars.iter().any(|c| ('\u{0980}'..='\u{09FF}').contains(c)) {
        return false;
    }
    let marks = chars.iter().filter(|c| "‡†ˆ‰Š‹ŒŽšœŸ¨©ª«¬®¯°±²³´µ¶·¸¹º»¼½¾¿ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏ×ØÙÚÛÜÝÞßàáâãäåæçèéêëìíîïðñòóôõö÷øùúûüýþ„…~".contains(**c)).count();
    // Typical Bijoy spellings: "Av" (আ), "w" + consonant (ি), "‡" (ে).
    let av = text.matches("Av").count();
    (marks >= 2 && marks * 12 >= chars.len()) || av >= 2
}

fn replace_all(text: &str, pairs: &[(&str, &str)]) -> String {
    let mut s = text.to_string();
    for (from, to) in pairs {
        if s.contains(from) {
            s = s.replace(from, to);
        }
    }
    s
}

/// The original's preConversionMap: tidy typing mistakes and spaces.
fn pre_convert(text: &str) -> String {
    let mut s = collapse_spaces(text);
    for (from, to) in [
        ("yy", "y"),
        ("vv", "v"),
        ("\u{AD}\u{AD}", "\u{AD}"),
        ("y&", "y"),
        ("\u{201E}&", "\u{201E}"),
        ("\u{2021}u", "u\u{2021}"),
        ("wu", "uw"),
        (" ,", ","),
        (" |", "|"),
        ("\\ ", ""),
        (" \\", ""),
        ("\\", ""),
    ] {
        s = s.replace(from, to);
    }
    // "\n +" and " +\n" drop spaces around line breaks; at most one blank line.
    let lines: Vec<&str> = s.split('\n').map(|l| l.trim_matches(' ')).collect();
    let mut out = lines.join("\n");
    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    out
}

fn collapse_spaces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = false;
    for c in text.chars() {
        if c == ' ' {
            if !last_space {
                out.push(c);
            }
            last_space = true;
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out
}

/// Convert Bijoy (SutonnyMJ) text to Unicode Bangla (FR-9.2).
pub fn to_unicode(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let s = pre_convert(text);
    let s = replace_all(&s, CONVERSION);
    let s = rearrange(&s);
    let s = move_typed_rephs(&s);
    let s = replace_all(&s, POST_CONVERSION);
    // Standard (NFC) form: ড় ঢ় য় as letter + nukta, as keyboards type them,
    // so search finds the converted text.
    s.replace('\u{09DC}', "\u{09A1}\u{09BC}").replace('\u{09DD}', "\u{09A2}\u{09BC}").replace('\u{09DF}', "\u{09AF}\u{09BC}")
}

const HALANT: char = '\u{09CD}';

fn is_pre_kar(c: Option<char>) -> bool {
    matches!(c, Some('ি' | 'ৈ' | 'ে'))
}
fn is_post_kar(c: Option<char>) -> bool {
    matches!(c, Some('া' | 'ো' | 'ৌ' | 'ৗ' | 'ু' | 'ূ' | 'ী' | 'ৃ'))
}
fn is_kar(c: Option<char>) -> bool {
    is_pre_kar(c) || is_post_kar(c)
}
fn is_consonant(c: Option<char>) -> bool {
    matches!(
        c,
        Some(
            'ক' | 'খ' | 'গ' | 'ঘ' | 'ঙ' | 'চ' | 'ছ' | 'জ' | 'ঝ' | 'ঞ' | 'ট' | 'ঠ' | 'ড' | 'ঢ' | 'ণ' | 'ত' | 'থ' | 'দ' | 'ধ' | 'ন'
                | 'প' | 'ফ' | 'ব' | 'ভ' | 'ম' | 'য' | 'র' | 'ল' | 'শ' | 'ষ' | 'স' | 'হ' | '\u{09DC}' | '\u{09DD}' | '\u{09DF}' | 'ৎ' | 'ং' | 'ঃ' | 'ঁ'
        )
    )
}
fn is_nukta(c: Option<char>) -> bool {
    c == Some('ঁ')
}
fn is_halant(c: Option<char>) -> bool {
    c == Some(HALANT)
}
fn is_space(c: Option<char>) -> bool {
    matches!(c, Some(' ' | '\t' | '\n' | '\r'))
}

fn at(s: &[char], i: isize) -> Option<char> {
    if i < 0 {
        None
    } else {
        s.get(i as usize).copied()
    }
}

/// s[from..to] with both ends kept inside the text.
fn part(s: &[char], from: isize, to: isize) -> Vec<char> {
    let n = s.len() as isize;
    let (a, b) = (from.clamp(0, n), to.clamp(0, n));
    if a >= b {
        Vec::new()
    } else {
        s[a as usize..b as usize].to_vec()
    }
}

/// How far back a reph (র্) moves: over consonant + halant pairs, and over
/// one vowel sign right before it.
fn reph_reach(s: &[char], i: isize) -> isize {
    let mut j = 1;
    loop {
        if i - j < 0 {
            break;
        }
        if is_consonant(at(s, i - j)) && is_halant(at(s, i - j - 1)) {
            j += 2;
        } else if j == 1 && is_kar(at(s, i - j)) {
            j += 1;
        } else {
            break;
        }
    }
    j
}

/// Move "র + halant" back in front of the letters it was typed after.
fn move_reph(s: &[char], i: isize, j: isize) -> Vec<char> {
    let mut t = part(s, 0, i - j);
    t.extend(at(s, i));
    t.extend(at(s, i + 1));
    t.extend(part(s, i - j, i));
    t.extend(part(s, i + 2, s.len() as isize));
    t
}

/// Akkhor's addition (not in bijoy2unicode): Bijoy types the reph (©, র্)
/// after the letter or conjunct it sits on — কর্ম is "Kg©" — but Unicode
/// puts it first. Move every such reph in front of its letters. A র +
/// hasanta typed on purpose ends with a zero-width non-joiner and stays.
fn move_typed_rephs(text: &str) -> String {
    let mut s: Vec<char> = text.chars().collect();
    let mut i: isize = 1;
    while i < s.len() as isize - 1 {
        let typed_after = at(&s, i) == Some('র')
            && is_halant(at(&s, i + 1))
            && at(&s, i + 2) != Some('\u{200C}')
            && (is_consonant(at(&s, i - 1)) || is_kar(at(&s, i - 1)))
            && !is_halant(at(&s, i - 1));
        if typed_after {
            let j = reph_reach(&s, i);
            s = move_reph(&s, i, j);
            i += 2;
            continue;
        }
        i += 1;
    }
    s.into_iter().collect()
}

/// Put characters in Unicode order: Bijoy types reph after the letters and
/// pre-base vowel signs (ি ে ৈ) before the consonant.
fn rearrange(text: &str) -> String {
    let mut s: Vec<char> = text.chars().collect();
    let len = |s: &Vec<char>| s.len() as isize;

    // Reph between two halants.
    let mut i: isize = 0;
    while i < len(&s) {
        if i < len(&s) - 1 && at(&s, i) == Some('র') && is_halant(at(&s, i + 1)) && is_halant(at(&s, i - 1)) {
            let j = reph_reach(&s, i);
            s = move_reph(&s, i, j);
            i += 1;
            continue;
        }
        i += 1;
    }

    // proConversionMap: a doubled halant becomes one.
    let joined: String = s.iter().collect();
    s = joined.replace("\u{09CD}\u{09CD}", "\u{09CD}").chars().collect();

    let mut i: isize = 0;
    while i < len(&s) {
        if i < len(&s) - 1
            && at(&s, i) == Some('র')
            && is_halant(at(&s, i + 1))
            && !is_halant(at(&s, i - 1))
            && is_halant(at(&s, i + 2))
        {
            let j = reph_reach(&s, i);
            s = move_reph(&s, i, j);
            i += 1;
            continue;
        }

        // Vowel sign + halant + consonant → halant + consonant + vowel sign.
        if i > 0 && at(&s, i) == Some(HALANT) && (is_kar(at(&s, i - 1)) || is_nukta(at(&s, i - 1))) && i < len(&s) - 1 {
            let mut t = part(&s, 0, i - 1);
            t.extend(at(&s, i));
            t.extend(at(&s, i + 1));
            t.extend(at(&s, i - 1));
            t.extend(part(&s, i + 2, len(&s)));
            s = t;
        }

        // র + halant + vowel sign → vowel sign + র + halant.
        if i > 0 && i < len(&s) - 1 && at(&s, i) == Some(HALANT) && at(&s, i - 1) == Some('র') && at(&s, i - 2) != Some(HALANT) && is_kar(at(&s, i + 1)) {
            let mut t = part(&s, 0, i - 1);
            t.extend(at(&s, i + 1));
            t.extend(at(&s, i - 1));
            t.extend(at(&s, i));
            t.extend(part(&s, i + 2, len(&s)));
            s = t;
        }

        // A pre-base vowel sign moves after its consonant (cluster); ে + া
        // becomes ো and ে + ৗ becomes ৌ.
        if i < len(&s) - 1 && is_pre_kar(at(&s, i)) && !is_space(at(&s, i + 1)) {
            let mut t = part(&s, 0, i);
            let mut j: isize = 1;
            while i + j < len(&s) - 1 && is_consonant(at(&s, i + j)) {
                if i + j < len(&s) && is_halant(at(&s, i + j + 1)) {
                    j += 2;
                } else {
                    break;
                }
            }
            t.extend(part(&s, i + 1, i + j + 1));
            let mut l = 0;
            let kar = at(&s, i);
            let after = at(&s, i + j + 1);
            if kar == Some('ে') && after == Some('া') {
                t.push('ো');
                l = 1;
            } else if kar == Some('ে') && after == Some('ৗ') {
                t.push('ৌ');
                l = 1;
            } else {
                t.extend(kar);
            }
            t.extend(part(&s, i + j + l + 1, len(&s)));
            s = t;
            i += j;
        }

        // Chandrabindu goes after a vowel sign.
        if i < len(&s) - 1 && is_nukta(at(&s, i)) && is_post_kar(at(&s, i + 1)) {
            let mut t = part(&s, 0, i);
            t.extend(at(&s, i + 1));
            t.extend(at(&s, i));
            t.extend(part(&s, i + 2, len(&s)));
            s = t;
        }

        i += 1;
    }
    s.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Tables generated from bijoy2unicode's converter.py (do not edit by hand)
// ---------------------------------------------------------------------------

/// `conversionMap` from bijoy2unicode, in its original order.
pub(crate) const CONVERSION: &[(&str, &str)] = &[
    ("Av", "\u{986}"),
    ("A", "\u{985}"),
    ("B", "\u{987}"),
    ("C", "\u{988}"),
    ("D", "\u{989}"),
    ("E", "\u{98A}"),
    ("F", "\u{98B}"),
    ("G", "\u{98F}"),
    ("H", "\u{990}"),
    ("I", "\u{993}"),
    ("J", "\u{994}"),
    ("K", "\u{995}"),
    ("L", "\u{996}"),
    ("M", "\u{997}"),
    ("N", "\u{998}"),
    ("O", "\u{999}"),
    ("P", "\u{99A}"),
    ("Q", "\u{99B}"),
    ("R", "\u{99C}"),
    ("S", "\u{99D}"),
    ("T", "\u{99E}"),
    ("U", "\u{99F}"),
    ("V", "\u{9A0}"),
    ("W", "\u{9A1}"),
    ("X", "\u{9A2}"),
    ("Y", "\u{9A3}"),
    ("Z", "\u{9A4}"),
    ("_", "\u{9A5}"),
    ("`", "\u{9A6}"),
    ("a", "\u{9A7}"),
    ("b", "\u{9A8}"),
    ("c", "\u{9AA}"),
    ("d", "\u{9AB}"),
    ("e", "\u{9AC}"),
    ("f", "\u{9AD}"),
    ("g", "\u{9AE}"),
    ("h", "\u{9AF}"),
    ("i", "\u{9B0}"),
    ("j", "\u{9B2}"),
    ("k", "\u{9B6}"),
    ("l", "\u{9B7}"),
    ("m", "\u{9B8}"),
    ("n", "\u{9B9}"),
    ("o", "\u{9DC}"),
    ("p", "\u{9DD}"),
    ("q", "\u{9DF}"),
    ("r", "\u{9CE}"),
    ("s", "\u{982}"),
    ("t", "\u{983}"),
    ("u", "\u{981}"),
    ("0", "\u{9E6}"),
    ("1", "\u{9E7}"),
    ("2", "\u{9E8}"),
    ("3", "\u{9E9}"),
    ("4", "\u{9EA}"),
    ("5", "\u{9EB}"),
    ("6", "\u{9EC}"),
    ("7", "\u{9ED}"),
    ("8", "\u{9EE}"),
    ("9", "\u{9EF}"),
    ("\u{2022}", "\u{999}\u{9CD}"),
    ("v", "\u{9BE}"),
    ("w", "\u{9BF}"),
    ("x", "\u{9C0}"),
    ("y", "\u{9C1}"),
    ("z", "\u{9C1}"),
    ("\u{201C}", "\u{9C1}"),
    ("\u{2013}", "\u{9C1}"),
    ("~", "\u{9C2}"),
    ("\u{192}", "\u{9C2}"),
    ("\u{201A}", "\u{9C2}"),
    ("\u{201E}\u{201E}", "\u{9C3}"),
    ("\u{201E}", "\u{9C3}"),
    ("\u{2026}", "\u{9C3}"),
    ("\u{2020}", "\u{9C7}"),
    ("\u{2021}", "\u{9C7}"),
    ("\u{2C6}", "\u{9C8}"),
    ("\u{2030}", "\u{9C8}"),
    ("\u{160}", "\u{9D7}"),
    ("|", "\u{964}"),
    ("&", "\u{9CD}\u{200C}"),
    ("^", "\u{9CD}\u{9AC}"),
    ("\u{2018}", "\u{9CD}\u{9A4}\u{9C1}"),
    ("\u{2019}", "\u{9CD}\u{9A5}"),
    ("\u{2039}", "\u{9CD}\u{995}"),
    ("\u{152}", "\u{9CD}\u{995}\u{9CD}\u{9B0}"),
    ("\u{201D}", "\u{99A}\u{9CD}"),
    ("\u{2014}", "\u{9CD}\u{9A4}"),
    ("\u{2DC}", "\u{9A6}\u{9CD}"),
    ("\u{2122}", "\u{9A6}\u{9CD}"),
    ("\u{161}", "\u{9A8}\u{9CD}"),
    ("\u{203A}", "\u{9A8}\u{9CD}"),
    ("\u{153}", "\u{9CD}\u{9A8}"),
    ("\u{178}", "\u{9CD}\u{9AC}"),
    ("\u{A1}", "\u{9CD}\u{9AC}"),
    ("\u{A2}", "\u{9CD}\u{9AD}"),
    ("\u{A3}", "\u{9CD}\u{9AD}\u{9CD}\u{9B0}"),
    ("\u{A4}", "\u{9AE}\u{9CD}"),
    ("\u{A5}", "\u{9CD}\u{9AE}"),
    ("\u{A6}", "\u{9CD}\u{9AC}"),
    ("\u{A7}", "\u{9CD}\u{9AE}"),
    ("\u{A8}", "\u{9CD}\u{9AF}"),
    ("\u{A9}", "\u{9B0}\u{9CD}"),
    ("\u{AA}", "\u{9CD}\u{9B0}"),
    ("\u{AB}", "\u{9CD}\u{9B0}"),
    ("\u{AC}", "\u{9CD}\u{9B2}"),
    ("\u{AD}", "\u{9CD}\u{9B2}"),
    ("\u{AE}", "\u{9B7}\u{9CD}"),
    ("\u{AF}", "\u{9B8}\u{9CD}"),
    ("\u{B0}", "\u{995}\u{9CD}\u{995}"),
    ("\u{B1}", "\u{995}\u{9CD}\u{99F}"),
    ("\u{B2}", "\u{995}\u{9CD}\u{9B7}\u{9CD}\u{9A3}"),
    ("\u{B3}", "\u{995}\u{9CD}\u{9A4}"),
    ("\u{B4}", "\u{995}\u{9CD}\u{9AE}"),
    ("\u{B5}", "\u{995}\u{9CD}\u{9B0}"),
    ("\u{B6}", "\u{995}\u{9CD}\u{9B7}"),
    ("\u{B7}", "\u{995}\u{9CD}\u{9B8}"),
    ("\u{B8}", "\u{997}\u{9C1}"),
    ("\u{B9}", "\u{99C}\u{9CD}\u{99E}"),
    ("\u{BA}", "\u{997}\u{9CD}\u{9A6}"),
    ("\u{BB}", "\u{997}\u{9CD}\u{9A7}"),
    ("\u{BC}", "\u{999}\u{9CD}\u{995}"),
    ("\u{BD}", "\u{999}\u{9CD}\u{997}"),
    ("\u{BE}", "\u{99C}\u{9CD}\u{99C}"),
    ("\u{BF}", "\u{9CD}\u{9A4}\u{9CD}\u{9B0}"),
    ("\u{C0}", "\u{99C}\u{9CD}\u{99D}"),
    ("\u{C1}", "\u{99C}\u{9CD}\u{99E}"),
    ("\u{C2}", "\u{99E}\u{9CD}\u{99A}"),
    ("\u{C3}", "\u{99E}\u{9CD}\u{99B}"),
    ("\u{C4}", "\u{99E}\u{9CD}\u{99C}"),
    ("\u{C5}", "\u{99E}\u{9CD}\u{99D}"),
    ("\u{C6}", "\u{99F}\u{9CD}\u{99F}"),
    ("\u{C7}", "\u{9A1}\u{9CD}\u{9A1}"),
    ("\u{C8}", "\u{9A3}\u{9CD}\u{99F}"),
    ("\u{C9}", "\u{9A3}\u{9CD}\u{9A0}"),
    ("\u{CA}", "\u{9A3}\u{9CD}\u{9A1}"),
    ("\u{CB}", "\u{9A4}\u{9CD}\u{9A4}"),
    ("\u{CC}", "\u{9A4}\u{9CD}\u{9A5}"),
    ("\u{CD}", "\u{9A4}\u{9CD}\u{9AE}"),
    ("\u{CE}", "\u{9A4}\u{9CD}\u{9B0}"),
    ("\u{CF}", "\u{9A6}\u{9CD}\u{9A6}"),
    ("\u{D0}", "-"),
    ("\u{D1}", "-"),
    ("\u{D2}", "\u{22}"),
    ("\u{D3}", "\u{22}"),
    ("\u{D4}", "'"),
    ("\u{D5}", "'"),
    ("\u{D6}", "\u{9CD}\u{9B0}"),
    ("\u{D7}", "\u{9A6}\u{9CD}\u{9A7}"),
    ("\u{D8}", "\u{9A6}\u{9CD}\u{9AC}"),
    ("\u{D9}", "\u{9A6}\u{9CD}\u{9AE}"),
    ("\u{DA}", "\u{9A8}\u{9CD}\u{9A0}"),
    ("\u{DB}", "\u{9A8}\u{9CD}\u{9A1}"),
    ("\u{DC}", "\u{9A8}\u{9CD}\u{9A7}"),
    ("\u{DD}", "\u{9A8}\u{9CD}\u{9B8}"),
    ("\u{DE}", "\u{9AA}\u{9CD}\u{99F}"),
    ("\u{DF}", "\u{9AA}\u{9CD}\u{9A4}"),
    ("\u{E0}", "\u{9AA}\u{9CD}\u{9AA}"),
    ("\u{E1}", "\u{9AA}\u{9CD}\u{9B8}"),
    ("\u{E2}", "\u{9AC}\u{9CD}\u{99C}"),
    ("\u{E3}", "\u{9AC}\u{9CD}\u{9A6}"),
    ("\u{E4}", "\u{9AC}\u{9CD}\u{9A7}"),
    ("\u{E5}", "\u{9AD}\u{9CD}\u{9B0}"),
    ("\u{E6}", "\u{9AE}\u{9CD}\u{9A8}"),
    ("\u{E7}", "\u{9AE}\u{9CD}\u{9AB}"),
    ("\u{E8}", "\u{9CD}\u{9A8}"),
    ("\u{E9}", "\u{9B2}\u{9CD}\u{995}"),
    ("\u{EA}", "\u{9B2}\u{9CD}\u{997}"),
    ("\u{EB}", "\u{9B2}\u{9CD}\u{99F}"),
    ("\u{EC}", "\u{9B2}\u{9CD}\u{9A1}"),
    ("\u{ED}", "\u{9B2}\u{9CD}\u{9AA}"),
    ("\u{EE}", "\u{9B2}\u{9CD}\u{9AB}"),
    ("\u{EF}", "\u{9B6}\u{9C1}"),
    ("\u{F0}", "\u{9B6}\u{9CD}\u{99A}"),
    ("\u{F1}", "\u{9B6}\u{9CD}\u{99B}"),
    ("\u{F2}", "\u{9B7}\u{9CD}\u{9A3}"),
    ("\u{F3}", "\u{9B7}\u{9CD}\u{99F}"),
    ("\u{F4}", "\u{9B7}\u{9CD}\u{9A0}"),
    ("\u{F5}", "\u{9B7}\u{9CD}\u{9AB}"),
    ("\u{F6}", "\u{9B8}\u{9CD}\u{996}"),
    ("\u{F7}", "\u{9B8}\u{9CD}\u{99F}"),
    ("\u{F8}", "\u{9B8}\u{9CD}\u{9A8}"),
    ("\u{F9}", "\u{9B8}\u{9CD}\u{9AB}"),
    ("\u{FA}", "\u{9CD}\u{9AA}"),
    ("\u{FB}", "\u{9B9}\u{9C1}"),
    ("\u{FC}", "\u{9B9}\u{9C3}"),
    ("\u{FD}", "\u{9B9}\u{9CD}\u{9A8}"),
    ("\u{FE}", "\u{9B9}\u{9CD}\u{9AE}"),
];

/// `postConversionMap` from bijoy2unicode, in its original order.
pub(crate) const POST_CONVERSION: &[(&str, &str)] = &[
    ("\u{9E6}\u{983}", "\u{9E6}:"),
    ("\u{9E7}\u{983}", "\u{9E7}:"),
    ("\u{9E8}\u{983}", "\u{9E8}:"),
    ("\u{9E9}\u{983}", "\u{9E9}:"),
    ("\u{9EA}\u{983}", "\u{9EA}:"),
    ("\u{9EB}\u{983}", "\u{9EB}:"),
    ("\u{9EC}\u{983}", "\u{9EC}:"),
    ("\u{9ED}\u{983}", "\u{9ED}:"),
    ("\u{9EE}\u{983}", "\u{9EE}:"),
    ("\u{9EF}\u{983}", "\u{9EF}:"),
    (" \u{983}", ":"),
    ("\u{A}\u{983}", "\u{A}:"),
    ("]\u{983}", "]:"),
    ("[\u{983}", "\u{5C}[:"),
    ("  ", " "),
    ("\u{985}\u{9BE}", "\u{986}"),
    ("\u{9CD}\u{200C}\u{9CD}\u{200C}", "\u{9CD}\u{200C}"),
];


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_common_bijoy_text() {
        let cases = [
            ("Avwg evsjvq Mvb MvB", "আমি বাংলায় গান গাই"),
            ("evsjv‡`k", "বাংলাদেশ"),
            ("Kg©", "কর্ম"),
            ("ag©", "ধর্ম"),
            ("c~e©", "পূর্ব"),
            ("mg_©b", "সমর্থন"),
            ("Kv‡h©i", "কার্যের"),
            ("cÖv_wgK", "প্রাথমিক"),
            ("we`¨vjq", "বিদ্যালয়"),
            ("†jvK", "লোক"),
            ("¶gZv", "ক্ষমতা"),
            ("weÁvb", "বিজ্ঞান"),
            ("Pv`", "চাদ"),
            ("Puv`", "চাঁদ"),
            ("ˆZwi", "তৈরি"),
            ("†gŠwjK", "মৌলিক"),
            ("2024 mvj", "২০২৪ সাল"),
        ];
        for (bijoy, unicode) in cases {
            assert_eq!(to_unicode(bijoy), unicode, "{bijoy}");
        }
    }

    #[test]
    fn spots_bijoy_fonts_and_text() {
        assert!(is_bijoy_font("SutonnyMJ"));
        assert!(is_bijoy_font("ABCDEF+SutonnyMJ-Bold"));
        assert!(is_bijoy_font("SulekhaTMJ"));
        assert!(!is_bijoy_font("NotoSansBengali-Regular"));
        assert!(!is_bijoy_font("Arial"));
        assert!(looks_like_bijoy("evsjv‡`‡ki ¯^vaxbZv"));
        assert!(looks_like_bijoy("Avwg Avi Avcwb"));
        assert!(!looks_like_bijoy("Invoice No. 2024 – total"));
        assert!(!looks_like_bijoy("আমি বাংলায়"));
    }
}
