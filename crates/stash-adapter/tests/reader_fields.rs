//! Field parsing, flags, and the display-only graph position (spec 005 T006; FR-013, FR-014,
//! FR-002a; data-model "Field").

use serde_json::{json, Value};
use stash_adapter::reader::field::{flag_set, graph_position, parse_field, Field, GraphPosition};
use stash_adapter::reader::keys::Shape;

fn value(f: Field) -> Value {
    match f {
        Field::Value(v) => v,
        Field::Unreadable { raw, reason } => panic!("unreadable {raw}: {reason}"),
    }
}

fn unreadable(f: Field) -> (Value, String) {
    match f {
        Field::Unreadable { raw, reason } => (raw, reason),
        Field::Value(v) => panic!("expected unreadable, got {v}"),
    }
}

#[test]
fn json_text_of_the_expected_kind_parses() {
    assert_eq!(
        value(parse_field(Shape::List, &json!("[\"a\",\"b\"]"))),
        json!(["a", "b"])
    );
    assert_eq!(
        value(parse_field(Shape::Object, &json!("{\"7\":[\"19\"]}"))),
        json!({"7": ["19"]})
    );
}

#[test]
fn a_json_value_stored_directly_is_accepted() {
    assert_eq!(
        value(parse_field(Shape::List, &json!([1, 2]))),
        json!([1, 2])
    );
    assert_eq!(
        value(parse_field(Shape::Object, &json!({"a": 1}))),
        json!({"a": 1})
    );
}

#[test]
fn invalid_json_keeps_the_raw_text() {
    let (raw, reason) = unreadable(parse_field(Shape::Object, &json!("{not json")));
    assert_eq!(raw, json!("{not json"));
    assert!(!reason.is_empty());
}

#[test]
fn the_wrong_top_level_kind_is_unreadable() {
    let (raw, reason) = unreadable(parse_field(Shape::Object, &json!("[1,2]")));
    assert_eq!(raw, json!("[1,2]"));
    assert!(reason.contains("object"), "{reason}");
    let (_, reason) = unreadable(parse_field(Shape::List, &json!({"a": 1})));
    assert!(reason.contains("list"), "{reason}");
    unreadable(parse_field(Shape::List, &json!("")));
    unreadable(parse_field(Shape::List, &json!(true)));
}

#[test]
fn ids_are_kept_as_strings() {
    assert_eq!(value(parse_field(Shape::Id, &json!("12"))), json!("12"));
    assert_eq!(value(parse_field(Shape::Id, &json!(" 12 "))), json!("12"));
    assert_eq!(value(parse_field(Shape::Id, &json!(12))), json!("12"));
    unreadable(parse_field(Shape::Id, &json!("")));
    unreadable(parse_field(Shape::Id, &json!([12])));
    unreadable(parse_field(Shape::Id, &json!(1.5)));
}

#[test]
fn the_four_spellings_of_a_set_flag() {
    for v in [
        json!(1),
        json!("1"),
        json!(true),
        json!("true"),
        json!(" 1 "),
    ] {
        assert!(flag_set(&v), "{v} is set");
    }
}

#[test]
fn every_other_flag_value_is_not_set_and_not_an_error() {
    for v in [
        json!(0),
        json!("0"),
        json!(false),
        json!("false"),
        json!("yes"),
        Value::Null,
        json!(2),
        json!(""),
        json!([1]),
    ] {
        assert!(!flag_set(&v), "{v} is not set");
    }
}

#[test]
fn a_graph_position_is_two_numbers() {
    assert_eq!(
        graph_position(&json!("{\"x\":1.5,\"y\":-2}")),
        Some(GraphPosition { x: 1.5, y: -2.0 })
    );
    assert_eq!(
        graph_position(&json!({"x": 0, "y": 3})),
        Some(GraphPosition { x: 0.0, y: 3.0 })
    );
}

#[test]
fn a_bad_graph_position_means_not_placed() {
    for v in [
        json!("{\"x\":1}"),
        json!("{\"x\":\"1\",\"y\":2}"),
        json!("{not json"),
        json!(""),
        json!([1, 2]),
        Value::Null,
    ] {
        assert_eq!(graph_position(&v), None, "{v}");
    }
}
