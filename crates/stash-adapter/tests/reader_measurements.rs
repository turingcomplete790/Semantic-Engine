//! Derived measurements match the plugin's `parseMeasurements` (spec 005 T008; FR-002b, SC-010).
//! The expected values come from `fixtures/reader/gen-measurements.mjs`, which runs the plugin's
//! own code at commit 2c04141.

use serde_json::Value;
use stash_adapter::reader::measurements::Measurements;

fn cases() -> Vec<Value> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/reader/measurements.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn every_parity_case_matches_the_plugin() {
    let cases = cases();
    assert!(cases.len() >= 30, "SC-010 needs at least 30 cases");
    for c in cases {
        let raw = c["measurements"].as_str().unwrap();
        let m = Measurements::parse(raw);
        assert_eq!(m.bust_size, c["bust"].as_f64(), "bust of {raw:?}");
        assert_eq!(m.waist, c["waist"].as_i64(), "waist of {raw:?}");
        assert_eq!(m.hip, c["hip"].as_i64(), "hip of {raw:?}");
        assert_eq!(m.waist_to_hip_ratio, c["whr"].as_f64(), "whr of {raw:?}");
    }
}

#[test]
fn the_raw_text_is_always_kept() {
    for c in cases() {
        let raw = c["measurements"].as_str().unwrap();
        assert_eq!(Measurements::parse(raw).raw, raw);
    }
}

#[test]
fn text_that_does_not_parse_derives_nothing() {
    for raw in ["natural", "", "N/A", "-24-34", "0D-24-34"] {
        let m = Measurements::parse(raw);
        assert_eq!(
            (m.bust_size, m.waist, m.hip, m.waist_to_hip_ratio),
            (None, None, None, None),
            "{raw:?}"
        );
    }
}
