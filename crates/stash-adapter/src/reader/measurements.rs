//! Bust Size, Waist, Hip, and Waist-to-Hip Ratio derived from a performer's free-text
//! `measurements` field (spec 005 FR-002b, research R2).
//!
//! This follows the semantic-tagging plugin's `parseMeasurements` and `DERIVED_PERFORMER_FIELDS`
//! (`src/ui/matrix.js` at commit `2c04141`, lines 218–356), which no design document describes
//! yet; the plugin is asked to document it (`specs/005-stash-adapter/plugin-notes.md`). The
//! plugin matches `^(\d+)\s*([A-Za-z]*)\s*(½|\.5|1\/2)?\s*(?:-\s*(\d+)\s*-\s*(\d+))?` against
//! the trimmed text; the parser below takes the same path through it by hand, with JavaScript's
//! meaning of `\s` and `\d`. Parity with the plugin's results is checked by
//! `tests/reader_measurements.rs` (SC-010).

use serde::{Deserialize, Serialize};

/// Half-cup steps from AA to L. DD, DDD and DDDD are read as E, F and G first.
const CUP_STEPS: [(&str, f64); 18] = [
    ("AA", 0.0),
    ("A", 1.0),
    ("A½", 2.0),
    ("B", 3.0),
    ("B½", 4.0),
    ("C", 5.0),
    ("C½", 6.0),
    ("D", 7.0),
    ("D½", 8.0),
    ("E", 9.0),
    ("E½", 10.0),
    ("F", 11.0),
    ("G", 12.0),
    ("H", 13.0),
    ("I", 14.0),
    ("J", 15.0),
    ("K", 16.0),
    ("L", 17.0),
];
const CUP_SYNONYMS: [(&str, &str); 3] = [("DD", "E"), ("DDD", "F"), ("DDDD", "G")];
const CUP_MULTIPLIER: f64 = 2.5;
const WAIST_BONUS_THRESHOLD: f64 = 28.0;
const WAIST_BONUS_PER_INCH: f64 = 2.0;

/// The raw text and what can be derived from it. A value that cannot be derived is absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Measurements {
    pub raw: String,
    pub bust_size: Option<f64>,
    pub waist: Option<i64>,
    pub hip: Option<i64>,
    pub waist_to_hip_ratio: Option<f64>,
}

/// JavaScript's `\s`: Unicode spaces plus the line terminators and the byte-order mark.
fn js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

struct Cursor<'a> {
    s: &'a str,
    at: usize,
}

impl Cursor<'_> {
    fn rest(&self) -> &str {
        &self.s[self.at..]
    }
    fn spaces(&mut self) {
        let n: usize = self
            .rest()
            .chars()
            .take_while(|c| js_space(*c))
            .map(char::len_utf8)
            .sum();
        self.at += n;
    }
    fn take_while(&mut self, f: impl Fn(char) -> bool) -> &str {
        let start = self.at;
        let n: usize = self
            .rest()
            .chars()
            .take_while(|c| f(*c))
            .map(char::len_utf8)
            .sum();
        self.at += n;
        &self.s[start..self.at]
    }
    fn digits(&mut self) -> Option<f64> {
        let d = self.take_while(|c| c.is_ascii_digit());
        // `Number("024")` is 24; a run too long for u64 still parses as a float, as in JS.
        (!d.is_empty()).then(|| d.parse::<f64>().unwrap_or(f64::INFINITY))
    }
    fn literal(&mut self, lit: &str) -> bool {
        if self.rest().starts_with(lit) {
            self.at += lit.len();
            true
        } else {
            false
        }
    }
}

/// The `(?:-\s*(\d+)\s*-\s*(\d+))?` group, after its leading `\s*`; `None` if it does not match.
fn waist_and_hip(c: &mut Cursor) -> Option<(f64, f64)> {
    c.spaces();
    if !c.literal("-") {
        return None;
    }
    c.spaces();
    let waist = c.digits()?;
    c.spaces();
    if !c.literal("-") {
        return None;
    }
    c.spaces();
    Some((waist, c.digits()?))
}

/// JavaScript's `Math.round`: the nearest integer, with ties toward +∞.
fn js_round(x: f64) -> f64 {
    let f = x.floor();
    if x - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

impl Measurements {
    pub fn parse(raw: &str) -> Measurements {
        let none = Measurements {
            raw: raw.to_string(),
            bust_size: None,
            waist: None,
            hip: None,
            waist_to_hip_ratio: None,
        };
        let trimmed = raw.trim_matches(js_space);
        let mut c = Cursor { s: trimmed, at: 0 };
        let Some(band) = c.digits() else {
            return none;
        };
        if band == 0.0 {
            return none;
        }
        c.spaces();
        let letters = c
            .take_while(|ch| ch.is_ascii_alphabetic())
            .to_ascii_uppercase();
        c.spaces();
        let half = c.literal("½") || c.literal(".5") || c.literal("1/2");
        // The text after the match is ignored, so a failed group needs no rewind.
        let (waist, hip) = waist_and_hip(&mut c).unzip();

        let base = CUP_SYNONYMS
            .iter()
            .find(|(from, _)| *from == letters)
            .map_or(letters.as_str(), |(_, to)| to);
        let cup = format!("{base}{}", if half { "½" } else { "" });
        let mut bust = band;
        if let Some((_, step)) = CUP_STEPS.iter().find(|(name, _)| *name == cup) {
            bust += step * CUP_MULTIPLIER;
        }
        if let Some(w) = waist {
            if w < WAIST_BONUS_THRESHOLD {
                bust += (WAIST_BONUS_THRESHOLD - w) * WAIST_BONUS_PER_INCH;
            }
        }
        // The plugin's `waist && hip`: a 0 on either side gives no ratio.
        let ratio = match (waist, hip) {
            (Some(w), Some(h)) if w != 0.0 && h != 0.0 => Some(js_round(w / h * 100.0) / 100.0),
            _ => None,
        };
        Measurements {
            raw: raw.to_string(),
            bust_size: Some(bust),
            waist: waist.map(|w| w as i64),
            hip: hip.map(|h| h as i64),
            waist_to_hip_ratio: ratio,
        }
    }
}
