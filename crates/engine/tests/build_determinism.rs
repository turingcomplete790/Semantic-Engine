//! Mapping is deterministic, whatever the order of the records (spec 007 T015; SC-007, FR-010).

mod common;

use common::build::{fixture_library, iris};
use engine::build::map_library;
use serde_json::Value;
use stash_adapter::reader::field::Field;

/// Reverses every JSON object's entries and every array, recursively.
fn reversed(v: &Value) -> Value {
    match v {
        Value::Array(xs) => Value::Array(xs.iter().rev().map(reversed).collect()),
        Value::Object(o) => {
            let mut out = serde_json::Map::new();
            for (k, x) in o.iter().rev() {
                out.insert(k.clone(), reversed(x));
            }
            Value::Object(out)
        }
        x => x.clone(),
    }
}

#[test]
fn mapping_twice_and_in_reverse_order_gives_the_same_facts() {
    let read = fixture_library();
    let once = map_library(&read, &iris()).facts;
    let twice = map_library(&read, &iris()).facts;
    assert_eq!(once, twice);

    let mut rev = read.clone();
    rev.tags.reverse();
    rev.performers.reverse();
    rev.scenes.reverse();
    rev.images.reverse();
    for m in rev.scenes.iter_mut().chain(rev.images.iter_mut()) {
        m.performers.reverse();
        for f in m.fields.values_mut() {
            if let Field::Value(v) = f {
                // The timeline's order is meaningful (positions), so it is kept.
                *f = Field::Value(
                    if v.is_array() && v.get(0).and_then(|e| e.get("id")).is_some() {
                        v.clone()
                    } else {
                        reversed(v)
                    },
                );
            }
        }
    }
    for t in &mut rev.tags {
        t.parents.reverse();
    }
    assert_eq!(once, map_library(&rev, &iris()).facts);
}
