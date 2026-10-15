mod common;
use semantic_vocabulary::load;
use serde_json::Value;

fn fixture() -> Value {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/library-keys.json"),
    )
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn every_fixture_key_is_mapped_or_retired() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    let known = v.mapped_fields();
    let fx = fixture();
    for entry in fx["keys"].as_array().unwrap() {
        let field = entry["field"].as_str().unwrap();
        assert!(
            known
                .iter()
                .any(|k| k == field || field.starts_with(&format!("{k}."))),
            "fixture key {field} has no mapping or retired entry"
        );
    }
}

#[test]
fn mappings_name_defined_terms_and_allowed_values() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    let r = common::rules(&v.check());
    assert!(!r.contains(&"V010".to_string()), "{:?}", v.check());
}

#[test]
fn retired_fields_are_never_mapped() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    for f in ["custom_fields.isGeneric", "custom_fields.looks"] {
        assert!(
            v.retired_fields().iter().any(|r| r == f),
            "{f} must be a retired entry"
        );
        assert!(
            !v.mapping_fields().iter().any(|m| m == f),
            "{f} must not be mapped"
        );
    }
}

#[test]
fn live_capture_keys_are_mapped_or_declared_foreign() {
    let v = load(&common::repo_vocab_dir()).unwrap();
    let known = v.mapped_fields();
    let fx = fixture();
    let foreign: Vec<&str> = fx["live_capture"]["foreign_keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    for (entity, data) in fx["live_capture"]["entities"].as_object().unwrap() {
        for key in data["keys"].as_object().unwrap().keys() {
            let field = format!("custom_fields.{key}");
            assert!(
                known.iter().any(|k| *k == field) || foreign.contains(&key.as_str()),
                "live key {key} on {entity} is neither mapped nor declared foreign"
            );
        }
    }
}
