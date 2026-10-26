//! Bad data is skipped or noted field by field, never fatally (spec 007 T030; US3, SC-006).

mod common;

use common::build::{fixture_library, fixture_path, iris};
use engine::build::map_library;
use serde_json::Value;
use stash_adapter::reader::field::Field;

#[test]
fn each_bad_field_is_noted_exactly_once() {
    let m = map_library(&fixture_library(), &iris());
    let mut actual: Vec<Value> = m
        .notes
        .as_slice()
        .iter()
        .map(|n| serde_json::to_value(n).unwrap())
        .collect();
    let mut expected: Vec<Value> = serde_json::from_str(
        &std::fs::read_to_string(fixture_path("expected-notes.json")).unwrap(),
    )
    .unwrap();
    let key = |v: &Value| v.to_string();
    actual.sort_by_key(key);
    expected.sort_by_key(key);
    let missing: Vec<_> = expected.iter().filter(|e| !actual.contains(e)).collect();
    let extra: Vec<_> = actual.iter().filter(|a| !expected.contains(a)).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "missing notes: {missing:#?}\nextra notes: {extra:#?}"
    );
    assert_eq!(actual.len(), expected.len(), "a note appears twice");
}

/// Every string value in the library: names, categories, variations, rule fields, and the raw
/// text of unreadable fields.
fn strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(xs) => xs.iter().for_each(|x| strings(x, out)),
        Value::Object(o) => o.values().for_each(|x| strings(x, out)),
        _ => {}
    }
}

#[test]
fn no_note_line_holds_a_value() {
    let read = fixture_library();
    let mut values = Vec::new();
    for t in &read.tags {
        values.push(t.name.clone());
    }
    let fields = read
        .tags
        .iter()
        .map(|t| &t.fields)
        .chain(read.scenes.iter().map(|m| &m.fields));
    for f in fields.flat_map(|f| f.values()) {
        match f {
            Field::Value(v) => strings(v, &mut values),
            Field::Unreadable { raw, .. } => strings(raw, &mut values),
        }
    }
    // Ids and the plugin's own short codes (entry ids such as "t1") locate a note; values with
    // words in them (names, categories, variations, raw text) must never appear.
    values.retain(|s| s.contains(' ') || s.len() > 12);
    assert!(values.iter().any(|v| v == "Unused Category"));
    let m = map_library(&read, &iris());
    for n in m.notes.as_slice() {
        let line = n.log_line();
        for v in &values {
            assert!(!line.contains(v.as_str()), "{line:?} holds {v:?}");
        }
    }
}
